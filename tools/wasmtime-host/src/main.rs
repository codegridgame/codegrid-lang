use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use wasmtime::error::Context as _;
use wasmtime::{
    bail, ensure, Config, Engine, ExternType, Instance, Memory, Module, Result, Store, StoreLimits,
    StoreLimitsBuilder, Trap, TypedFunc,
};

const WASM_PAGE_BYTES: u64 = 65_536;
const ABI_VERSION: u32 = 4;
const API_VERSION: u32 = 3;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_FUEL_PER_CALL: u64 = 1_000_000_000;
const ABI_HELPER_FUEL: u64 = 1_000_000;
const WASM_STACK_LIMIT_BYTES: usize = 2 * 1024 * 1024;
const MEMORY_LIMIT_PROBE_BYTES: u64 = 1 * 1024 * 1024;
const LOW_RUN_FUEL: u64 = 100_000;
const DEFAULT_WORK_UNITS_PER_CALL: u64 = 1_000_000;

struct HostState {
    limits: StoreLimits,
}

struct WasmtimeSession {
    store: Store<HostState>,
    memory: Memory,
    abi_version: TypedFunc<(), u32>,
    alloc_buffer: TypedFunc<u32, u32>,
    free_buffer: TypedFunc<u32, u32>,
    process_request: TypedFunc<(u32, u32), u64>,
    fuel_per_call: u64,
}

impl WasmtimeSession {
    fn instantiate(engine: &Engine, module: &Module, memory_limit_bytes: u64) -> Result<Self> {
        let memory_limit = usize::try_from(memory_limit_bytes)
            .context("linear-memory ceiling does not fit this host's usize")?;
        let mut store = Store::new(
            engine,
            HostState {
                limits: StoreLimitsBuilder::new()
                    .memory_size(memory_limit)
                    .instances(1)
                    .memories(1)
                    .tables(1)
                    .table_elements(4096)
                    .build(),
            },
        );
        store.limiter(|state| &mut state.limits);
        store.set_fuel(DEFAULT_FUEL_PER_CALL)?;
        let instance = Instance::new(&mut store, module, &[])
            .context("instantiate server module with no imports")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("server module did not export memory")?;
        let abi_version = instance
            .get_typed_func::<(), u32>(&mut store, "codegrid_server_abi_version")
            .context("server ABI version export has the wrong signature")?;
        let alloc_buffer = instance
            .get_typed_func::<u32, u32>(&mut store, "codegrid_server_alloc_buffer")
            .context("server buffer allocator export has the wrong signature")?;
        let free_buffer = instance
            .get_typed_func::<u32, u32>(&mut store, "codegrid_server_free_buffer")
            .context("server buffer release export has the wrong signature")?;
        let process_request = instance
            .get_typed_func::<(u32, u32), u64>(&mut store, "codegrid_server_process_request")
            .context("server request processor export has the wrong signature")?;

        let mut session = Self {
            store,
            memory,
            abi_version,
            alloc_buffer,
            free_buffer,
            process_request,
            fuel_per_call: DEFAULT_FUEL_PER_CALL,
        };
        let actual_version = session.call_abi_version()?;
        ensure!(
            actual_version == ABI_VERSION,
            "expected server ABI v{ABI_VERSION}, got v{actual_version}"
        );
        Ok(session)
    }

    fn call_abi_version(&mut self) -> Result<u32> {
        self.store.set_fuel(ABI_HELPER_FUEL)?;
        Ok(self.abi_version.call(&mut self.store, ())?)
    }

    fn call_alloc(&mut self, length: u32) -> Result<u32> {
        self.store.set_fuel(ABI_HELPER_FUEL)?;
        Ok(self.alloc_buffer.call(&mut self.store, length)?)
    }

    fn call_free(&mut self, pointer: u32) -> Result<u32> {
        self.store.set_fuel(ABI_HELPER_FUEL)?;
        Ok(self.free_buffer.call(&mut self.store, pointer)?)
    }

    fn dispatch(&mut self, request: Value) -> Result<Value> {
        self.dispatch_with_fuel(request, self.fuel_per_call)
    }

    fn dispatch_with_fuel(&mut self, request: Value, process_fuel: u64) -> Result<Value> {
        let request_bytes = serde_json::to_vec(&request).context("serialize ABI request")?;
        let request_length = u32::try_from(request_bytes.len())
            .context("serialized ABI request exceeds the u32 length field")?;
        let request_pointer = self.call_alloc(request_length)?;
        ensure!(
            request_pointer != 0,
            "server ABI rejected a valid request buffer"
        );
        self.memory
            .write(&mut self.store, request_pointer as usize, &request_bytes)
            .context("write request bytes to guest memory")?;

        self.store.set_fuel(process_fuel)?;
        let packed_response = self
            .process_request
            .call(&mut self.store, (request_pointer, request_length))
            .context("call codegrid_server_process_request")?;
        ensure!(
            packed_response != 0,
            "server ABI failed to allocate a response buffer"
        );
        let response_pointer = (packed_response >> 32) as u32;
        let response_length = packed_response as u32;
        ensure!(
            response_pointer != 0,
            "server ABI returned a null response pointer"
        );
        ensure!(
            response_length != 0,
            "server ABI returned an empty response buffer"
        );
        let response_length = response_length as usize;
        ensure!(
            response_length <= MAX_RESPONSE_BYTES,
            "server ABI returned {response_length} response bytes, above the 8 MiB harness ceiling"
        );
        let response_end = (response_pointer as usize)
            .checked_add(response_length)
            .context("response pointer range overflowed the host address width")?;
        ensure!(
            response_end <= self.memory.data_size(&self.store),
            "server ABI response pointer range exceeds current linear memory"
        );
        let mut response_bytes = vec![0; response_length];
        self.memory
            .read(&self.store, response_pointer as usize, &mut response_bytes)
            .context("copy response bytes from guest memory")?;
        let response =
            serde_json::from_slice(&response_bytes).context("parse ABI response JSON")?;

        ensure!(
            self.call_free(response_pointer)? == 1,
            "server ABI did not release the response buffer"
        );
        ensure!(
            self.call_free(request_pointer)? == 1,
            "server ABI did not release the request buffer"
        );
        Ok(response)
    }
}

fn main() -> Result<()> {
    let wasm_path = wasm_artifact_path()?;
    let memory_limit_bytes = configured_memory_limit()?;
    let wasm_bytes = fs::read(&wasm_path)
        .with_context(|| format!("read server WASM artifact {}", wasm_path.display()))?;

    let mut config = Config::new();
    config
        .consume_fuel(true)
        .max_wasm_stack(WASM_STACK_LIMIT_BYTES);
    let engine = Engine::new(&config).context("create fuel-metered Wasmtime engine")?;
    let module = Module::new(&engine, &wasm_bytes).context("compile server WASM module")?;

    verify_module_shape(&module, memory_limit_bytes)?;
    verify_store_memory_limit(&engine, &module, memory_limit_bytes)?;

    let root = repository_root();
    let cli_path = cli_binary_path(&root)?;
    let suite: Value = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/conformance-v1.json"))
            .context("read shared Full conformance fixture")?,
    )
    .context("parse shared Full conformance fixture")?;
    ensure!(
        suite["schema_version"] == 1,
        "unsupported shared fixture schema version"
    );
    let cases = suite["cases"]
        .as_array()
        .context("conformance fixture cases must be an array")?;
    ensure!(
        cases.len() == 75,
        "shared Full fixture must contain all 75 cases, found {}",
        cases.len()
    );

    let mut session = WasmtimeSession::instantiate(&engine, &module, memory_limit_bytes)?;
    let initialized = session.dispatch(initialize_request(DEFAULT_WORK_UNITS_PER_CALL))?;
    ensure!(
        initialized["status"] == "initialized",
        "server session failed initialization"
    );

    let mut baseline_cases = Vec::with_capacity(cases.len());
    for fixture in cases {
        let result = run_conformance_fixture(
            &mut session,
            &cli_path,
            fixture,
            DEFAULT_WORK_UNITS_PER_CALL,
        )?;
        baseline_cases.push(json!({
            "id": fixture["id"],
            "result": result,
        }));
    }
    let baseline_path =
        target_directory(&root).join("wasmtime-host/native-cli-full-baseline-v1.json");
    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "create native full-result baseline directory {}",
                parent.display()
            )
        })?;
    }
    let baseline = json!({
        "schema": "codegrid.native-cli.full-run-baseline",
        "schema_version": 1,
        "suite_id": suite["suite_id"],
        "api_version": API_VERSION,
        "max_work_units_per_call": DEFAULT_WORK_UNITS_PER_CALL.to_string(),
        "cases": baseline_cases,
    });
    fs::write(&baseline_path, serde_json::to_vec_pretty(&baseline)?)
        .with_context(|| format!("write full Native CLI baseline {}", baseline_path.display()))?;
    let shutdown = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "shutdown",
    }))?;
    ensure!(
        shutdown["status"] == "closed",
        "server session did not shut down"
    );
    drop(session);

    let loop_fixture = cases
        .iter()
        .find(|fixture| fixture["id"] == "wrap-boundary-yields-after-bounded-run")
        .context("Full fixture is missing its deterministic looping case")?;
    verify_work_limit_yield(
        &engine,
        &module,
        memory_limit_bytes,
        &cli_path,
        loop_fixture,
    )?;
    verify_low_fuel_discards_session(&engine, &module, memory_limit_bytes, loop_fixture)?;

    println!(
        "PASS: Wasmtime 49.0.1 server ABI v4 / Runtime API v3, all {} Full fixtures with complete native CLI parity, work-limit yielding, memory/stack limits, and low-fuel session discard. Native full-result baseline: {}",
        cases.len(),
        baseline_path.display()
    );
    Ok(())
}

fn verify_module_shape(module: &Module, expected_memory_bytes: u64) -> Result<()> {
    ensure!(
        module.imports().next().is_none(),
        "server module must not import host functions or capabilities"
    );
    let exports: Vec<_> = module.exports().collect();
    let export_names: Vec<_> = exports.iter().map(|export| export.name()).collect();
    for name in [
        "memory",
        "codegrid_server_abi_version",
        "codegrid_server_alloc_buffer",
        "codegrid_server_free_buffer",
        "codegrid_server_process_request",
    ] {
        ensure!(
            export_names.contains(&name),
            "server module is missing export `{name}`"
        );
    }

    let memories: Vec<_> = exports
        .iter()
        .filter_map(|export| match export.ty() {
            ExternType::Memory(memory_type) => Some((export.name(), memory_type)),
            _ => None,
        })
        .collect();
    ensure!(
        memories.len() == 1,
        "server module must define and export exactly one memory"
    );
    ensure!(
        memories[0].0 == "memory",
        "the server linear memory must be named `memory`"
    );
    let memory_type = &memories[0].1;
    let expected_pages = expected_memory_bytes / WASM_PAGE_BYTES;
    ensure!(
        expected_memory_bytes % WASM_PAGE_BYTES == 0,
        "configured memory ceiling must be WebAssembly-page aligned"
    );
    ensure!(
        memory_type.maximum() == Some(expected_pages),
        "module maximum is {:?} pages; configured build maximum is {expected_pages} pages",
        memory_type.maximum()
    );
    ensure!(
        memory_type.minimum() <= expected_pages,
        "module minimum exceeds its configured maximum"
    );
    Ok(())
}

fn verify_store_memory_limit(
    engine: &Engine,
    module: &Module,
    module_limit_bytes: u64,
) -> Result<()> {
    let memory_type = module
        .exports()
        .find_map(|export| match export.ty() {
            ExternType::Memory(memory_type) if export.name() == "memory" => Some(memory_type),
            _ => None,
        })
        .context("server module is missing its exported memory type")?;
    let minimum_bytes = memory_type.minimum() * WASM_PAGE_BYTES;
    let probe_limit_bytes = MEMORY_LIMIT_PROBE_BYTES.max(
        minimum_bytes
            .checked_add(WASM_PAGE_BYTES)
            .context("module minimum memory size overflowed")?,
    );
    ensure!(
        probe_limit_bytes < module_limit_bytes,
        "module maximum leaves no room to verify a distinct StoreLimits ceiling"
    );
    let mut session = WasmtimeSession::instantiate(engine, module, probe_limit_bytes)
        .context("instantiate disposable memory-limit probe")?;
    let initial_pages = session.memory.size(&session.store);
    let initial_bytes = initial_pages * WASM_PAGE_BYTES;
    ensure!(
        initial_bytes < probe_limit_bytes,
        "module initial memory leaves no room to exercise StoreLimits"
    );
    let probe_pages = probe_limit_bytes / WASM_PAGE_BYTES;
    let pages_to_limit = probe_pages - initial_pages;
    ensure!(
        pages_to_limit > 0,
        "memory-limit probe must grow at least one page"
    );
    session
        .memory
        .grow(&mut session.store, pages_to_limit)
        .context("grow probe memory up to the StoreLimits ceiling")?;
    ensure!(
        session.memory.data_size(&session.store) == probe_limit_bytes as usize,
        "probe memory did not grow to the configured StoreLimits ceiling"
    );
    ensure!(
        session.memory.grow(&mut session.store, 1).is_err(),
        "StoreLimits allowed probe memory to exceed its configured ceiling"
    );
    Ok(())
}

fn run_conformance_fixture(
    session: &mut WasmtimeSession,
    cli_path: &Path,
    fixture: &Value,
    work_limit: u64,
) -> Result<Value> {
    let fixture_id = fixture["id"]
        .as_str()
        .context("Full fixture is missing its id")?;
    let source = fixture["source"]
        .as_str()
        .with_context(|| format!("fixture `{fixture_id}` is missing inline source"))?;
    let run = &fixture["run"];

    let compilation = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "compile",
        "source": source,
    }))?;
    ensure!(
        compilation["status"] == "compiled",
        "fixture `{fixture_id}` did not compile: {compilation}"
    );
    let program = compilation["program"]
        .as_str()
        .with_context(|| format!("fixture `{fixture_id}` returned no program handle"))?;

    let created = session.dispatch(create_instance_request(program, run))?;
    ensure!(
        created["operation"] == "create_instance",
        "fixture `{fixture_id}` failed to create an instance: {created}"
    );
    let instance = created["instance"]
        .as_str()
        .with_context(|| format!("fixture `{fixture_id}` returned no instance handle"))?;

    let result = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "run",
        "instance": instance,
        "max_ticks": run["max_ticks"],
    }))?;
    ensure!(
        result["abi_version"] == ABI_VERSION
            && result["api_version"] == API_VERSION
            && result["operation"] == "run",
        "fixture `{fixture_id}` returned the wrong ABI/API operation: {result}"
    );

    let native = run_native_cli(cli_path, fixture, work_limit)?;
    compare_fixture_expectations(fixture_id, fixture, &result, &native, work_limit)?;
    compare_complete_results(fixture_id, &result, &native)?;
    let normalized_native = normalize_native_result(&native.json)?;

    let released_instance = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "release_instance",
        "instance": instance,
    }))?;
    ensure!(
        released_instance["operation"] == "release_instance",
        "fixture `{fixture_id}` failed to release its instance: {released_instance}"
    );
    let released_program = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "release_program",
        "program": program,
    }))?;
    ensure!(
        released_program["operation"] == "release_program",
        "fixture `{fixture_id}` failed to release its program: {released_program}"
    );
    Ok(normalized_native)
}

fn create_instance_request(program: &str, run: &Value) -> Value {
    json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "create_instance",
        "program": program,
        "input": run["input"],
        "initial_memory": run.get("initial_memory").cloned().unwrap_or_else(|| json!([])),
        "boundary_mode": run["boundary"],
        "seed": run["seed"],
        "custom_execution_limit": run["custom_execution_limit"],
    })
}

fn compare_fixture_expectations(
    fixture_id: &str,
    fixture: &Value,
    server: &Value,
    native: &NativeCliResult,
    work_limit: u64,
) -> Result<()> {
    let expected = &fixture["expected"];
    let snapshot = &server["snapshot"];
    ensure!(
        native.exit_code == expected["process_exit_code"].as_i64().unwrap_or(-1),
        "fixture `{fixture_id}` native CLI exit code mismatch: expected {}, got {}",
        expected["process_exit_code"],
        native.exit_code
    );
    ensure!(
        server["status"] == expected["status"],
        "fixture `{fixture_id}` run status mismatch: expected {}, got {}",
        expected["status"],
        server["status"]
    );
    ensure!(
        snapshot["status"] == expected["vm_status"],
        "fixture `{fixture_id}` VM status mismatch: expected {}, got {}",
        expected["vm_status"],
        snapshot["status"]
    );
    ensure!(
        snapshot["registers"] == expected["registers"]
            && snapshot["output"] == expected["output"]
            && snapshot["committed_ticks"] == expected["committed_ticks"],
        "fixture `{fixture_id}` register/output/tick expectations do not match the server result"
    );
    for metric in [
        "operation_count",
        "used_cell_count",
        "used_memory_address_count",
        "peak_data_stack_usage",
        "peak_instruction_stack_usage",
        "peak_call_stack_usage",
    ] {
        ensure!(
            snapshot["metrics"][metric] == expected[metric],
            "fixture `{fixture_id}` metric `{metric}` mismatch: expected {}, got {}",
            expected[metric],
            snapshot["metrics"][metric]
        );
    }
    ensure!(
        snapshot["metrics"]["instruction_variety"] == expected["instruction_variety"],
        "fixture `{fixture_id}` instruction variety mismatch"
    );
    let actual_error_codes = snapshot["errors"]
        .as_array()
        .context("server snapshot errors must be an array")?
        .iter()
        .map(|error| error["code"].clone())
        .collect::<Vec<_>>();
    let expected_error_codes = expected["error_codes"]
        .as_array()
        .context("fixture error_codes must be an array")?;
    ensure!(
        actual_error_codes.as_slice() == expected_error_codes.as_slice(),
        "fixture `{fixture_id}` ordered error codes mismatch: expected {}, got {}",
        expected["error_codes"],
        json!(actual_error_codes)
    );
    ensure!(
        snapshot["fault"].is_null(),
        "fixture `{fixture_id}` unexpectedly returned a VM fault: {}",
        snapshot["fault"]
    );
    ensure!(
        server["events"].is_array() && server["newly_emitted_output"].is_array(),
        "fixture `{fixture_id}` run response must contain event and output deltas"
    );
    ensure!(
        snapshot["metrics"]["global_tick"] == snapshot["committed_ticks"],
        "fixture `{fixture_id}` metric global_tick must match the snapshot tick"
    );

    let run = &fixture["run"];
    let config = &native.json["configuration"];
    for (field, expected_value) in [
        ("boundary_mode", run["boundary"].clone()),
        ("seed", run["seed"].clone()),
        (
            "custom_execution_limit",
            run["custom_execution_limit"].clone(),
        ),
        ("max_ticks", run["max_ticks"].clone()),
        ("max_work_units", json!(work_limit.to_string())),
        ("input", run["input"].clone()),
    ] {
        ensure!(
            config[field] == expected_value,
            "fixture `{fixture_id}` native CLI did not use expected {field}: expected {expected_value}, got {}",
            config[field]
        );
    }

    let expected_memory = run
        .get("initial_memory")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry["value"].as_u64() != Some(0))
        .collect::<Vec<_>>();
    ensure!(
        config["initial_memory"] == json!(expected_memory),
        "fixture `{fixture_id}` native CLI did not use expected initial memory"
    );
    assert_optional_fixture_expectations(fixture_id, expected, snapshot)
}

fn assert_optional_fixture_expectations(
    fixture_id: &str,
    expected: &Value,
    snapshot: &Value,
) -> Result<()> {
    let threads = snapshot["threads"]
        .as_array()
        .context("server snapshot threads must be an array")?;
    let errors = snapshot["errors"]
        .as_array()
        .context("server snapshot errors must be an array")?;
    let runtime_main = &snapshot["runtime_program"]["main"]["cells"];
    let actual = json!({
        "error_custom_internal_ticks": errors.iter()
            .filter(|error| error["scope"]["kind"] == "custom")
            .map(|error| error["scope"]["internal_tick"].clone()).collect::<Vec<_>>(),
        "error_global_ticks": errors.iter()
            .map(|error| error["global_tick"].clone()).collect::<Vec<_>>(),
        "error_thread_id_groups": errors.iter()
            .map(|error| error["details"].get("thread_ids")
                .or_else(|| error["details"].get("internal_thread_ids")))
            .filter_map(|ids| ids.cloned()).collect::<Vec<_>>(),
        "error_thread_ids": errors.iter()
            .filter_map(|error| error["details"].get("thread_id"))
            .cloned().collect::<Vec<_>>(),
        "memory": snapshot["memory"],
        "remaining_input": snapshot["remaining_input"],
        "runtime_main_attachment_tokens": runtime_main.as_array().map(|cells| cells.iter()
            .map(|cell| cell["attachment"].clone()).collect::<Vec<_>>()),
        "runtime_main_primary_tokens": runtime_main.as_array().map(|cells| cells.iter()
            .map(|cell| cell["primary"].clone()).collect::<Vec<_>>()),
        "thread_call_stack_sizes": threads.iter()
            .map(|thread| thread["call_frames"].as_array().map(Vec::len)).collect::<Vec<_>>(),
        "thread_data_stack_sizes": threads.iter()
            .map(|thread| thread["data_stack"].as_array().map(Vec::len)).collect::<Vec<_>>(),
        "thread_direction": threads.first().map(|thread| thread["direction"].clone()),
        "thread_instruction_stack_sizes": threads.iter()
            .map(|thread| thread["instruction_stack"].as_array().map(Vec::len)).collect::<Vec<_>>(),
        "thread_random_state": threads.first().map(|thread| thread["random_state"].clone()),
        "threads": threads.iter().map(|thread| json!({
            "id": thread["id"],
            "position": thread["position"],
            "direction": thread["direction"],
        })).collect::<Vec<_>>(),
    });
    let expected_object = expected
        .as_object()
        .context("fixture expected result must be an object")?;
    for (field, value) in actual
        .as_object()
        .context("derived fixture expectations must be an object")?
    {
        if let Some(expected_value) = expected_object.get(field) {
            ensure!(
                value == expected_value,
                "fixture `{fixture_id}` optional field `{field}` mismatch: expected {expected_value}, got {value}"
            );
        }
    }
    Ok(())
}

struct NativeCliResult {
    exit_code: i64,
    json: Value,
}

struct FixtureTempDir(PathBuf);

impl FixtureTempDir {
    fn create(fixture_id: &str) -> Result<Self> {
        let safe_id = fixture_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>();
        for attempt in 0..1000 {
            let path = env::temp_dir().join(format!(
                "codegrid-wasmtime-{}-{safe_id}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("create temporary CLI fixture directory {}", path.display())
                    })
                }
            }
        }
        bail!("could not allocate a temporary directory for fixture `{fixture_id}`")
    }
}

impl Drop for FixtureTempDir {
    fn drop(&mut self) {
        for file in ["program.cg", "initial-memory.json"] {
            let _ = fs::remove_file(self.0.join(file));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

fn run_native_cli(cli_path: &Path, fixture: &Value, work_limit: u64) -> Result<NativeCliResult> {
    let fixture_id = fixture["id"]
        .as_str()
        .context("Full fixture is missing its id")?;
    let source = fixture["source"]
        .as_str()
        .with_context(|| format!("fixture `{fixture_id}` is missing inline source"))?;
    let run = &fixture["run"];
    let input = run["input"]
        .as_array()
        .with_context(|| format!("fixture `{fixture_id}` input must be an array"))?
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|value| *value <= u8::MAX as u64)
                .map(|value| value.to_string())
                .with_context(|| format!("fixture `{fixture_id}` has an invalid input byte"))
        })
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let initial_memory = run
        .get("initial_memory")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let temp_dir = FixtureTempDir::create(fixture_id)?;
    let source_path = temp_dir.0.join("program.cg");
    let memory_path = temp_dir.0.join("initial-memory.json");
    fs::write(&source_path, source)
        .with_context(|| format!("write temporary source for fixture `{fixture_id}`"))?;
    fs::write(&memory_path, serde_json::to_vec(&initial_memory)?)
        .with_context(|| format!("write initial memory for fixture `{fixture_id}`"))?;

    let output = Command::new(cli_path)
        .arg("run")
        .arg(&source_path)
        .arg("--boundary")
        .arg(
            run["boundary"]
                .as_str()
                .context("fixture boundary must be a string")?,
        )
        .arg("--seed")
        .arg(
            run["seed"]
                .as_str()
                .context("fixture seed must be a decimal string")?,
        )
        .arg("--custom-limit")
        .arg(
            run["custom_execution_limit"]
                .as_str()
                .context("fixture custom limit must be a decimal string")?,
        )
        .arg("--max-ticks")
        .arg(
            run["max_ticks"]
                .as_str()
                .context("fixture tick limit must be a decimal string")?,
        )
        .arg("--max-work-units")
        .arg(work_limit.to_string())
        .arg("--input")
        .arg(input)
        .arg("--initial-memory-file")
        .arg(&memory_path)
        .output()
        .with_context(|| format!("run native CLI for fixture `{fixture_id}`"))?;
    let exit_code = output
        .status
        .code()
        .context("native CLI terminated without an exit code")?;
    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("native CLI stdout for `{fixture_id}` was not UTF-8"))?;
    let json = serde_json::from_str(&stdout)
        .with_context(|| format!("parse complete native CLI JSON for `{fixture_id}`"))?;
    Ok(NativeCliResult {
        exit_code: i64::from(exit_code),
        json,
    })
}

fn compare_complete_results(
    fixture_id: &str,
    server: &Value,
    native: &NativeCliResult,
) -> Result<()> {
    let expected = normalize_native_result(&native.json)?;
    let actual = normalize_server_result(server)?;
    if let Some(difference) = first_json_difference(&expected, &actual, "$run") {
        bail!(
            "fixture `{fixture_id}` full Native CLI / Runtime API result mismatch at {difference}"
        );
    }
    Ok(())
}

fn normalize_native_result(result: &Value) -> Result<Value> {
    assert_exact_keys(
        result,
        &[
            "configuration",
            "diagnostics",
            "events",
            "newly_emitted_output",
            "schema",
            "schema_version",
            "snapshot",
            "status",
            "yield_reason",
        ],
        "native CLI result",
    )?;
    ensure!(
        result["schema"] == "codegrid.cli.run-result" && result["schema_version"] == 1,
        "native CLI returned an unsupported result schema"
    );
    ensure!(
        result["diagnostics"] == json!([]),
        "native CLI fixture run unexpectedly returned source diagnostics"
    );
    let status = match result["status"].as_str() {
        Some("halted") => "halted",
        Some("runtime_error") => "error",
        Some("vm_fault") => "vm_fault",
        Some("yielded") if result["yield_reason"] == "tick_slice_exhausted" => "tick_limit_reached",
        Some("yielded") if result["yield_reason"] == "work_unit_budget_exhausted" => {
            "work_limit_reached"
        }
        other => bail!("native CLI returned unsupported run status {other:?}"),
    };
    Ok(json!({
        "status": status,
        "events": normalize_events(&result["events"], ResultShape::NativeCli)?,
        "newly_emitted_output": result["newly_emitted_output"],
        "snapshot": normalize_snapshot(&result["snapshot"], ResultShape::NativeCli)?,
    }))
}

fn normalize_server_result(result: &Value) -> Result<Value> {
    assert_exact_keys(
        result,
        &[
            "abi_version",
            "api_version",
            "events",
            "newly_emitted_output",
            "operation",
            "snapshot",
            "status",
        ],
        "server ABI run response",
    )?;
    ensure!(
        result["abi_version"] == ABI_VERSION
            && result["api_version"] == API_VERSION
            && result["operation"] == "run",
        "server run response has a mismatched ABI/API version or operation"
    );
    let status = result["status"]
        .as_str()
        .context("server run status must be a string")?;
    ensure!(
        [
            "halted",
            "error",
            "tick_limit_reached",
            "work_limit_reached"
        ]
        .contains(&status),
        "server returned an unsupported run status `{status}`"
    );
    Ok(json!({
        "status": status,
        "events": normalize_events(&result["events"], ResultShape::ServerApi)?,
        "newly_emitted_output": result["newly_emitted_output"],
        "snapshot": normalize_snapshot(&result["snapshot"], ResultShape::ServerApi)?,
    }))
}

#[derive(Clone, Copy)]
enum ResultShape {
    NativeCli,
    ServerApi,
}

fn normalize_snapshot(snapshot: &Value, shape: ResultShape) -> Result<Value> {
    assert_exact_keys(
        snapshot,
        &[
            "committed_ticks",
            "errors",
            "fault",
            "memory",
            "metrics",
            "output",
            "registers",
            "remaining_input",
            "runtime_program",
            "status",
            "threads",
        ],
        "full VM snapshot",
    )?;
    let threads = snapshot["threads"]
        .as_array()
        .context("snapshot threads must be an array")?
        .iter()
        .map(|thread| normalize_thread(thread, shape, true))
        .collect::<Result<Vec<_>>>()?;
    let memory = snapshot["memory"]
        .as_array()
        .context("snapshot memory must be an array")?;
    for entry in memory {
        assert_exact_keys(entry, &["address", "value"], "snapshot memory entry")?;
    }
    let errors = snapshot["errors"]
        .as_array()
        .context("snapshot errors must be an array")?
        .iter()
        .map(|error| normalize_error(error, shape))
        .collect::<Result<Vec<_>>>()?;
    let metrics = normalize_metrics(&snapshot["metrics"], shape)?;
    Ok(json!({
        "status": snapshot["status"],
        "committed_ticks": snapshot["committed_ticks"],
        "registers": snapshot["registers"],
        "memory": memory,
        "remaining_input": snapshot["remaining_input"],
        "output": snapshot["output"],
        "runtime_program": normalize_runtime_program(&snapshot["runtime_program"], shape)?,
        "threads": threads,
        "metrics": metrics,
        "errors": errors,
        "fault": snapshot["fault"],
    }))
}

fn normalize_runtime_program(program: &Value, shape: ResultShape) -> Result<Value> {
    assert_exact_keys(program, &["functions", "main"], "runtime Outer program")?;
    let functions = program["functions"]
        .as_object()
        .context("runtime program functions must be an object")?;
    let mut normalized_functions = serde_json::Map::new();
    for (id, board) in functions {
        normalized_functions.insert(id.clone(), normalize_runtime_board(board, shape)?);
    }
    Ok(json!({
        "main": normalize_runtime_board(&program["main"], shape)?,
        "functions": normalized_functions,
    }))
}

fn normalize_runtime_board(board: &Value, shape: ResultShape) -> Result<Value> {
    assert_exact_keys(
        board,
        &["cells", "folded_blocks", "height", "width"],
        "runtime board",
    )?;
    let cells = board["cells"]
        .as_array()
        .context("runtime board cells must be an array")?
        .iter()
        .map(|cell| {
            assert_exact_keys(cell, &["attachment", "entry", "prefix", "primary"], "runtime cell")?;
            let entry = match cell["entry"].as_str() {
                Some(entry) if matches!(shape, ResultShape::ServerApi) => {
                    entry.strip_prefix('~').with_context(|| {
                        format!("API entry token `{entry}` must include its source marker")
                    })?
                }
                Some(entry) => entry,
                None => "",
            };
            Ok(json!({
                "entry": if entry.is_empty() { Value::Null } else { json!(entry) },
                "primary": cell["primary"],
                "prefix": cell["prefix"],
                "attachment": cell["attachment"],
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "width": board["width"],
        "height": board["height"],
        "cells": cells,
        "folded_blocks": board["folded_blocks"],
    }))
}

fn normalize_metrics(metrics: &Value, shape: ResultShape) -> Result<Value> {
    assert_exact_keys(
        metrics,
        &[
            "global_tick",
            "instruction_variety",
            "operation_count",
            "peak_call_stack_usage",
            "peak_data_stack_usage",
            "peak_instruction_stack_usage",
            "used_cell_count",
            "used_cells",
            "used_memory_address_count",
            "used_memory_addresses",
        ],
        "full runtime metrics",
    )?;
    let used_cells = metrics["used_cells"]
        .as_array()
        .context("metrics used_cells must be an array")?
        .iter()
        .map(normalize_static_cell)
        .collect::<Result<Vec<_>>>()?;
    let used_memory_addresses = metrics["used_memory_addresses"]
        .as_array()
        .context("metrics used_memory_addresses must be an array")?
        .iter()
        .map(normalize_memory_location)
        .collect::<Result<Vec<_>>>()?;
    let _ = shape;
    Ok(json!({
        "global_tick": metrics["global_tick"],
        "operation_count": metrics["operation_count"],
        "used_cell_count": metrics["used_cell_count"],
        "used_cells": used_cells,
        "used_memory_address_count": metrics["used_memory_address_count"],
        "used_memory_addresses": used_memory_addresses,
        "peak_data_stack_usage": metrics["peak_data_stack_usage"],
        "peak_instruction_stack_usage": metrics["peak_instruction_stack_usage"],
        "peak_call_stack_usage": metrics["peak_call_stack_usage"],
        "instruction_variety": metrics["instruction_variety"],
    }))
}

fn normalize_events(events: &Value, shape: ResultShape) -> Result<Value> {
    let events = events.as_array().context("run events must be an array")?;
    events
        .iter()
        .map(|event| normalize_event(event, shape))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn normalize_event(event: &Value, shape: ResultShape) -> Result<Value> {
    let kind = event["kind"]
        .as_str()
        .context("event kind must be a string")?;
    let scope = event.get("scope").context("event is missing scope")?;
    let mut result = serde_json::Map::new();
    result.insert("kind".to_owned(), json!(kind));
    result.insert("scope".to_owned(), scope.clone());
    match kind {
        "cell_reached" => {
            assert_exact_keys(
                event,
                &["cell", "kind", "scope", "thread_id"],
                "cell_reached event",
            )?;
            result.insert("thread_id".to_owned(), event["thread_id"].clone());
            result.insert("cell".to_owned(), normalize_static_cell(&event["cell"])?);
        }
        "input_consumed" => {
            assert_exact_keys(
                event,
                &["kind", "scope", "thread_id", "value"],
                "input_consumed event",
            )?;
            result.insert("thread_id".to_owned(), event["thread_id"].clone());
            result.insert("value".to_owned(), event["value"].clone());
        }
        "register_changed" => {
            assert_exact_keys(
                event,
                &["kind", "new", "old", "register", "scope"],
                "register_changed event",
            )?;
            for key in ["register", "old", "new"] {
                result.insert(key.to_owned(), event[key].clone());
            }
        }
        "memory_changed" => {
            assert_exact_keys(
                event,
                &["kind", "location", "new", "old", "scope"],
                "memory_changed event",
            )?;
            result.insert(
                "location".to_owned(),
                normalize_memory_location(&event["location"])?,
            );
            result.insert("old".to_owned(), event["old"].clone());
            result.insert("new".to_owned(), event["new"].clone());
        }
        "code_changed" => {
            assert_exact_keys(
                event,
                &["cell", "kind", "new", "old", "scope"],
                "code_changed event",
            )?;
            result.insert("cell".to_owned(), normalize_static_cell(&event["cell"])?);
            result.insert("old".to_owned(), event["old"].clone());
            result.insert("new".to_owned(), event["new"].clone());
        }
        "thread_changed" => {
            assert_exact_keys(
                event,
                &["after", "before", "kind", "scope"],
                "thread_changed event",
            )?;
            for key in ["before", "after"] {
                result.insert(key.to_owned(), normalize_thread(&event[key], shape, false)?);
            }
        }
        other => bail!("unsupported VM event kind `{other}`"),
    }
    Ok(Value::Object(result))
}

fn normalize_thread(thread: &Value, shape: ResultShape, snapshot: bool) -> Result<Value> {
    let is_server = matches!(shape, ResultShape::ServerApi);
    let mut expected_keys = vec![
        "board",
        "data_stack",
        "direction",
        "id",
        "instruction_stack",
        "page",
        "phase",
        "position",
        "random_state",
        "register_pointer",
    ];
    if snapshot || is_server {
        expected_keys.push("code_grid");
    }
    expected_keys.push(if is_server {
        "call_frames"
    } else {
        "call_stack"
    });
    assert_exact_keys(thread, &expected_keys, "Full thread state")?;

    let instruction_stack = thread["instruction_stack"]
        .as_array()
        .context("thread instruction_stack must be an array")?
        .iter()
        .map(|item| match shape {
            ResultShape::ServerApi => item.as_u64().with_context(|| {
                format!("Runtime API instruction code must be an integer: {item}")
            }),
            ResultShape::NativeCli if item["kind"] == "empty" => {
                assert_exact_keys(item, &["kind"], "Empty instruction stack item")?;
                Ok(32)
            }
            ResultShape::NativeCli => {
                assert_exact_keys(
                    item,
                    &["instruction_code", "kind", "token"],
                    "Primary instruction stack item",
                )?;
                item["instruction_code"]
                    .as_u64()
                    .with_context(|| format!("CLI instruction stack entry has no code: {item}"))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let frames_key = if is_server {
        "call_frames"
    } else {
        "call_stack"
    };
    let call_frames = thread[frames_key]
        .as_array()
        .context("thread call frames must be an array")?
        .iter()
        .map(normalize_call_frame)
        .collect::<Result<Vec<_>>>()?;

    let mut result = serde_json::Map::new();
    if snapshot {
        result.insert(
            "code_grid".to_owned(),
            normalize_code_grid_name(&thread["code_grid"])?,
        );
    } else if is_server {
        // Runtime API thread-change events include the scope-derived code grid;
        // the CLI's equivalent event payload omits it.
        let _ = normalize_code_grid_name(&thread["code_grid"])?;
    }
    result.insert("id".to_owned(), thread["id"].clone());
    result.insert("board".to_owned(), normalize_board_name(&thread["board"])?);
    result.insert("position".to_owned(), thread["position"].clone());
    result.insert("direction".to_owned(), thread["direction"].clone());
    result.insert(
        "register_pointer".to_owned(),
        thread["register_pointer"].clone(),
    );
    result.insert("page".to_owned(), thread["page"].clone());
    result.insert("data_stack".to_owned(), thread["data_stack"].clone());
    result.insert("instruction_stack".to_owned(), json!(instruction_stack));
    result.insert("call_frames".to_owned(), json!(call_frames));
    result.insert("phase".to_owned(), thread["phase"].clone());
    result.insert("random_state".to_owned(), thread["random_state"].clone());
    Ok(Value::Object(result))
}

fn normalize_call_frame(frame: &Value) -> Result<Value> {
    assert_exact_keys(
        frame,
        &["call_position", "caller_board", "saved_direction"],
        "thread call frame",
    )?;
    Ok(json!({
        "caller_board": normalize_board_name(&frame["caller_board"])? ,
        "call_position": frame["call_position"],
        "saved_direction": frame["saved_direction"],
    }))
}

fn normalize_error(error: &Value, shape: ResultShape) -> Result<Value> {
    assert_exact_keys(
        error,
        &["code", "details", "error_number", "global_tick", "scope"],
        "runtime error",
    )?;
    let mut details = error["details"].clone();
    if let Some(object) = details.as_object_mut() {
        if let Some(board) = object.get("board").cloned() {
            object.insert("board".to_owned(), normalize_board_name(&board)?);
        }
        if let Some(cell) = object.get("cell").cloned() {
            object.insert("cell".to_owned(), normalize_static_cell(&cell)?);
        }
    }
    let _ = shape;
    let number = error["error_number"]
        .as_str()
        .context("runtime error number must be a string")?;
    ensure!(
        number.len() == 4 && number.bytes().all(|byte| byte.is_ascii_digit()),
        "runtime error number must contain exactly four digits"
    );
    Ok(json!({
        "code": error["code"],
        "error_number": error["error_number"],
        "global_tick": error["global_tick"],
        "scope": error["scope"],
        "details": details,
    }))
}

fn normalize_static_cell(cell: &Value) -> Result<Value> {
    assert_exact_keys(
        cell,
        &["board", "code_grid", "folded_block", "position"],
        "static cell id",
    )?;
    Ok(json!({
        "code_grid": normalize_code_grid_name(&cell["code_grid"])? ,
        "board": normalize_board_name(&cell["board"])? ,
        "folded_block": cell["folded_block"],
        "position": cell["position"],
    }))
}

fn normalize_memory_location(location: &Value) -> Result<Value> {
    assert_exact_keys(location, &["address", "space"], "memory location")?;
    Ok(location.clone())
}

fn normalize_code_grid_name(value: &Value) -> Result<Value> {
    match value {
        Value::String(name) => Ok(json!(name)),
        Value::Object(object) if object.get("kind").and_then(Value::as_str) == Some("outer") => {
            assert_exact_keys(value, &["kind"], "Outer code-grid ID")?;
            Ok(json!("outer"))
        }
        Value::Object(object) if object.get("kind").and_then(Value::as_str) == Some("custom") => {
            assert_exact_keys(value, &["id", "kind"], "Custom code-grid ID")?;
            Ok(json!(format!(
                "custom:{}",
                value["id"]
                    .as_u64()
                    .context("Custom ID must be an integer")?
            )))
        }
        _ => bail!("unsupported code-grid identifier {value}"),
    }
}

fn normalize_board_name(value: &Value) -> Result<Value> {
    match value {
        Value::String(name) => Ok(json!(name)),
        Value::Object(object) if object.get("kind").and_then(Value::as_str) == Some("main") => {
            assert_exact_keys(value, &["kind"], "Main board ID")?;
            Ok(json!("main"))
        }
        Value::Object(object) if object.get("kind").and_then(Value::as_str) == Some("function") => {
            assert_exact_keys(value, &["id", "kind"], "Function board ID")?;
            Ok(json!(format!(
                "function:{}",
                value["id"]
                    .as_u64()
                    .context("Function ID must be an integer")?
            )))
        }
        _ => bail!("unsupported board identifier {value}"),
    }
}

fn first_json_difference(left: &Value, right: &Value, path: &str) -> Option<String> {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left
                .keys()
                .chain(right.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                let child_path = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(difference) = first_json_difference(left, right, &child_path) {
                            return Some(difference);
                        }
                    }
                    (Some(_), None) => {
                        return Some(format!("{child_path}: missing from Wasmtime result"))
                    }
                    (None, Some(_)) => {
                        return Some(format!("{child_path}: missing from native result"))
                    }
                    (None, None) => unreachable!(),
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path}: lengths differ (native {}, Wasmtime {})",
                    left.len(),
                    right.len()
                ));
            }
            left.iter()
                .zip(right)
                .enumerate()
                .find_map(|(index, (left, right))| {
                    first_json_difference(left, right, &format!("{path}[{index}]"))
                })
        }
        _ if left == right => None,
        _ => Some(format!("{path}: native {left}, Wasmtime {right}")),
    }
}

fn cli_binary_path(root: &Path) -> Result<PathBuf> {
    let path = env::var_os("CODEGRID_CLI_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            target_directory(root)
                .join("release")
                .join(format!("codegrid{}", env::consts::EXE_SUFFIX))
        });
    ensure!(
        path.is_file(),
        "native CLI is required as the complete-result oracle; build it with `cargo build --release -p codegrid-cli` or set CODEGRID_CLI_PATH (looked for {})",
        path.display()
    );
    Ok(path)
}

fn verify_work_limit_yield(
    engine: &Engine,
    module: &Module,
    memory_limit_bytes: u64,
    cli_path: &Path,
    fixture: &Value,
) -> Result<()> {
    let fixture_id = fixture["id"]
        .as_str()
        .context("work-limit fixture is missing its id")?;
    let mut session = WasmtimeSession::instantiate(engine, module, memory_limit_bytes)
        .context("instantiate disposable work-limited session")?;
    let initialized = session.dispatch(initialize_request(1))?;
    ensure!(
        initialized["status"] == "initialized",
        "work-limited session failed initialization"
    );
    let compilation = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "compile",
        "source": fixture["source"],
    }))?;
    ensure!(
        compilation["status"] == "compiled",
        "work-limit fixture `{fixture_id}` did not compile"
    );
    let program = compilation["program"]
        .as_str()
        .context("work-limit compile returned no program handle")?;
    let created = session.dispatch(create_instance_request(program, &fixture["run"]))?;
    let instance = created["instance"]
        .as_str()
        .context("work-limit fixture returned no instance handle")?;
    let result = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "run",
        "instance": instance,
        "max_ticks": fixture["run"]["max_ticks"],
    }))?;
    ensure!(
        result["status"] == "work_limit_reached" && result["snapshot"]["committed_ticks"] == "1",
        "one-unit deterministic budget did not yield after preserving one committed tick: {result}"
    );
    ensure!(
        !result["events"]
            .as_array()
            .context("work-limited run must return events")?
            .is_empty(),
        "work-limited run failed to preserve events from its committed tick"
    );
    let native = run_native_cli(cli_path, fixture, 1)?;
    ensure!(
        native.exit_code == 7,
        "native CLI work-limit probe must exit 7, got {}",
        native.exit_code
    );
    compare_complete_results(fixture_id, &result, &native)?;

    let shutdown = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "shutdown",
    }))?;
    ensure!(
        shutdown["status"] == "closed",
        "work-limited session did not shut down"
    );
    Ok(())
}

fn verify_low_fuel_discards_session(
    engine: &Engine,
    module: &Module,
    memory_limit_bytes: u64,
    fixture: &Value,
) -> Result<()> {
    let mut session = WasmtimeSession::instantiate(engine, module, memory_limit_bytes)
        .context("instantiate disposable low-fuel session")?;
    let initialized = session.dispatch(initialize_request(DEFAULT_WORK_UNITS_PER_CALL))?;
    ensure!(
        initialized["status"] == "initialized",
        "low-fuel session failed initialization"
    );
    let compilation = session.dispatch(json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "compile",
        "source": fixture["source"],
    }))?;
    ensure!(
        compilation["status"] == "compiled",
        "low-fuel loop source failed to compile"
    );
    let program = compilation["program"]
        .as_str()
        .context("low-fuel loop compile returned no program handle")?;
    let created = session.dispatch(create_instance_request(program, &fixture["run"]))?;
    ensure!(
        created["operation"] == "create_instance",
        "low-fuel loop instance was not created"
    );
    let instance = created["instance"]
        .as_str()
        .context("low-fuel loop creation returned no instance handle")?;

    let run_result = session.dispatch_with_fuel(
        json!({
            "abi_version": ABI_VERSION,
            "api_version": API_VERSION,
            "operation": "run",
            "instance": instance,
            "max_ticks": "1000000",
        }),
        LOW_RUN_FUEL,
    );
    let fuel_error = match run_result {
        Ok(response) => {
            bail!("low-fuel loop unexpectedly completed within {LOW_RUN_FUEL} fuel: {response}")
        }
        Err(error) => error,
    };
    ensure!(
        fuel_error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel),
        "low-fuel loop failed for a reason other than Wasmtime OutOfFuel: {fuel_error:#}"
    );

    // The ABI call trapped midway through a request. Drop the whole Store and
    // instance rather than trying to reuse potentially partial guest state.
    drop(session);
    Ok(())
}

fn initialize_request(max_work_units_per_call: u64) -> Value {
    json!({
        "abi_version": ABI_VERSION,
        "api_version": API_VERSION,
        "operation": "initialize",
        "host_limits": {
            "max_source_bytes": 4 * 1024 * 1024,
            "max_compiled_programs": 8,
            "max_instances": 8,
            "max_input_bytes": 1024 * 1024,
            "max_initial_memory_entries": 65_536,
            "max_run_ticks_per_call": "1000000",
            "max_total_ticks_per_instance": "10000000",
            "max_work_units_per_call": max_work_units_per_call.to_string(),
            "max_response_bytes": 8 * 1024 * 1024,
            "max_instance_state_bytes": 8 * 1024 * 1024,
        },
    })
}

fn assert_exact_keys(value: &Value, keys: &[&str], context: &str) -> Result<()> {
    let object = value
        .as_object()
        .with_context(|| format!("{context} must be an object"))?;
    let actual: Vec<_> = object.keys().map(String::as_str).collect();
    let mut expected = keys.to_vec();
    expected.sort_unstable();
    ensure!(
        actual == expected,
        "{context} keys differ: expected {expected:?}, got {actual:?}"
    );
    Ok(())
}

fn configured_memory_limit() -> Result<u64> {
    let value = env::var("CODEGRID_WASM_MAX_MEMORY_BYTES").context(
        "set CODEGRID_WASM_MAX_MEMORY_BYTES to the value used to build the server module",
    )?;
    let bytes = value
        .parse::<u64>()
        .context("CODEGRID_WASM_MAX_MEMORY_BYTES must be unsigned decimal bytes")?;
    ensure!(
        bytes > 0 && bytes <= 4_294_967_296 && bytes % WASM_PAGE_BYTES == 0,
        "CODEGRID_WASM_MAX_MEMORY_BYTES must be a page-aligned value in 1..=4294967296"
    );
    Ok(bytes)
}

fn wasm_artifact_path() -> Result<PathBuf> {
    if let Some(path) = env::args_os().nth(1) {
        ensure!(
            env::args_os().nth(2).is_none(),
            "usage: codegrid-wasmtime-host [path-to-server-wasm]"
        );
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("CODEGRID_WASM_PATH") {
        return Ok(PathBuf::from(path));
    }
    Ok(target_directory(&repository_root())
        .join("wasm32-unknown-unknown/release/codegrid_wasm_server.wasm"))
}

fn target_directory(root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join("target"))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("standalone host manifest must live under tools/wasmtime-host")
}
