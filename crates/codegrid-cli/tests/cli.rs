use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

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
}

#[test]
fn native_host_failures_and_debug_errors_have_stable_identifiers() {
    use std::process::Stdio;
    let bad_args = run_cli(&["run"]);
    assert!(String::from_utf8_lossy(&bad_args.stderr).contains("[cli.invalid_arguments]"));
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
    assert_eq!(responses[1]["error"]["code"], "debug.invalid_request");
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
