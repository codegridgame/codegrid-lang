//! No-import WebAssembly server adapter over the host-neutral Runtime API.
//!
//! The ABI accepts one bounded JSON request in a guest-allocated buffer and
//! returns a packed pointer/length pair for a bounded JSON response. Every run
//! recompiles source inside the adapter; no executable IR crosses the boundary.

use codegrid_runtime_api::error_number;
use codegrid_runtime_api::{ApiError, MetricCounterOverflow, RUNTIME_API_VERSION};
use serde_json::{json, Value};

mod projection;
mod session;
pub use session::ServerAdapter;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::BTreeMap};

pub const SERVER_ABI_VERSION: u32 = 4;

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
const MAX_RETAINED_BUFFER_BYTES: usize = 16 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
const MAX_RETAINED_BUFFER_COUNT: usize = 1024;
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_INITIAL_MEMORY_ENTRIES: usize = 65_536;
const MAX_RUN_TICKS_PER_CALL: u64 = 1_000_000;
const MAX_TOTAL_TICKS_PER_INSTANCE: u64 = 10_000_000;
const MAX_WORK_UNITS_PER_CALL: u64 = 1_000_000;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static BUFFERS: RefCell<BTreeMap<u32, Box<[u8]>>> = const { RefCell::new(BTreeMap::new()) };
    static SERVER_ADAPTER: RefCell<ServerAdapter> = const { RefCell::new(ServerAdapter::new()) };
}

fn parse_input(value: Option<&Value>) -> Result<Vec<u8>, (&'static str, &'static str)> {
    let items = value
        .and_then(Value::as_array)
        .ok_or(("invalid_input", "input must be an array of byte values"))?;
    if items.len() > MAX_INPUT_BYTES {
        return Err((
            "input_payload_limit_exceeded",
            "input exceeds the adapter byte limit",
        ));
    }
    items
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|number| u8::try_from(number).ok())
                .ok_or((
                    "invalid_input_byte",
                    "each input value must be an integer from 0 to 255",
                ))
        })
        .collect()
}

fn bounded_usize(
    value: Option<&Value>,
    name: &'static str,
    maximum: usize,
) -> Result<usize, (&'static str, &'static str)> {
    let parsed = value
        .and_then(Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or((
            "invalid_host_limit",
            "count and byte limits must be unsigned integers",
        ))?;
    if parsed > maximum {
        return Err(("host_limit_exceeds_adapter_ceiling", name));
    }
    Ok(parsed)
}

fn bounded_decimal_u64(
    value: Option<&Value>,
    name: &'static str,
    maximum: u64,
    require_positive: bool,
) -> Result<u64, (&'static str, &'static str)> {
    let parsed = decimal_u64(value, name)?;
    if (require_positive && parsed == 0) || parsed > maximum {
        return Err(("host_limit_exceeds_adapter_ceiling", name));
    }
    Ok(parsed)
}

fn decimal_u64(
    value: Option<&Value>,
    name: &'static str,
) -> Result<u64, (&'static str, &'static str)> {
    let text = value
        .and_then(Value::as_str)
        .ok_or(("invalid_decimal_integer", name))?;
    if !canonical_unsigned_decimal(text) {
        return Err(("invalid_decimal_integer", name));
    }
    text.parse::<u64>()
        .map_err(|_| ("invalid_decimal_integer", name))
}

fn canonical_unsigned_decimal(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
}

fn canonical_signed_decimal(text: &str) -> bool {
    if text == "0" {
        return true;
    }
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && !digits.starts_with('0')
        && !text.starts_with('+')
}

fn parse_memory_address(
    text: &str,
) -> Result<codegrid_runtime_api::MemoryAddress, (&'static str, &'static str)> {
    if !canonical_signed_decimal(text) {
        return Err((
            "invalid_memory_address",
            "memory address must be a canonical signed decimal string",
        ));
    }
    text.parse::<codegrid_runtime_api::MemoryAddress>()
        .map_err(|_| {
            (
                "invalid_memory_address",
                "memory address must be a canonical signed decimal string",
            )
        })
}

fn required_string<'a>(
    value: Option<&'a Value>,
    name: &'static str,
) -> Result<&'a str, (&'static str, &'static str)> {
    value
        .and_then(Value::as_str)
        .ok_or(("invalid_request_field", name))
}

fn required_u64(value: Option<&Value>) -> Result<u64, (&'static str, &'static str)> {
    value.and_then(Value::as_u64).ok_or((
        "invalid_request_field",
        "version fields must be unsigned integers",
    ))
}

fn error_response(code: &str, message: &str, details: Value) -> Value {
    json!({
        "abi_version": SERVER_ABI_VERSION,
        "api_version": RUNTIME_API_VERSION,
        "error": {"code": code, "error_number": error_number("server", code), "message": message, "details": details},
    })
}

fn api_error_response(api_version: u32, error: ApiError) -> Value {
    let (code, details) = match error {
        ApiError::UnsupportedVersion {
            received,
            supported,
        } => (
            "unsupported_api_version",
            json!({"received": received, "supported": supported}),
        ),
        ApiError::SourceLimitExceeded {
            received_bytes,
            maximum,
        } => (
            "source_payload_limit_exceeded",
            json!({"received_bytes": received_bytes, "maximum": maximum}),
        ),
        ApiError::ProgramLimitReached { maximum } => {
            ("program_limit_reached", json!({"maximum": maximum}))
        }
        ApiError::InstanceLimitReached { maximum } => {
            ("instance_limit_reached", json!({"maximum": maximum}))
        }
        ApiError::InputLimitExceeded {
            received_bytes,
            maximum,
        } => (
            "input_payload_limit_exceeded",
            json!({"received_bytes": received_bytes, "maximum": maximum}),
        ),
        ApiError::InitialMemoryLimitExceeded {
            received_entries,
            maximum,
        } => (
            "initial_memory_limit_exceeded",
            json!({"received_entries": received_entries, "maximum": maximum}),
        ),
        ApiError::DuplicateInitialMemoryAddress { address } => (
            "duplicate_initial_memory_address",
            json!({"address": address.to_string()}),
        ),
        ApiError::InvalidConfiguration { field } => {
            ("invalid_configuration", json!({"field": field}))
        }
        ApiError::UnknownProgramHandle { handle } => (
            "unknown_program_handle",
            json!({"program": handle.get().to_string()}),
        ),
        ApiError::UnknownInstanceHandle { handle } => (
            "unknown_instance_handle",
            json!({"instance": handle.get().to_string()}),
        ),
        ApiError::ZeroTickBudget => ("zero_tick_budget", Value::Null),
        ApiError::RunTickBudgetExceeded { requested, maximum } => (
            "run_tick_budget_exceeded",
            json!({"requested": requested.to_string(), "maximum": maximum.to_string()}),
        ),
        ApiError::InstanceTickBudgetExceeded {
            requested,
            remaining,
            maximum,
        } => (
            "instance_tick_budget_exceeded",
            json!({"requested": requested.to_string(), "remaining": remaining.to_string(), "maximum": maximum.to_string()}),
        ),
        ApiError::WorkUnitBudgetExceeded { maximum } => (
            "work_unit_budget_exceeded",
            json!({"maximum": maximum.to_string()}),
        ),
        ApiError::HandleSpaceExhausted => ("handle_space_exhausted", Value::Null),
        ApiError::VmInitialization(error) => (
            "vm_initialization_error",
            json!({"kind": format!("{error:?}")}),
        ),
    };
    error_response(code, "Runtime API rejected the request", details)
        .as_object()
        .map(|response| {
            let mut response = response.clone();
            response.insert("api_version".to_owned(), json!(api_version));
            Value::Object(response)
        })
        .unwrap_or(Value::Null)
}

fn direction_name(direction: codegrid_runtime_api::Direction) -> &'static str {
    match direction {
        codegrid_runtime_api::Direction::Up => "up",
        codegrid_runtime_api::Direction::Down => "down",
        codegrid_runtime_api::Direction::Left => "left",
        codegrid_runtime_api::Direction::Right => "right",
    }
}

fn metric_counter_name(counter: MetricCounterOverflow) -> &'static str {
    match counter {
        MetricCounterOverflow::OperationCount => "operation_count",
        MetricCounterOverflow::GlobalTick => "global_tick",
        MetricCounterOverflow::DataStackUsage => "data_stack_usage",
        MetricCounterOverflow::InstructionStackUsage => "instruction_stack_usage",
        MetricCounterOverflow::CallStackUsage => "call_stack_usage",
    }
}

#[cfg(target_arch = "wasm32")]
fn allocate_buffer(bytes: Box<[u8]>) -> Option<(u32, u32)> {
    let pointer = u32::try_from(bytes.as_ptr() as usize).ok()?;
    let length = u32::try_from(bytes.len()).ok()?;
    if pointer == 0 || length == 0 {
        return None;
    }
    BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        if buffers.contains_key(&pointer) || buffers.len() >= MAX_RETAINED_BUFFER_COUNT {
            return None;
        }
        let retained_bytes = buffers
            .values()
            .try_fold(0usize, |total, buffer| total.checked_add(buffer.len()))?;
        if retained_bytes.checked_add(length as usize)? > MAX_RETAINED_BUFFER_BYTES {
            return None;
        }
        buffers.insert(pointer, bytes);
        Some((pointer, length))
    })
}

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn codegrid_server_abi_version() -> u32 {
    SERVER_ABI_VERSION
}

/// Allocates a zero-filled guest buffer. Returns zero for invalid or excessive
/// lengths. The host writes request JSON into this owned region before dispatch.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn codegrid_server_alloc_buffer(length: u32) -> u32 {
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if length == 0 || length > MAX_REQUEST_BYTES {
        return 0;
    }
    let mut bytes = Vec::new();
    if bytes.try_reserve_exact(length).is_err() {
        return 0;
    }
    bytes.resize(length, 0);
    allocate_buffer(bytes.into_boxed_slice())
        .map(|(pointer, _)| pointer)
        .unwrap_or(0)
}

/// Releases a request or response buffer previously returned by this module.
/// Returns one on success and zero for an unknown pointer.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn codegrid_server_free_buffer(pointer: u32) -> u32 {
    u32::from(BUFFERS.with(|buffers| buffers.borrow_mut().remove(&pointer).is_some()))
}

/// Processes a request buffer and returns `(response_pointer << 32) | length`.
/// A response buffer sized to the session's configured response ceiling is
/// reserved before dispatch. A zero result means the request was invalid or
/// the response could not be reserved; in the latter case the request was not
/// dispatched. The host must copy or consume the response before freeing both
/// request and response buffers.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn codegrid_server_process_request(pointer: u32, length: u32) -> u64 {
    let Ok(request_length) = usize::try_from(length) else {
        return 0;
    };
    let request_is_valid = BUFFERS.with(|buffers| {
        let buffers = buffers.borrow();
        buffers
            .get(&pointer)
            .is_some_and(|buffer| buffer.get(..request_length).is_some())
    });
    if !request_is_valid {
        return 0;
    }

    let response_capacity =
        SERVER_ADAPTER.with(|adapter| adapter.borrow().response_buffer_capacity());
    let Ok(response_capacity) = u32::try_from(response_capacity) else {
        return 0;
    };
    let response_pointer = codegrid_server_alloc_buffer(response_capacity);
    if response_pointer == 0 {
        return 0;
    }

    let response = BUFFERS.with(|buffers| {
        let buffers = buffers.borrow();
        let buffer = buffers.get(&pointer)?;
        let request = buffer.get(..request_length)?;
        Some(SERVER_ADAPTER.with(|adapter| adapter.borrow_mut().process_request(request)))
    });
    let Some(response) = response else {
        codegrid_server_free_buffer(response_pointer);
        return 0;
    };
    if response.is_empty() || response.len() > response_capacity as usize {
        codegrid_server_free_buffer(response_pointer);
        return 0;
    }
    let Ok(response_length) = u32::try_from(response.len()) else {
        codegrid_server_free_buffer(response_pointer);
        return 0;
    };
    let response_written = BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        let response_buffer = buffers.get_mut(&response_pointer)?;
        response_buffer
            .get_mut(..response.len())?
            .copy_from_slice(&response);
        Some(())
    });
    if response_written.is_none() {
        codegrid_server_free_buffer(response_pointer);
        return 0;
    }
    (u64::from(response_pointer) << 32) | u64::from(response_length)
}

#[cfg(test)]
mod tests {
    use super::session::ServerAdapter;
    use serde_json::{json, Value};

    fn server_limits() -> Value {
        json!({
            "max_source_bytes": 4096,
            "max_compiled_programs": 8,
            "max_instances": 8,
            "max_input_bytes": 4096,
            "max_initial_memory_entries": 65536,
            "max_run_ticks_per_call": "1000",
            "max_total_ticks_per_instance": "10000",
            "max_gas_per_instance": "100000000",
            "max_work_units_per_call": "10000",
            "max_response_bytes": 1048576,
            "max_instance_state_bytes": 524288
        })
    }

    fn dispatch_bytes(adapter: &mut ServerAdapter, request: &[u8]) -> Vec<u8> {
        adapter.process_request(request)
    }

    fn dispatch(adapter: &mut ServerAdapter, request: Value) -> Value {
        let response = dispatch_bytes(adapter, &serde_json::to_vec(&request).unwrap());
        serde_json::from_slice(&response).unwrap()
    }

    fn assert_control_response_fits(adapter: &mut ServerAdapter, request: Value) -> Value {
        let maximum = adapter.response_buffer_capacity();
        assert_eq!(
            maximum, 256,
            "uninitialized/closed control ceiling is 256 bytes"
        );
        let response = dispatch_bytes(adapter, &serde_json::to_vec(&request).unwrap());
        assert!(
            response.len() <= maximum,
            "control response of {} bytes exceeded its {maximum}-byte capacity: {}",
            response.len(),
            String::from_utf8_lossy(&response)
        );
        serde_json::from_slice(&response).expect("control response must be complete JSON")
    }

    fn operation(name: &str) -> Value {
        json!({
            "abi_version": 4,
            "api_version": 3,
            "operation": name,
            "gas_hard_limit": "100000000",
        })
    }

    #[test]
    fn control_responses_fit_the_preinitialized_and_closed_capacity() {
        let mut adapter = ServerAdapter::new();
        assert_eq!(adapter.response_buffer_capacity(), 256);
        for request in [
            json!({}),
            json!({"abi_version": 99, "api_version": 3, "operation": "check"}),
            json!({"abi_version": 4, "api_version": u64::MAX, "operation": "check"}),
            json!({"abi_version": 4, "api_version": 99, "operation": "check"}),
            json!({"abi_version": 4, "api_version": 3}),
            json!({"abi_version": 4, "api_version": 3, "operation": "initialize"}),
        ] {
            assert_control_response_fits(&mut adapter, request);
        }

        for name in [
            "check",
            "compile",
            "program_view",
            "create_instance",
            "step",
            "run",
            "snapshot",
            "release_program",
            "release_instance",
        ] {
            let response = assert_control_response_fits(&mut adapter, operation(name));
            assert_eq!(response["error"]["code"], "runtime_not_initialized");
        }

        let mut initialize = operation("initialize");
        let mut limits = server_limits();
        limits["max_response_bytes"] = json!(256);
        limits["max_instance_state_bytes"] = json!(256);
        initialize["host_limits"] = limits;
        let response = assert_control_response_fits(&mut adapter, initialize);
        assert_eq!(response["status"], "initialized");
        assert_eq!(adapter.response_buffer_capacity(), 256);

        assert_eq!(
            assert_control_response_fits(&mut adapter, operation("shutdown"))["status"],
            "closed"
        );
        for name in [
            "initialize",
            "shutdown",
            "check",
            "compile",
            "unknown-operation",
        ] {
            let response = assert_control_response_fits(&mut adapter, operation(name));
            if name != "shutdown" {
                assert_eq!(response["error"]["code"], "runtime_closed");
            }
        }
    }

    #[test]
    fn runtime_operations_before_initialization_return_a_structured_error() {
        let mut adapter = ServerAdapter::new();

        for name in [
            "check",
            "compile",
            "program_view",
            "create_instance",
            "step",
            "run",
            "snapshot",
            "release_program",
            "release_instance",
        ] {
            let response = dispatch(&mut adapter, operation(name));

            assert_eq!(
                response["error"]["code"], "runtime_not_initialized",
                "operation {name} must not reach runtime methods before initialization"
            );
        }
    }

    #[test]
    fn oversized_compile_and_diagnostics_return_complete_bounded_error_json() {
        let maximum = 1024;
        let mut limits = server_limits();
        limits["max_compiled_programs"] = json!(1);
        limits["max_response_bytes"] = json!(maximum);
        limits["max_instance_state_bytes"] = json!(maximum);

        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = limits;
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        let mut oversized_compile = operation("compile");
        oversized_compile["source"] = json!(format!("~> {}\n", "_ ".repeat(500)));
        let compile_bytes = adapter.process_request(
            &serde_json::to_vec(&oversized_compile).expect("compile request must serialize"),
        );
        assert!(compile_bytes.len() <= maximum);
        let compile_response: Value =
            serde_json::from_slice(&compile_bytes).expect("limit result must be complete JSON");
        assert_eq!(
            compile_response["error"]["code"],
            "response_payload_limit_exceeded"
        );

        let mut valid_compile = operation("compile");
        valid_compile["source"] = json!("~> , . ;\n");
        assert_eq!(dispatch(&mut adapter, valid_compile)["status"], "compiled");

        let mut oversized_check = operation("check");
        oversized_check["source"] = json!("~x\n".repeat(100));
        let check_bytes = adapter.process_request(
            &serde_json::to_vec(&oversized_check).expect("check request must serialize"),
        );
        assert!(check_bytes.len() <= maximum);
        let check_response: Value =
            serde_json::from_slice(&check_bytes).expect("limit result must be complete JSON");
        assert_eq!(
            check_response["error"]["code"],
            "response_payload_limit_exceeded"
        );
    }

    #[test]
    fn persistent_runtime_exposes_compile_step_run_snapshot_and_release_lifecycle() {
        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(
            dispatch(&mut adapter, initialize.clone())["status"],
            "initialized"
        );
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        let source = "~> , . ;\n";
        let mut compile = operation("compile");
        compile["source"] = json!(source);
        let compiled = dispatch(&mut adapter, compile);
        assert_eq!(compiled["status"], "compiled");
        assert_eq!(compiled["view"]["ir_format_version"], 3);
        let program = compiled["program"].as_str().unwrap().to_owned();
        let mut program_view_request = operation("program_view");
        program_view_request["program"] = json!(program.clone());
        assert_eq!(
            dispatch(&mut adapter, program_view_request)["view"],
            compiled["view"],
            "program_view must return the same canonical Full view as compile"
        );

        let create = |input: u8| {
            let mut request = operation("create_instance");
            request["program"] = json!(program);
            request["input"] = json!([input]);
            request["seed"] = json!("0");
            request["custom_execution_limit"] = json!("100");
            request["gas_hard_limit"] = json!("100000000");
            request["initial_memory"] = json!([]);
            request
        };
        let first = dispatch(&mut adapter, create(41));
        let second = dispatch(&mut adapter, create(99));
        let first_id = first["instance"].as_str().unwrap().to_owned();
        let second_id = second["instance"].as_str().unwrap().to_owned();

        let mut step = operation("step");
        step["instance"] = json!(first_id);
        let stepped = dispatch(&mut adapter, step);
        assert_eq!(stepped["operation"], "step");
        assert!(stepped["result"]["newly_emitted_output"].is_array());
        assert!(stepped["result"]["errors"].is_array());
        assert_eq!(stepped["result"]["committed_ticks"], "1");

        let mut snapshot_first = operation("snapshot");
        snapshot_first["instance"] = json!(first_id);
        let first_snapshot = dispatch(&mut adapter, snapshot_first);
        assert_eq!(first_snapshot["snapshot"]["committed_ticks"], "1");
        assert_eq!(first_snapshot["snapshot"]["registers"][0], 0);
        assert!(first_snapshot["snapshot"]["threads"].is_array());
        assert!(first_snapshot["snapshot"].get("runtime_program").is_some());
        assert!(first_snapshot["snapshot"]["metrics"]["used_memory_addresses"].is_array());

        let mut snapshot_second = operation("snapshot");
        snapshot_second["instance"] = json!(second_id);
        let second_snapshot = dispatch(&mut adapter, snapshot_second);
        assert_eq!(second_snapshot["snapshot"]["committed_ticks"], "0");
        assert_eq!(second_snapshot["snapshot"]["output"], json!([]));

        let mut release_program = operation("release_program");
        release_program["program"] = json!(program);
        assert_eq!(
            dispatch(&mut adapter, release_program)["operation"],
            "release_program"
        );

        let run = |adapter: &mut ServerAdapter, instance: &str| {
            let mut request = operation("run");
            request["instance"] = json!(instance);
            request["max_ticks"] = json!("10");
            dispatch(adapter, request)
        };
        let second_result = run(&mut adapter, &second_id);
        let first_result = run(&mut adapter, &first_id);
        assert_eq!(second_result["snapshot"]["output"], json!([99]));
        assert_eq!(first_result["snapshot"]["output"], json!([41]));

        for instance in [&first_id, &second_id] {
            let mut release = operation("release_instance");
            release["instance"] = json!(instance);
            assert_eq!(
                dispatch(&mut adapter, release)["operation"],
                "release_instance"
            );
        }
        assert_eq!(
            dispatch(&mut adapter, operation("shutdown"))["status"],
            "closed"
        );
        let mut closed_request = operation("check");
        closed_request["source"] = json!(source);
        assert_eq!(
            dispatch(&mut adapter, closed_request)["error"]["code"],
            "runtime_closed"
        );
    }

    #[test]
    fn runtime_api_v3_full_conformance_suite_matches_snapshot_and_result_fields() {
        let suite: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/conformance-v1.json"))
                .expect("shared Full conformance suite must be valid JSON");
        assert_eq!(suite["schema_version"], 1);
        let cases = suite["cases"]
            .as_array()
            .expect("suite cases must be an array");
        assert!(!cases.is_empty());

        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        for fixture in cases {
            let id = fixture["id"].as_str().expect("case id must be a string");
            let run = &fixture["run"];
            let compiled = dispatch(
                &mut adapter,
                json!({"abi_version":4,"api_version":3,"operation":"compile","source":fixture["source"]}),
            );
            assert_eq!(compiled["status"], "compiled", "{id}: compile");
            let created = dispatch(
                &mut adapter,
                json!({
                    "abi_version":4,"api_version":3,"operation":"create_instance",
                    "program":compiled["program"],"input":run["input"],
                    "seed":run["seed"],
                    "custom_execution_limit":run["custom_execution_limit"],
                    "gas_hard_limit":run["gas_hard_limit"],
                    "initial_memory":run.get("initial_memory").cloned().unwrap_or_else(|| json!([])),
                }),
            );
            assert!(
                created["instance"].is_string(),
                "{id}: create instance: {created}"
            );
            let result = dispatch(
                &mut adapter,
                json!({
                    "abi_version":4,"api_version":3,"operation":"run",
                    "instance":created["instance"],"max_ticks":run["max_ticks"],
                }),
            );
            let expected = &fixture["expected"];
            assert_eq!(result["status"], expected["status"], "{id}: run status");
            assert_eq!(
                result["snapshot"]["status"], expected["vm_status"],
                "{id}: VM status"
            );
            assert_eq!(
                result["snapshot"]["output"], expected["output"],
                "{id}: output"
            );
            assert_eq!(
                result["snapshot"]["registers"], expected["registers"],
                "{id}: registers"
            );
            assert_eq!(
                result["snapshot"]["committed_ticks"], expected["committed_ticks"],
                "{id}: ticks"
            );
            for metric in [
                "operation_count",
                "gas_used",
                "execution_gas",
                "memory_gas",
                "stack_gas",
                "gas_schedule_version",
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
                result["snapshot"]["metrics"]["instruction_variety"],
                expected["instruction_variety"],
                "{id}: instruction variety"
            );
            let errors = result["snapshot"]["errors"]
                .as_array()
                .expect("errors must be an array");
            assert_eq!(
                json!(errors
                    .iter()
                    .map(|error| error["code"].clone())
                    .collect::<Vec<_>>()),
                expected["error_codes"],
                "{id}: error codes"
            );

            let threads = result["snapshot"]["threads"]
                .as_array()
                .expect("Full snapshot must include threads");
            let actual_optional = json!({
                "error_custom_internal_ticks": errors.iter().filter(|error| error["scope"]["kind"] == "custom").map(|error| error["scope"]["internal_tick"].clone()).collect::<Vec<_>>(),
                "error_global_ticks": errors.iter().map(|error| error["global_tick"].clone()).collect::<Vec<_>>(),
                "error_thread_id_groups": errors.iter().filter_map(|error| error["details"].get("thread_ids").or_else(|| error["details"].get("internal_thread_ids")).cloned()).collect::<Vec<_>>(),
                "error_thread_ids": errors.iter().filter_map(|error| error["details"].get("thread_id").cloned()).collect::<Vec<_>>(),
                "memory": result["snapshot"]["memory"],
                "remaining_input": result["snapshot"]["remaining_input"],
                "runtime_main_attachment_tokens": result["snapshot"]["runtime_program"]["main"]["cells"].as_array().unwrap().iter().map(|cell| cell["attachment"].clone()).collect::<Vec<_>>(),
                "runtime_main_primary_tokens": result["snapshot"]["runtime_program"]["main"]["cells"].as_array().unwrap().iter().map(|cell| cell["primary"].clone()).collect::<Vec<_>>(),
                "thread_call_stack_sizes": threads.iter().map(|thread| thread["call_frames"].as_array().unwrap().len()).collect::<Vec<_>>(),
                "thread_data_stack_sizes": threads.iter().map(|thread| thread["data_stack"].as_array().unwrap().len()).collect::<Vec<_>>(),
                "thread_direction": threads.first().map(|thread| thread["direction"].clone()),
                "thread_instruction_stack_sizes": threads.iter().map(|thread| thread["instruction_stack"].as_array().unwrap().len()).collect::<Vec<_>>(),
                "thread_random_state": threads.first().map(|thread| thread["random_state"].clone()),
                "threads": threads.iter().map(|thread| json!({"id":thread["id"],"position":thread["position"],"direction":thread["direction"]})).collect::<Vec<_>>(),
            });
            for field in [
                "error_custom_internal_ticks",
                "error_global_ticks",
                "error_thread_id_groups",
                "error_thread_ids",
                "memory",
                "remaining_input",
                "runtime_main_attachment_tokens",
                "runtime_main_primary_tokens",
                "thread_call_stack_sizes",
                "thread_data_stack_sizes",
                "thread_direction",
                "thread_instruction_stack_sizes",
                "thread_random_state",
                "threads",
            ] {
                if expected.get(field).is_some() {
                    assert_eq!(actual_optional[field], expected[field], "{id}: {field}");
                }
            }

            dispatch(
                &mut adapter,
                json!({"abi_version":4,"api_version":3,"operation":"release_instance","instance":created["instance"]}),
            );
            dispatch(
                &mut adapter,
                json!({"abi_version":4,"api_version":3,"operation":"release_program","program":compiled["program"]}),
            );
        }
    }

    #[test]
    fn check_returns_source_diagnostics_without_running_the_vm() {
        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        let mut check = operation("check");
        check["source"] = json!("~x\n");
        let response = dispatch(&mut adapter, check);
        assert_eq!(response["operation"], "check");
        assert_eq!(response["diagnostics"][0]["severity"], "error");
        assert_eq!(response["diagnostics"][0]["code"], "source.invalid_entry");
        assert!(response["diagnostics"][0]["span"]["start"].is_string());
    }

    #[test]
    fn create_instance_preserves_wide_seed_custom_limit_and_initial_memory() {
        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        let mut compile = operation("compile");
        compile["source"] = json!("~> ;\n");
        let program = dispatch(&mut adapter, compile)["program"].clone();
        let address = "-340282366920938463463374607431768211456";
        let mut create = operation("create_instance");
        create["program"] = program;
        create["input"] = json!([]);
        create["seed"] = json!(u64::MAX.to_string());
        create["custom_execution_limit"] = json!(u64::MAX.to_string());
        create["initial_memory"] = json!([
            {"address": address, "value": 255},
            {"address": "0", "value": 0},
        ]);
        let instance = dispatch(&mut adapter, create)["instance"].clone();
        assert!(
            instance.is_string(),
            "v3 instance creation should accept exact wide integers"
        );

        let mut snapshot = operation("snapshot");
        snapshot["instance"] = instance;
        let snapshot = dispatch(&mut adapter, snapshot)["snapshot"].clone();
        assert_eq!(
            snapshot["memory"],
            json!([{"address": address, "value": 255}])
        );
        assert!(snapshot["threads"][0]["random_state"].is_string());
        assert_eq!(snapshot["metrics"]["global_tick"], "0");
    }

    #[test]
    fn initial_memory_rejects_noncanonical_or_duplicate_wide_addresses() {
        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");
        let mut compile = operation("compile");
        compile["source"] = json!("~> ;\n");
        let program = dispatch(&mut adapter, compile)["program"].clone();

        for address in ["01", "-0", "+1"] {
            let mut create = operation("create_instance");
            create["program"] = program.clone();
            create["input"] = json!([]);
            create["seed"] = json!("0");
            create["custom_execution_limit"] = json!("1");
            create["initial_memory"] = json!([{"address": address, "value": 1}]);
            assert_eq!(
                dispatch(&mut adapter, create)["error"]["code"],
                "invalid_memory_address"
            );
        }

        let mut duplicate = operation("create_instance");
        duplicate["program"] = program;
        duplicate["input"] = json!([]);
        duplicate["seed"] = json!("0");
        duplicate["custom_execution_limit"] = json!("1");
        duplicate["initial_memory"] = json!([
            {"address": "-123456789012345678901234567890", "value": 1},
            {"address": "-123456789012345678901234567890", "value": 2},
        ]);
        let response = dispatch(&mut adapter, duplicate);
        assert_eq!(
            response["error"]["code"],
            "duplicate_initial_memory_address"
        );
        assert_eq!(
            response["error"]["details"]["address"],
            "-123456789012345678901234567890"
        );
    }

    #[test]
    fn invalid_input_is_a_host_error_and_not_a_vm_result() {
        let mut adapter = ServerAdapter::new();
        let mut initialize = operation("initialize");
        initialize["host_limits"] = server_limits();
        assert_eq!(dispatch(&mut adapter, initialize)["status"], "initialized");

        let mut compile = operation("compile");
        compile["source"] = json!("~> ;\n");
        let program = dispatch(&mut adapter, compile)["program"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut create = operation("create_instance");
        create["program"] = json!(program);
        create["input"] = json!([256]);
        let response = dispatch(&mut adapter, create);
        assert_eq!(response["error"]["code"], "invalid_input_byte");
        assert!(response.get("snapshot").is_none());
    }

    #[test]
    fn unsupported_versions_and_over_ceiling_limits_are_distinct() {
        let mut adapter = ServerAdapter::new();
        let mut unsupported_abi = operation("check");
        unsupported_abi["abi_version"] = json!(99);
        let response = dispatch(&mut adapter, unsupported_abi);
        assert_eq!(response["error"]["code"], "unsupported_abi_version");

        let mut excessive_limit = operation("initialize");
        let mut limits = server_limits();
        limits["max_run_ticks_per_call"] = json!("1000001");
        excessive_limit["host_limits"] = limits;
        let response = dispatch(&mut adapter, excessive_limit);
        assert_eq!(
            response["error"]["code"],
            "host_limit_exceeds_adapter_ceiling"
        );
    }
}
