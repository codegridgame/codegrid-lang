use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use codegrid_level_api::{LevelApi, SafetyProfile, LEVEL_API_VERSION};
use serde_json::{json, Value};

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const CONFORMANCE: &str = include_str!("../../../tests/fixtures/conformance-v1.json");

struct TempFile(PathBuf);

impl TempFile {
    fn new(extension: &str, contents: &[u8]) -> Self {
        loop {
            let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "codegrid-cli-{}-{id}.{extension}",
                std::process::id()
            ));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary test file must be creatable: {error}"),
            };
            file.write_all(contents)
                .expect("temporary test data must be writable");
            return Self(path);
        }
    }

    fn text_path(&self) -> &str {
        self.0.to_str().expect("temporary paths are UTF-8")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run_cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codegrid"))
        .args(arguments)
        .output()
        .expect("the integration test must be able to start codegrid")
}

fn parse_result(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "CLI stdout must contain a JSON run result: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn configured_args<'a>(source: &'a str) -> Vec<&'a str> {
    vec![
        "run",
        source,
        "--boundary",
        "exit",
        "--seed",
        "0",
        "--custom-limit",
        "1000",
        "--max-ticks",
        "100",
        "--max-work-units",
        "100000",
    ]
}

fn exact_io_level(expected_output: &[u8]) -> String {
    json!({
        "format_version": 1,
        "level_id": "cli.evaluate.test",
        "level_version": 1,
        "evaluation_type": "ExactIO",
        "program_rules": {
            "allowed_instructions": ["HALT", "OUTPUT"],
            "allowed_attachments": [],
            "main_board": {"width": 8, "height": 2},
            "function_board": {"width": 8, "height": 2},
            "max_functions": 0,
            "max_custom": 0,
            "max_threads": 1,
            "memory_enabled": false
        },
        "constraints": {},
        "scoring": {"metrics": {}},
        "evaluation": {"tests": [{
            "visible": true,
            "input": [],
            "expected_output": expected_output
        }]}
    })
    .to_string()
}

fn evaluate_args<'a>(level: &'a str, source: &'a str, profile: &'a str) -> Vec<&'a str> {
    vec![
        "evaluate",
        level,
        source,
        "--mode",
        "debug",
        "--boundary",
        "exit",
        "--seed",
        "18446744073709551615",
        "--custom-limit",
        "1000",
        "--limits-file",
        profile,
    ]
}

fn level_api_call(api: &mut LevelApi, operation: &str, extra: Value) -> Value {
    let mut request = json!({"api_version": LEVEL_API_VERSION, "operation": operation});
    request
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let response = api.request_json(&request.to_string());
    serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("Level API response must be JSON ({error}): {response}"))
}

fn direct_api_evaluate(level_text: &str, source: &str, mode: &str) -> Value {
    let profile_text = include_str!("../../../fixtures/levels/profiles/local-v1.json");
    let profile = SafetyProfile::from_json(profile_text).expect("shared local profile is valid");
    let work_budget = profile.max_work_per_call.to_string();
    let mut api = LevelApi::new(profile).expect("level API must initialize");
    let loaded = level_api_call(&mut api, "load_level", json!({"level_json": level_text}));
    let level = loaded["handle"]
        .as_str()
        .expect("valid level returns handle")
        .to_owned();
    let compiled = level_api_call(&mut api, "compile_program", json!({"source": source}));
    let program = compiled["handle"]
        .as_str()
        .expect("valid program returns handle")
        .to_owned();
    let started = level_api_call(
        &mut api,
        "start_evaluation",
        json!({
            "level": level,
            "program": program,
            "mode": mode,
            "boundary_mode": "Exit",
            "shuffle_seed": "18446744073709551615",
            "custom_execution_limit": "1000"
        }),
    );
    let evaluation = started["handle"]
        .as_str()
        .expect("evaluation returns handle")
        .to_owned();
    loop {
        let response = level_api_call(
            &mut api,
            "advance_evaluation",
            json!({"evaluation": evaluation, "work_budget": work_budget}),
        );
        if response["status"] == "pending" {
            continue;
        }
        assert_eq!(
            response["status"], "result",
            "direct API evaluation must finish"
        );
        return response["result"].clone();
    }
}

#[test]
fn level_manifest_cli_matches_direct_rust_api_complete_results() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/levels");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("conformance-v1.json")).unwrap())
            .unwrap();
    for case in manifest["cases"].as_array().unwrap() {
        let profile = SafetyProfile::from_json(
            &std::fs::read_to_string(root.join(manifest["profile"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let mut api = LevelApi::new(profile.clone()).unwrap();
        let level_path = root.join(case["level"].as_str().unwrap());
        let source_path = root.join(case["program"].as_str().unwrap());
        let level = level_api_call(
            &mut api,
            "load_level",
            json!({"level_json":std::fs::read_to_string(&level_path).unwrap()}),
        );
        let program = level_api_call(
            &mut api,
            "compile_program",
            json!({"source":std::fs::read_to_string(&source_path).unwrap()}),
        );
        let evaluation = level_api_call(
            &mut api,
            "start_evaluation",
            json!({"level":level["handle"],"program":program["handle"],"mode":case["mode"],"boundary_mode":case["boundary"],"shuffle_seed":case["seed"],"custom_execution_limit":case["custom_limit"]}),
        );
        assert_eq!(evaluation["status"], "ok", "{case}: {evaluation}");
        let direct = loop {
            let response = level_api_call(
                &mut api,
                "advance_evaluation",
                json!({"evaluation":evaluation["handle"],"work_budget":profile.max_work_per_call.to_string()}),
            );
            if response["status"] != "pending" {
                break response;
            }
        };
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codegrid"))
            .arg("evaluate")
            .arg(level_path)
            .arg(source_path)
            .args([
                "--mode",
                &case["mode"].as_str().unwrap().to_ascii_lowercase(),
                "--boundary",
                &case["boundary"].as_str().unwrap().to_ascii_lowercase(),
                "--seed",
                case["seed"].as_str().unwrap(),
                "--custom-limit",
                case["custom_limit"].as_str().unwrap(),
                "--limits-file",
            ])
            .arg(root.join(manifest["profile"].as_str().unwrap()))
            .output()
            .unwrap();
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(cli, direct, "Complete permitted result for {}", case["id"]);
        assert_eq!(cli["result"]["status"], case["expected_status"]);
    }
}

#[test]
fn check_accepts_full_source_and_reports_compiler_diagnostics() {
    let valid = TempFile::new("cg", b"~> + ;\n");
    let output = run_cli(&["check", valid.text_path()]);
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid"));
    assert!(output.stderr.is_empty());

    let invalid = TempFile::new("cg", b"@size 1x1\n@main\n~> [0\n@end main\n");
    let output = run_cli(&["check", invalid.text_path()]);
    assert_eq!(output.status.code(), Some(4));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(":3:"), "{diagnostic}");
    assert!(diagnostic.contains("error:"), "{diagnostic}");
}

#[test]
fn check_reports_invalid_utf8_and_missing_sources_as_io_errors() {
    let invalid_utf8 = TempFile::new("cg", &[0xff, 0xfe]);
    let output = run_cli(&["check", invalid_utf8.text_path()]);
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8 source"));

    let missing =
        std::env::temp_dir().join(format!("codegrid-cli-absent-{}.cg", std::process::id()));
    let output = run_cli(&["check", missing.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn run_emits_a_source_error_result_without_creating_a_vm_snapshot() {
    let invalid = TempFile::new("cg", b"~> [0\n");
    let arguments = configured_args(invalid.text_path());
    let output = run_cli(&arguments);
    assert_eq!(output.status.code(), Some(4));
    let result = parse_result(&output);
    assert_eq!(result["status"], "source_error");
    assert_eq!(result["snapshot"], Value::Null);
    assert_eq!(result["yield_reason"], Value::Null);
    assert!(result["events"].as_array().unwrap().is_empty());
    assert!(result["newly_emitted_output"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!result["diagnostics"].as_array().unwrap().is_empty());
    assert_eq!(result["diagnostics"][0]["code"], "ir.undefined_function");
    assert_eq!(result["diagnostics"][0]["error_number"], "2015");
}

#[test]
fn native_host_failures_and_debug_errors_have_stable_identifiers() {
    use std::process::Stdio;
    let bad_args = run_cli(&["run"]);
    assert!(String::from_utf8_lossy(&bad_args.stderr).contains("[cli.invalid_arguments]"));
    assert!(String::from_utf8_lossy(&bad_args.stderr).contains("[5000] [cli.invalid_arguments]"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_codegrid"))
        .args(["debug", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"{\"command\":\"step\"}\n{\"command\":\"unknown\"}\n{\"command\":\"disconnect\"}\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses[0]["debug_protocol_version"], 2);
    assert_eq!(responses[0]["error"]["code"], "debug.no_program");
    assert_eq!(responses[0]["error"]["error_number"], "5103");
    assert_eq!(responses[1]["error"]["code"], "debug.invalid_request");
    assert_eq!(responses[1]["error"]["error_number"], "5100");
    assert!(responses[0]["error"]["message"].is_string());
}

#[test]
fn cli_matches_all_shared_full_runtime_fixtures() {
    let suite: Value = serde_json::from_str(CONFORMANCE).expect("Full fixture index is valid JSON");
    assert_eq!(suite["schema_version"], 1);
    for case in suite["cases"].as_array().expect("suite has cases") {
        let id = case["id"].as_str().unwrap();
        let source = TempFile::new("cg", case["source"].as_str().unwrap().as_bytes());
        let run = &case["run"];
        let input = run["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut arguments = configured_args(source.text_path());
        arguments[3] = run["boundary"].as_str().unwrap();
        arguments[5] = run["seed"].as_str().unwrap();
        arguments[7] = run["custom_execution_limit"].as_str().unwrap();
        arguments[9] = run["max_ticks"].as_str().unwrap();
        arguments.extend(["--input", &input]);
        let initial_memory_file = run["initial_memory"].as_array().map(|memory| {
            let bytes = serde_json::to_vec(memory).expect("fixture memory encodes as JSON");
            TempFile::new("json", &bytes)
        });
        if let Some(memory_file) = initial_memory_file.as_ref() {
            arguments.extend(["--initial-memory-file", memory_file.text_path()]);
        }
        let output = run_cli(&arguments);
        let expected = &case["expected"];
        assert_eq!(
            output.status.code(),
            Some(expected["process_exit_code"].as_i64().unwrap() as i32),
            "{id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = parse_result(&output);
        assert_eq!(result["schema"], "codegrid.cli.run-result", "{id}");
        assert_eq!(result["schema_version"], 1, "{id}");
        let expected_status = match expected["process_exit_code"].as_i64().unwrap() {
            0 => "halted",
            5 => "runtime_error",
            6 => "yielded",
            code => panic!("unexpected fixture exit code {code} for {id}"),
        };
        assert_eq!(result["status"], expected_status, "{id}");
        assert_eq!(result["snapshot"]["status"], expected["vm_status"], "{id}");
        assert_eq!(result["snapshot"]["output"], expected["output"], "{id}");
        assert_eq!(
            result["snapshot"]["registers"], expected["registers"],
            "{id}"
        );
        assert_eq!(
            result["snapshot"]["committed_ticks"], expected["committed_ticks"],
            "{id}"
        );
        assert_eq!(
            result["snapshot"]["metrics"]["operation_count"], expected["operation_count"],
            "{id}"
        );
        for metric in [
            "used_cell_count",
            "used_memory_address_count",
            "peak_data_stack_usage",
            "peak_instruction_stack_usage",
            "peak_call_stack_usage",
        ] {
            assert_eq!(
                result["snapshot"]["metrics"][metric], expected[metric],
                "{id}: {metric}"
            );
        }
        assert_eq!(
            result["snapshot"]["metrics"]["instruction_variety"], expected["instruction_variety"],
            "{id}"
        );
        if !expected["remaining_input"].is_null() {
            assert_eq!(
                result["snapshot"]["remaining_input"], expected["remaining_input"],
                "{id}: remaining input"
            );
        }
        if !expected["memory"].is_null() {
            let expected_memory = if expected["memory"].is_array() {
                expected["memory"].clone()
            } else {
                json!([expected["memory"]])
            };
            assert_eq!(
                result["snapshot"]["memory"], expected_memory,
                "{id}: memory"
            );
        }
        for (field, part) in [
            ("runtime_main_primary_tokens", "primary"),
            ("runtime_main_attachment_tokens", "attachment"),
        ] {
            if let Some(expected_tokens) = expected[field].as_array() {
                let actual_tokens = result["snapshot"]["runtime_program"]["main"]["cells"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|cell| cell[part].clone())
                    .collect::<Vec<_>>();
                assert_eq!(actual_tokens, *expected_tokens, "{id}: {field}");
            }
        }
        if let Some(direction) = expected["thread_direction"].as_str() {
            assert_eq!(
                result["snapshot"]["threads"][0]["direction"], direction,
                "{id}: thread direction"
            );
        }
        if !expected["thread_random_state"].is_null() {
            assert_eq!(
                result["snapshot"]["threads"][0]["random_state"], expected["thread_random_state"],
                "{id}: thread random state"
            );
        }
        for (expected_field, stack_field) in [
            ("thread_data_stack_sizes", "data_stack"),
            ("thread_instruction_stack_sizes", "instruction_stack"),
            ("thread_call_stack_sizes", "call_stack"),
        ] {
            if let Some(expected_sizes) = expected[expected_field].as_array() {
                let actual_sizes = result["snapshot"]["threads"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|thread| json!(thread[stack_field].as_array().unwrap().len()))
                    .collect::<Vec<_>>();
                assert_eq!(actual_sizes, *expected_sizes, "{id}: {expected_field}");
            }
        }
        if let Some(expected_threads) = expected["threads"].as_array() {
            let actual_threads = result["snapshot"]["threads"].as_array().unwrap();
            assert_eq!(
                actual_threads.len(),
                expected_threads.len(),
                "{id}: threads"
            );
            for (actual, expected_thread) in actual_threads.iter().zip(expected_threads) {
                for field in ["id", "position", "direction"] {
                    if !expected_thread[field].is_null() {
                        assert_eq!(
                            actual[field], expected_thread[field],
                            "{id}: thread {field}"
                        );
                    }
                }
            }
        }
        let errors = result["snapshot"]["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|error| error["code"].clone())
            .collect::<Vec<_>>();
        let expected_errors = expected["error_codes"].as_array().unwrap().clone();
        assert_eq!(errors, expected_errors, "{id}");
        let runtime_errors = result["snapshot"]["errors"].as_array().unwrap();
        for (expected_field, actual_values) in [
            (
                "error_global_ticks",
                runtime_errors
                    .iter()
                    .map(|error| error["global_tick"].clone())
                    .collect::<Vec<_>>(),
            ),
            (
                "error_custom_internal_ticks",
                runtime_errors
                    .iter()
                    .filter_map(|error| {
                        (error["scope"]["kind"] == "custom")
                            .then(|| error["scope"]["internal_tick"].clone())
                    })
                    .collect::<Vec<_>>(),
            ),
        ] {
            if let Some(expected_values) = expected[expected_field].as_array() {
                assert_eq!(actual_values, *expected_values, "{id}: {expected_field}");
            }
        }
        if let Some(expected_ids) = expected["error_thread_ids"].as_array() {
            let mut actual_ids = Vec::new();
            for error in runtime_errors {
                let details = &error["details"];
                if let Some(id) = details["thread_id"].as_str() {
                    actual_ids.push(json!(id));
                }
                for field in ["thread_ids", "internal_thread_ids"] {
                    if let Some(ids) = details[field].as_array() {
                        actual_ids.extend(ids.iter().cloned());
                    }
                }
            }
            assert_eq!(actual_ids, *expected_ids, "{id}: error thread IDs");
        }
        if let Some(expected_groups) = expected["error_thread_id_groups"].as_array() {
            let mut actual_groups = Vec::new();
            for error in runtime_errors {
                let details = &error["details"];
                if let Some(ids) = details["thread_ids"].as_array() {
                    actual_groups.push(json!(ids));
                } else if let Some(ids) = details["internal_thread_ids"].as_array() {
                    actual_groups.push(json!(ids));
                } else if let Some(id) = details["thread_id"].as_str() {
                    actual_groups.push(json!([id]));
                }
            }
            assert_eq!(actual_groups, *expected_groups, "{id}: error thread groups");
        }
        if expected_status == "yielded" {
            assert_eq!(result["yield_reason"], "tick_slice_exhausted", "{id}");
        } else {
            assert!(result["yield_reason"].is_null(), "{id}");
        }
    }
}

#[test]
fn run_uses_versioned_json_and_preserves_wide_integer_configuration() {
    let source = TempFile::new("cg", b"~> ;\n");
    let mut args = configured_args(source.text_path());
    args[5] = "18446744073709551615";
    let output = run_cli(&args);
    assert_eq!(output.status.code(), Some(0));
    let result = parse_result(&output);
    assert_eq!(result["configuration"]["seed"], "18446744073709551615");
    assert_eq!(result["configuration"]["max_work_units"], "100000");
    assert_eq!(result["snapshot"]["threads"].as_array().unwrap().len(), 1);
    assert_eq!(result["snapshot"]["threads"][0]["id"], "0");
    assert_eq!(result["snapshot"]["metrics"]["global_tick"], "2");
}

#[test]
fn run_normalizes_initial_memory_and_rejects_duplicate_or_malformed_entries() {
    let source = TempFile::new("cg", b"~> ;\n");
    let memory = TempFile::new(
        "json",
        br#"[{"address":"-1","value":12},{"address":"0","value":0}]"#,
    );
    let mut args = configured_args(source.text_path());
    args.extend(["--initial-memory-file", memory.text_path()]);
    let output = run_cli(&args);
    assert_eq!(output.status.code(), Some(0));
    let result = parse_result(&output);
    assert_eq!(
        result["configuration"]["initial_memory"],
        json!([{"address":"-1","value":12}])
    );
    assert_eq!(
        result["snapshot"]["memory"],
        json!([{"address":"-1","value":12}])
    );

    for data in [
        br#"[{"address":"1","value":1},{"address":"1","value":2}]"#.as_slice(),
        br#"[{"address":"01","value":1}]"#.as_slice(),
        br#"[{"address":"-0","value":1}]"#.as_slice(),
        br#"[{"address":"0","value":1,"extra":2}]"#.as_slice(),
    ] {
        let malformed = TempFile::new("json", data);
        let mut args = configured_args(source.text_path());
        args.extend(["--initial-memory-file", malformed.text_path()]);
        let output = run_cli(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn run_distinguishes_runtime_errors_tick_slices_and_work_yields() {
    let looping = TempFile::new("cg", b"~> >\n");
    let mut tick_args = configured_args(looping.text_path());
    tick_args[9] = "1";
    let tick = run_cli(&tick_args);
    assert_eq!(tick.status.code(), Some(6));
    let tick_result = parse_result(&tick);
    assert_eq!(tick_result["status"], "yielded");
    assert_eq!(tick_result["yield_reason"], "tick_slice_exhausted");

    let mut work_args = configured_args(looping.text_path());
    work_args[11] = "1";
    let work = run_cli(&work_args);
    assert_eq!(work.status.code(), Some(7));
    let work_result = parse_result(&work);
    assert_eq!(work_result["status"], "yielded");
    assert_eq!(work_result["yield_reason"], "work_unit_budget_exhausted");

    let bounded = TempFile::new("cg", b"~> >\n");
    let mut error_args = configured_args(bounded.text_path());
    error_args[3] = "exit";
    error_args[9] = "10";
    let error = run_cli(&error_args);
    assert_eq!(error.status.code(), Some(5));
    assert_eq!(parse_result(&error)["status"], "runtime_error");
    assert_eq!(
        parse_result(&error)["snapshot"]["errors"][0]["code"],
        "OutOfBounds"
    );
}

#[test]
fn run_rejects_noncanonical_and_invalid_host_inputs_without_json() {
    let source = TempFile::new("cg", b"~> ;\n");
    let mut args = configured_args(source.text_path());
    args[5] = "01";
    let leading_zero_seed = run_cli(&args);
    assert_eq!(leading_zero_seed.status.code(), Some(2));
    assert!(leading_zero_seed.stdout.is_empty());

    let invalid_input = TempFile::new("json", b"[1,1.0]");
    let mut args = configured_args(source.text_path());
    args.extend(["--input-file", invalid_input.text_path()]);
    let output = run_cli(&args);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let inline = configured_args(source.text_path());
    for invalid in [" ", "01", "1,,2", "256"] {
        let mut args = inline.clone();
        args.extend(["--input", invalid]);
        let output = run_cli(&args);
        assert_eq!(output.status.code(), Some(2), "{invalid:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn run_serialization_is_deterministic_and_emits_only_json_on_stdout() {
    let source = TempFile::new("cg", b"~> + . ;\n");
    let input = TempFile::new("json", b"[7]");
    let mut args = configured_args(source.text_path());
    args.extend(["--input-file", input.text_path()]);
    let first = run_cli(&args);
    let second = run_cli(&args);
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stdout.ends_with(b"\n"));
    assert_eq!(
        first.stdout.iter().filter(|byte| **byte == b'\n').count() > 1,
        true
    );
    let value = parse_result(&first);
    assert!(value["snapshot"]["runtime_program"]["main"]["cells"].is_array());
    assert!(value["events"].is_array());
    assert!(value["snapshot"]["metrics"]["used_cells"].is_array());
}

#[test]
fn evaluate_cli_result_matches_the_direct_shared_level_api() {
    let level_text = exact_io_level(&[]);
    let level = TempFile::new("json", level_text.as_bytes());
    let source = TempFile::new("cg", b"~> ;\n");
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );
    let args = evaluate_args(level.text_path(), source.text_path(), profile.text_path());
    let output = run_cli(&args);

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count() > 1,
        true,
        "JSON mode writes one complete pretty-printed response"
    );
    let cli_response = parse_result(&output);
    assert_eq!(cli_response["schema"], "codegrid.level.response");
    assert_eq!(cli_response["api_version"], 1);
    assert_eq!(cli_response["status"], "result");
    assert_eq!(cli_response["result"]["status"], "Passed");
    assert_eq!(
        cli_response["result"],
        direct_api_evaluate(&level_text, "~> ;\n", "Debug"),
        "CLI must preserve the shared Level API semantic result"
    );
    assert_eq!(
        cli_response["result"]["configuration"]["shuffle_seed"],
        "18446744073709551615"
    );
    assert_eq!(cli_response["result"]["rating"], Value::Null);
    assert_eq!(cli_response["result"]["scoring"], json!([]));

    let mut rated_value: Value = serde_json::from_str(&level_text).unwrap();
    rated_value["scoring"]["metrics"] = json!({"cost": {"target": 1}});
    let rated_level = TempFile::new("json", rated_value.to_string().as_bytes());
    let rated_args = evaluate_args(
        rated_level.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let rated = run_cli(&rated_args);
    assert_eq!(rated.status.code(), Some(0));
    let rated_result = parse_result(&rated);
    assert_eq!(rated_result["result"]["rating"], 3);
    assert_eq!(rated_result["result"]["scoring"][0]["name"], "cost");
    assert_eq!(rated_result["result"]["scoring"][0]["target"], "1");

    let human_args = [
        "evaluate",
        level.text_path(),
        source.text_path(),
        "--mode",
        "debug",
        "--boundary",
        "exit",
        "--seed",
        "0",
        "--custom-limit",
        "1000",
        "--limits-file",
        profile.text_path(),
        "--format",
        "human",
    ];
    let human = run_cli(&human_args);
    assert_eq!(human.status.code(), Some(0));
    let human_text = String::from_utf8_lossy(&human.stdout);
    assert!(human_text.contains("Evaluation: Passed"));
    assert!(!human_text.trim_start().starts_with('{'));
}

#[test]
fn evaluate_grouped_permissions_and_default_instructions() {
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );
    let cases = [
        (
            include_str!("../../../fixtures/levels/echo.json"),
            include_str!("../../../fixtures/levels/echo.cg"),
        ),
        (
            include_str!("../../../fixtures/levels/functions.json"),
            include_str!("../../../fixtures/levels/functions.cg"),
        ),
        (
            include_str!("../../../fixtures/levels/custom-memory.json"),
            include_str!("../../../fixtures/levels/custom-memory.cg"),
        ),
        (
            include_str!("../../../fixtures/levels/memory.json"),
            include_str!("../../../fixtures/levels/memory.cg"),
        ),
        (
            include_str!("../../../fixtures/levels/instruction-stack.json"),
            include_str!("../../../fixtures/levels/instruction-stack.cg"),
        ),
    ];
    for (level_text, source_text) in cases {
        let mut value: Value = serde_json::from_str(level_text).unwrap();
        value["program_rules"]["allowed_instructions"] = json!([
            "CMP",
            "READ",
            "REGISTER_POINTER",
            "STACK",
            "CODEC",
            "MEMORY",
            "PAGE",
            "SHIFT",
            "CALL",
            "CUSTOM",
            "CLEAR",
            "ADD",
            "SUB",
            "NAND",
            "FOLDED_BLOCK",
            "RANDOM_DIRECTION"
        ]);
        let level = TempFile::new("json", value.to_string().as_bytes());
        let source = TempFile::new("cg", source_text.as_bytes());
        let output = run_cli(&evaluate_args(
            level.text_path(),
            source.text_path(),
            profile.text_path(),
        ));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(parse_result(&output)["result"]["status"], "Passed");
    }
    let mut value: Value = serde_json::from_str(&exact_io_level(&[0])).unwrap();
    value["program_rules"]["allowed_instructions"] = json!([]);
    let level = TempFile::new("json", value.to_string().as_bytes());
    let source = TempFile::new("cg", b"~> > . ;\n");
    let output = run_cli(&evaluate_args(
        level.text_path(),
        source.text_path(),
        profile.text_path(),
    ));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(parse_result(&output)["result"]["status"], "Passed");
}

#[test]
fn evaluate_official_echo_u64_max_matches_the_direct_level_api() {
    let mut level_value: Value = serde_json::from_str(&exact_io_level(&[42])).unwrap();
    level_value["program_rules"]["allowed_instructions"] = json!(["HALT", "OUTPUT", "READ_RIGHT"]);
    level_value["evaluation"]["tests"][0]["input"] = json!([42]);
    let level_text = level_value.to_string();
    let level = TempFile::new("json", level_text.as_bytes());
    let source = TempFile::new("cg", b"~> ,> . ;\n");
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );
    let mut args = evaluate_args(level.text_path(), source.text_path(), profile.text_path());
    let mode_index = args.iter().position(|arg| *arg == "--mode").unwrap();
    args[mode_index + 1] = "official";
    let output = run_cli(&args);

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli_response = parse_result(&output);
    assert_eq!(cli_response["result"]["mode"], "Official");
    assert_eq!(cli_response["result"]["status"], "Passed");
    assert_eq!(
        cli_response["result"]["visible_tests"][0]["input"],
        json!([42])
    );
    assert_eq!(
        cli_response["result"]["visible_tests"][0]["actual_output"],
        json!([42])
    );
    assert_eq!(
        cli_response["result"],
        direct_api_evaluate(&level_text, "~> ,> . ;\n", "Official"),
        "CLI must preserve the complete shared result for the same official request"
    );
}

#[test]
fn evaluate_reports_source_and_level_rejections_as_complete_api_responses() {
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );

    let level = TempFile::new("json", exact_io_level(&[]).as_bytes());
    let invalid_source = TempFile::new("cg", b"~> [0\n");
    let source_args = evaluate_args(
        level.text_path(),
        invalid_source.text_path(),
        profile.text_path(),
    );
    let source_result = run_cli(&source_args);
    assert_eq!(source_result.status.code(), Some(4));
    let source_response = parse_result(&source_result);
    assert_eq!(source_response["status"], "source_rejected");
    assert!(source_response["diagnostics"].is_array());

    let malformed_level = TempFile::new("json", b"{}");
    let source = TempFile::new("cg", b"~> ;\n");
    let args = evaluate_args(
        malformed_level.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let malformed = run_cli(&args);
    assert_eq!(malformed.status.code(), Some(10));
    let malformed_response = parse_result(&malformed);
    assert_eq!(malformed_response["status"], "level_rejected");

    let malformed_json = TempFile::new("json", b"{");
    let args = evaluate_args(
        malformed_json.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let malformed_json_result = run_cli(&args);
    assert_eq!(malformed_json_result.status.code(), Some(10));
    assert_eq!(
        parse_result(&malformed_json_result)["status"],
        "level_rejected"
    );

    let mut unsupported_value: Value = serde_json::from_str(&exact_io_level(&[])).unwrap();
    unsupported_value["evaluation_type"] = json!("Environment");
    unsupported_value["evaluation"] = json!({
        "scene_type": "Elevator",
        "scene_data": {},
        "goals": {}
    });
    let unsupported_scene = TempFile::new("json", unsupported_value.to_string().as_bytes());
    let args = evaluate_args(
        unsupported_scene.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let unsupported = run_cli(&args);
    assert_eq!(unsupported.status.code(), Some(10));
    let unsupported_response = parse_result(&unsupported);
    assert_eq!(unsupported_response["status"], "level_rejected");
    assert_eq!(
        unsupported_response["error"]["category"],
        "UnsupportedSceneType"
    );
}

#[test]
fn evaluate_maps_player_rejection_failure_constraints_and_hidden_redaction() {
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );
    let level = TempFile::new("json", exact_io_level(&[]).as_bytes());
    let forbidden_source = TempFile::new("cg", b"~> + ;\n");
    let args = evaluate_args(
        level.text_path(),
        forbidden_source.text_path(),
        profile.text_path(),
    );
    let rejected = run_cli(&args);
    assert_eq!(rejected.status.code(), Some(9));
    assert_eq!(
        parse_result(&rejected)["result"]["status"],
        "ProgramRejected"
    );

    let mut generated_value: Value = serde_json::from_str(&exact_io_level(&[])).unwrap();
    generated_value["program_rules"]["allowed_instructions"] =
        json!(["READ_RIGHT", "DECODE", "MOVE_RIGHT", "HALT"]);
    generated_value["program_rules"]["allowed_attachments"] = json!(["WRITE_CODE"]);
    generated_value["evaluation"]["tests"][0]["input"] = json!([43]);
    let generated_level = TempFile::new("json", generated_value.to_string().as_bytes());
    let generated_source = TempFile::new("cg", b"@main\n~> ,> & >= ;\n@end main\n");
    let args = evaluate_args(
        generated_level.text_path(),
        generated_source.text_path(),
        profile.text_path(),
    );
    let generated = run_cli(&args);
    assert_eq!(
        generated.status.code(),
        Some(9),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let generated_result = parse_result(&generated);
    assert_eq!(generated_result["result"]["status"], "ProgramRejected");
    assert_eq!(
        generated_result["result"]["failure"]["reason"],
        "GeneratedInstructionNotAllowed"
    );

    // A generated direction is permitted by default even when not listed.
    generated_value["evaluation"]["tests"][0]["input"] = json!([94]);
    let default_generated_level = TempFile::new("json", generated_value.to_string().as_bytes());
    let allowed = run_cli(&evaluate_args(
        default_generated_level.text_path(),
        generated_source.text_path(),
        profile.text_path(),
    ));
    assert_eq!(allowed.status.code(), Some(0));
    assert_eq!(parse_result(&allowed)["result"]["status"], "Passed");

    let wrong_level = TempFile::new("json", exact_io_level(&[1]).as_bytes());
    let output_source = TempFile::new("cg", b"~> . ;\n");
    let args = evaluate_args(
        wrong_level.text_path(),
        output_source.text_path(),
        profile.text_path(),
    );
    let wrong = run_cli(&args);
    assert_eq!(wrong.status.code(), Some(9));
    assert_eq!(parse_result(&wrong)["result"]["status"], "TestFailed");

    let mut constrained_value: Value = serde_json::from_str(&exact_io_level(&[])).unwrap();
    constrained_value["constraints"]["max_ticks"] = json!(0);
    let constrained_level = TempFile::new("json", constrained_value.to_string().as_bytes());
    let halt_source = TempFile::new("cg", b"~> ;\n");
    let args = evaluate_args(
        constrained_level.text_path(),
        halt_source.text_path(),
        profile.text_path(),
    );
    let constrained = run_cli(&args);
    assert_eq!(constrained.status.code(), Some(9));
    assert_eq!(
        parse_result(&constrained)["result"]["status"],
        "ConstraintExceeded"
    );

    let mut private_level: Value = serde_json::from_str(&exact_io_level(&[])).unwrap();
    private_level["evaluation"]["tests"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "visible": false,
            "input": [77],
            "expected_output": [99]
        }));
    let private_level = TempFile::new("json", private_level.to_string().as_bytes());
    let mut args = evaluate_args(
        private_level.text_path(),
        halt_source.text_path(),
        profile.text_path(),
    );
    let mode_index = args.iter().position(|arg| *arg == "--mode").unwrap();
    args[mode_index + 1] = "official";
    let hidden_failure = run_cli(&args);
    assert_eq!(hidden_failure.status.code(), Some(9));
    let public_text = String::from_utf8_lossy(&hidden_failure.stdout);
    let hidden = parse_result(&hidden_failure);
    // Content digests may contain the same decimal substring as a private byte.
    // Check semantic disclosure fields rather than searching opaque hash text.
    assert!(hidden["result"]["visible_tests"]
        .as_array()
        .unwrap()
        .iter()
        .all(|test| {
            ["input", "expected_output", "actual_output"]
                .iter()
                .all(|field| {
                    !test[*field]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|byte| byte == &json!(77) || byte == &json!(99))
                })
        }));
    assert!(public_text.contains("HiddenTestFailed"));
}

#[test]
fn evaluate_rejects_bad_arguments_profiles_and_files_without_json() {
    let level = TempFile::new("json", exact_io_level(&[]).as_bytes());
    let source = TempFile::new("cg", b"~> ;\n");
    let profile = TempFile::new(
        "json",
        include_str!("../../../fixtures/levels/profiles/local-v1.json").as_bytes(),
    );

    let mut missing = evaluate_args(level.text_path(), source.text_path(), profile.text_path());
    missing.pop();
    let invalid_arguments = run_cli(&missing);
    assert_eq!(invalid_arguments.status.code(), Some(2));
    assert!(invalid_arguments.stdout.is_empty());

    let malformed_profile = TempFile::new("json", b"{}");
    let args = evaluate_args(
        level.text_path(),
        source.text_path(),
        malformed_profile.text_path(),
    );
    let invalid_profile = run_cli(&args);
    assert_eq!(invalid_profile.status.code(), Some(2));
    assert!(invalid_profile.stdout.is_empty());

    let missing_path =
        std::env::temp_dir().join(format!("codegrid-missing-{}.json", std::process::id()));
    let missing_path = missing_path.to_str().expect("temporary paths are UTF-8");
    let args = evaluate_args(missing_path, source.text_path(), profile.text_path());
    let missing_file = run_cli(&args);
    assert_eq!(missing_file.status.code(), Some(3));
    assert!(missing_file.stdout.is_empty());

    let invalid_utf8_level = TempFile::new("json", &[0xff, 0xfe]);
    let args = evaluate_args(
        invalid_utf8_level.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let invalid_utf8 = run_cli(&args);
    assert_eq!(invalid_utf8.status.code(), Some(3));
    assert!(invalid_utf8.stdout.is_empty());

    let bom_level = TempFile::new("json", b"\xef\xbb\xbf{}");
    let args = evaluate_args(
        bom_level.text_path(),
        source.text_path(),
        profile.text_path(),
    );
    let bom = run_cli(&args);
    assert_eq!(bom.status.code(), Some(3));
    assert!(bom.stdout.is_empty());
}

#[test]
fn evaluate_returns_resource_status_and_rejects_oversized_inputs() {
    let mut profile_value: Value = serde_json::from_str(include_str!(
        "../../../fixtures/levels/profiles/local-v1.json"
    ))
    .unwrap();
    profile_value["max_total_work"] = json!("1");
    let low_work_profile = TempFile::new("json", profile_value.to_string().as_bytes());
    let mut looping_level_value: Value = serde_json::from_str(&exact_io_level(&[])).unwrap();
    looping_level_value["program_rules"]["allowed_instructions"] =
        json!(["HALT", "OUTPUT", "MOVE_RIGHT"]);
    let level = TempFile::new("json", looping_level_value.to_string().as_bytes());
    let looping_source = TempFile::new("cg", b"~> >\n");
    let mut args = evaluate_args(
        level.text_path(),
        looping_source.text_path(),
        low_work_profile.text_path(),
    );
    let boundary_index = args.iter().position(|arg| *arg == "exit").unwrap();
    args[boundary_index] = "wrap";
    let resource = run_cli(&args);
    assert_eq!(resource.status.code(), Some(11));
    assert_eq!(
        parse_result(&resource)["result"]["status"],
        "ResourceLimitExceeded"
    );

    profile_value["max_level_bytes"] = json!("8");
    let tiny_profile = TempFile::new("json", profile_value.to_string().as_bytes());
    let args = evaluate_args(
        level.text_path(),
        looping_source.text_path(),
        tiny_profile.text_path(),
    );
    let too_large = run_cli(&args);
    assert_eq!(too_large.status.code(), Some(11));
    assert!(too_large.stdout.is_empty());
}

#[test]
fn evaluate_response_ceiling_returns_resource_exit_code() {
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../../fixtures/levels/profiles/local-v1.json"
    ))
    .unwrap();
    profile["max_response_bytes"] = json!("512");
    let profile = TempFile::new("json", profile.to_string().as_bytes());
    let level = TempFile::new("json", include_bytes!("../../../fixtures/levels/echo.json"));
    let source = TempFile::new("cg", include_bytes!("../../../fixtures/levels/echo.cg"));
    let output = run_cli(&evaluate_args(
        level.text_path(),
        source.text_path(),
        profile.text_path(),
    ));
    assert_eq!(output.status.code(), Some(11));
    assert_eq!(
        parse_result(&output)["error"]["code"],
        "level_api.response_too_large"
    );
}
