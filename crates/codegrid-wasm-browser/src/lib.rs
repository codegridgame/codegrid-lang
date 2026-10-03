//! Browser-facing WebAssembly bindings over the shared runtime API.
//!
//! This adapter contains no language rules. It owns only the JavaScript
//! binding and the JSON projection for the Full Runtime API contract.

use codegrid_runtime_api::error_number;
use codegrid_runtime_api::{
    ApiError, BoardId, BoardView, BoundaryMode, CallFrameSnapshot, CheckRequest, CodeGridId,
    CodeGridView, CompileOutcome, CompileRequest, Coordinate, CreateInstanceRequest, Diagnostic,
    Direction, ExecutionScope, HostLimits, InstanceHandle, InstructionKind, MemoryAddress,
    MemoryEntry, MemoryLocationId, MemorySpaceId, MetricCounterOverflow, ProgramHandle,
    ProgramView, ProgramViewRequest, ReleaseInstanceRequest, ReleaseProgramRequest, RunRequest,
    RunStatus, RuntimeApi, RuntimeConfiguration, RuntimeError, RuntimeErrorKind,
    RuntimeMetricSummary, RuntimeMetrics, RuntimeSnapshotView, RuntimeThreadSnapshotView, Severity,
    SnapshotRequest, StaticCellId, StepRequest, StepResponseView, ThreadPhaseSnapshot, VmEvent,
    VmFault, VmStatus, YieldReason, RUNTIME_API_VERSION,
};
use js_sys::Uint8Array;
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Map;
use serde_json::{json, Value};
use std::fmt::Display;
use std::io::{self, Write};
use std::num::NonZeroU64;
use std::str::FromStr;
use wasm_bindgen::{prelude::*, JsValue};

/// Stateful browser runtime with per-instance program and VM registries.
#[wasm_bindgen]
pub struct BrowserRuntime {
    runtime: RuntimeApi,
    max_request_bytes: u32,
    max_response_bytes: u32,
    max_instance_state_bytes: u32,
}

const MIN_RESPONSE_BYTES: u32 = 256;
const MIN_INSTANCE_STATE_BYTES: u32 = 256;
const RESPONSE_LIMIT_ERROR: &str = r#"{"api_version":3,"error":{"code":"response_payload_limit_exceeded","error_number":"7019","message":"serialized response exceeds the configured byte limit"}}"#;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigurationWire {
    boundary_mode: String,
    seed: String,
    custom_execution_limit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemoryEntryWire {
    address: String,
    value: u8,
}

struct BoundedJsonWriter {
    maximum: usize,
    bytes: Vec<u8>,
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "serialized JSON response exceeds its configured byte limit",
            ));
        }
        self.bytes
            .try_reserve_exact(buffer.len())
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_bounded_json<T: Serialize + ?Sized>(value: &T, maximum: usize) -> String {
    let mut writer = BoundedJsonWriter {
        maximum,
        bytes: Vec::new(),
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return RESPONSE_LIMIT_ERROR.to_owned();
    }
    String::from_utf8(writer.bytes).expect("serde_json always emits valid UTF-8")
}

struct BoundedJsonCounter {
    maximum: usize,
    bytes: usize,
}

impl Write for BoundedJsonCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.maximum.saturating_sub(self.bytes) {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "serialized VM snapshot exceeds its configured state byte limit",
            ));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn json_fits_byte_limit<T: Serialize + ?Sized>(value: &T, maximum: usize) -> bool {
    serde_json::to_writer(&mut BoundedJsonCounter { maximum, bytes: 0 }, value).is_ok()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function measureUtf8BytesUpTo(value, maximum) {
  if (typeof value !== 'string') return -1;
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code < 0x80) bytes += 1;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && index + 1 < value.length) {
      const next = value.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) { bytes += 4; index += 1; }
      else bytes += 3;
    } else bytes += 3;
    if (bytes > maximum) return maximum + 1;
  }
  return bytes;
}
export function measureUtf8PairBytesUpTo(first, second, maximum) {
  const firstBytes = measureUtf8BytesUpTo(first, maximum);
  if (firstBytes < 0 || firstBytes > maximum) return firstBytes < 0 ? -1 : maximum + 1;
  const secondBytes = measureUtf8BytesUpTo(second, maximum - firstBytes);
  if (secondBytes < 0 || secondBytes > maximum - firstBytes) {
    return secondBytes < 0 ? -1 : maximum + 1;
  }
  return firstBytes + secondBytes;
}
export function measureCreateRequestBytesUpTo(program, input, initialMemory, configuration, maximum) {
  if (typeof program !== 'string') return -1;
  if (typeof initialMemory !== 'string') return -3;
  if (typeof configuration !== 'string') return -4;
  const inputBytes = byteArrayLengthUpTo(input, maximum);
  if (inputBytes < 0) return -2;
  const programBytes = measureUtf8BytesUpTo(program, maximum);
  const memoryBytes = measureUtf8BytesUpTo(initialMemory, maximum);
  const configurationBytes = measureUtf8BytesUpTo(configuration, maximum);
  if (programBytes < 0 || memoryBytes < 0 || configurationBytes < 0) return -1;
  if (programBytes > maximum || memoryBytes > maximum || configurationBytes > maximum || inputBytes > maximum) return maximum + 1;
  const total = programBytes + memoryBytes + configurationBytes + inputBytes;
  return total > maximum ? maximum + 1 : total;
}
export function byteArrayLengthUpTo(value, maximum) {
  if (!ArrayBuffer.isView(value)) return -1;
  const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
  const typedArrayTag = Object.getOwnPropertyDescriptor(typedArrayPrototype, Symbol.toStringTag).get.call(value);
  if (typedArrayTag !== 'Uint8Array') return -1;
  return value.byteLength > maximum ? maximum + 1 : value.byteLength;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = measureUtf8BytesUpTo)]
    fn measure_utf8_bytes_up_to(value: &JsValue, maximum: f64) -> f64;
    #[wasm_bindgen(js_name = measureUtf8PairBytesUpTo)]
    fn measure_utf8_pair_bytes_up_to(first: &JsValue, second: &JsValue, maximum: f64) -> f64;
    #[wasm_bindgen(js_name = measureCreateRequestBytesUpTo)]
    fn measure_create_request_bytes_up_to(
        program: &JsValue,
        input: &JsValue,
        initial_memory: &JsValue,
        configuration: &JsValue,
        maximum: f64,
    ) -> f64;
    #[wasm_bindgen(js_name = byteArrayLengthUpTo)]
    fn byte_array_length_up_to(value: &JsValue, maximum: f64) -> f64;
}

#[cfg(not(target_arch = "wasm32"))]
fn measure_utf8_bytes_up_to(_value: &JsValue, _maximum: f64) -> f64 {
    -1.0
}

#[cfg(not(target_arch = "wasm32"))]
fn measure_utf8_pair_bytes_up_to(_first: &JsValue, _second: &JsValue, _maximum: f64) -> f64 {
    -1.0
}

#[cfg(not(target_arch = "wasm32"))]
fn measure_create_request_bytes_up_to(
    _program: &JsValue,
    _input: &JsValue,
    _initial_memory: &JsValue,
    _configuration: &JsValue,
    _maximum: f64,
) -> f64 {
    -1.0
}

#[cfg(not(target_arch = "wasm32"))]
fn byte_array_length_up_to(_value: &JsValue, _maximum: f64) -> f64 {
    -1.0
}

fn bounded_string(value: &JsValue, maximum: u32) -> Result<String, BoundedInputError> {
    match measure_utf8_bytes_up_to(value, f64::from(maximum)) {
        measured if measured < 0.0 => Err(BoundedInputError::WrongType),
        measured if measured > f64::from(maximum) => Err(BoundedInputError::TooLarge),
        _ => value.as_string().ok_or(BoundedInputError::WrongType),
    }
}

fn bounded_string_pair(
    first: &JsValue,
    second: &JsValue,
    maximum: u32,
) -> Result<(String, String), BoundedInputError> {
    match measure_utf8_pair_bytes_up_to(first, second, f64::from(maximum)) {
        measured if measured < 0.0 => Err(BoundedInputError::WrongType),
        measured if measured > f64::from(maximum) => Err(BoundedInputError::TooLarge),
        _ => Ok((
            first.as_string().ok_or(BoundedInputError::WrongType)?,
            second.as_string().ok_or(BoundedInputError::WrongType)?,
        )),
    }
}

fn bounded_byte_array(value: &JsValue, maximum: u32) -> Result<Vec<u8>, BoundedInputError> {
    match byte_array_length_up_to(value, f64::from(maximum)) {
        measured if measured < 0.0 => Err(BoundedInputError::WrongType),
        measured if measured > f64::from(maximum) => Err(BoundedInputError::TooLarge),
        measured => {
            let mut bytes = bounded_zeroed_buffer(measured as usize, maximum as usize)?;
            Uint8Array::new(value).copy_to(&mut bytes);
            Ok(bytes)
        }
    }
}

fn bounded_zeroed_buffer(length: usize, maximum: usize) -> Result<Vec<u8>, BoundedInputError> {
    if length > maximum {
        return Err(BoundedInputError::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| BoundedInputError::AllocationFailed)?;
    bytes.resize(length, 0);
    Ok(bytes)
}

#[derive(Clone, Copy, Debug)]
enum BoundedInputError {
    WrongType,
    TooLarge,
    AllocationFailed,
}

fn bounded_input_error_json(field: &str, error: BoundedInputError, maximum: usize) -> String {
    let (code, message) = match (field, error) {
        ("source", BoundedInputError::TooLarge) => (
            "source_payload_limit_exceeded",
            "source exceeds the configured UTF-8 byte limit",
        ),
        ("input", BoundedInputError::TooLarge) => (
            "input_payload_limit_exceeded",
            "input exceeds the configured byte limit",
        ),
        ("source", BoundedInputError::WrongType) => ("invalid_source", "source must be a string"),
        ("input", BoundedInputError::WrongType) => ("invalid_input", "input must be a Uint8Array"),
        ("input", BoundedInputError::AllocationFailed) => (
            "input_allocation_failed",
            "available WebAssembly memory could not copy the input bytes",
        ),
        (_, BoundedInputError::TooLarge) => (
            "request_payload_limit_exceeded",
            "request field exceeds its configured byte limit",
        ),
        (_, BoundedInputError::AllocationFailed) => (
            "request_allocation_failed",
            "available WebAssembly memory could not copy the request field",
        ),
        (_, BoundedInputError::WrongType) => {
            ("invalid_request_field", "request field has an invalid type")
        }
    };
    serialize_bounded_json(
        &json!({
            "api_version": RUNTIME_API_VERSION,
            "error": { "code": code, "error_number": error_number("browser", code), "message": message },
        }),
        maximum,
    )
}

fn as_u32_limit(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn parse_u32_number(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0)
        .then_some(value as u32)
}

fn host_limit_error(message: &str) -> JsValue {
    JsValue::from_str(&format!(
        "[{}] [browser.invalid_host_limit] {message}",
        error_number("browser", "browser.invalid_host_limit").expect("Registered browser error")
    ))
}

#[wasm_bindgen]
impl BrowserRuntime {
    /// Creates an isolated runtime using explicit host resource limits.
    ///
    /// Payload and response ceilings are checked before copying strings or
    /// typed arrays into WASM. Wide execution limits use decimal strings.
    #[wasm_bindgen(constructor)]
    pub fn new(
        max_source_bytes: f64,
        max_compiled_programs: f64,
        max_instances: f64,
        max_input_bytes: f64,
        max_initial_memory_entries: f64,
        max_request_bytes: f64,
        max_response_bytes: f64,
        max_instance_state_bytes: f64,
        max_run_ticks_per_call: JsValue,
        max_total_ticks_per_instance: JsValue,
        max_work_units_per_call: JsValue,
    ) -> Result<BrowserRuntime, JsValue> {
        let max_source_bytes = parse_u32_number(max_source_bytes)
            .ok_or_else(|| host_limit_error("max_source_bytes must be a u32 integer"))?;
        let max_compiled_programs = parse_u32_number(max_compiled_programs)
            .ok_or_else(|| host_limit_error("max_compiled_programs must be a u32 integer"))?;
        let max_instances = parse_u32_number(max_instances)
            .ok_or_else(|| host_limit_error("max_instances must be a u32 integer"))?;
        let max_input_bytes = parse_u32_number(max_input_bytes)
            .ok_or_else(|| host_limit_error("max_input_bytes must be a u32 integer"))?;
        let max_initial_memory_entries = parse_u32_number(max_initial_memory_entries)
            .ok_or_else(|| host_limit_error("max_initial_memory_entries must be a u32 integer"))?;
        let max_request_bytes = parse_u32_number(max_request_bytes)
            .ok_or_else(|| host_limit_error("max_request_bytes must be a u32 integer"))?;
        let max_response_bytes = parse_u32_number(max_response_bytes)
            .filter(|maximum| *maximum >= MIN_RESPONSE_BYTES)
            .ok_or_else(|| host_limit_error("max_response_bytes must be a u32 of at least 256"))?;
        let max_instance_state_bytes = parse_u32_number(max_instance_state_bytes)
            .filter(|maximum| *maximum >= MIN_INSTANCE_STATE_BYTES)
            .ok_or_else(|| {
                host_limit_error("max_instance_state_bytes must be a u32 of at least 256")
            })?;
        let max_run_ticks_per_call = bounded_string(&max_run_ticks_per_call, 20)
            .map_err(|_| host_limit_error("max_run_ticks_per_call must be a decimal string"))?;
        let max_total_ticks_per_instance = bounded_string(&max_total_ticks_per_instance, 20)
            .map_err(|_| {
                host_limit_error("max_total_ticks_per_instance must be a decimal string")
            })?;
        let max_work_units_per_call = bounded_string(&max_work_units_per_call, 20)
            .map_err(|_| host_limit_error("max_work_units_per_call must be a decimal string"))?;
        Self::with_limits(
            max_source_bytes,
            max_compiled_programs,
            max_instances,
            max_input_bytes,
            max_initial_memory_entries,
            max_request_bytes,
            max_response_bytes,
            max_instance_state_bytes,
            &max_run_ticks_per_call,
            &max_total_ticks_per_instance,
            &max_work_units_per_call,
        )
        .map_err(|message| host_limit_error(&message))
    }

    /// Returns the host-neutral API version understood by this adapter.
    pub fn api_version(&self) -> u32 {
        self.runtime.api_version()
    }

    /// Returns versioned diagnostics for an in-memory source string.
    pub fn check(&self, source: JsValue) -> String {
        let maximum = as_u32_limit(self.runtime.limits().max_source_bytes());
        match bounded_string(&source, maximum) {
            Ok(source) => self.check_source(source),
            Err(error) => {
                bounded_input_error_json("source", error, self.max_response_bytes as usize)
            }
        }
    }

    /// Compiles Full source and returns only a runtime-local program handle.
    pub fn compile(&mut self, source: JsValue) -> String {
        let maximum = as_u32_limit(self.runtime.limits().max_source_bytes());
        match bounded_string(&source, maximum) {
            Ok(source) => self.compile_source(source),
            Err(error) => {
                bounded_input_error_json("source", error, self.max_response_bytes as usize)
            }
        }
    }

    /// Returns the canonical read-only projection for a live program handle.
    pub fn program_view(&self, program: JsValue) -> String {
        let Some(program) = bounded_string(&program, 20).ok() else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.program_view_handle(&program)
    }

    /// Creates an isolated VM instance from a compiled handle.
    ///
    /// Input is passed as a JavaScript `Uint8Array`. Initial memory and
    /// configuration are bounded JSON strings; all wide integers in them are
    /// canonical decimal strings.
    pub fn create_instance(
        &mut self,
        program: JsValue,
        input: JsValue,
        initial_memory_json: JsValue,
        configuration_json: JsValue,
    ) -> String {
        let measured = measure_create_request_bytes_up_to(
            &program,
            &input,
            &initial_memory_json,
            &configuration_json,
            f64::from(self.max_request_bytes),
        );
        if measured < 0.0 {
            let (code, message) = match measured as i32 {
                -1 => ("invalid_program_handle", "program must be a decimal string"),
                -2 => ("invalid_input", "input must be a Uint8Array"),
                -3 => (
                    "invalid_initial_memory",
                    "initial_memory must be a JSON string",
                ),
                -4 => (
                    "invalid_configuration",
                    "configuration must be a JSON string",
                ),
                _ => (
                    "invalid_request",
                    "create-instance request has invalid fields",
                ),
            };
            return input_error_json(code, message, self.max_response_bytes as usize);
        }
        if measured > f64::from(self.max_request_bytes) {
            return bounded_input_error_json(
                "request",
                BoundedInputError::TooLarge,
                self.max_response_bytes as usize,
            );
        }
        let Some(program) = bounded_string(&program, 20).ok() else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let input = match bounded_byte_array(
            &input,
            as_u32_limit(self.runtime.limits().max_input_bytes()),
        ) {
            Ok(input) => input,
            Err(error) => {
                return bounded_input_error_json("input", error, self.max_response_bytes as usize)
            }
        };
        let (initial_memory_json, configuration_json) = match bounded_string_pair(
            &initial_memory_json,
            &configuration_json,
            self.max_request_bytes,
        ) {
            Ok(values) => values,
            Err(BoundedInputError::TooLarge) => {
                return bounded_input_error_json(
                    "request",
                    BoundedInputError::TooLarge,
                    self.max_response_bytes as usize,
                )
            }
            Err(_) => {
                return input_error_json(
                    "invalid_instance_configuration",
                    "initial_memory and configuration must be JSON strings",
                    self.max_response_bytes as usize,
                )
            }
        };
        self.create_instance_data(program, input, initial_memory_json, configuration_json)
    }

    /// Executes exactly one outer Global Tick.
    pub fn step(&mut self, instance: JsValue) -> String {
        let Some(instance) = bounded_string(&instance, 20).ok() else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.step_handle(&instance)
    }

    /// Runs a bounded tick slice and returns a full resynchronization snapshot.
    pub fn run(&mut self, instance: JsValue, max_ticks: JsValue) -> String {
        let Some(instance) = bounded_string(&instance, 20).ok() else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let Some(max_ticks) = bounded_string(&max_ticks, 20).ok() else {
            return input_error_json(
                "invalid_tick_limit",
                "max_ticks must be a u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.run_handle(&instance, &max_ticks)
    }

    /// Reads a full snapshot without advancing the VM.
    pub fn snapshot(&mut self, instance: JsValue) -> String {
        let Some(instance) = bounded_string(&instance, 20).ok() else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.snapshot_handle(&instance)
    }

    /// Releases a runtime-local compiled-program handle.
    pub fn release_program(&mut self, program: JsValue) -> String {
        let Some(program) = bounded_string(&program, 20).ok() else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.release_program_handle(&program)
    }

    /// Releases a runtime-local VM instance.
    pub fn release_instance(&mut self, instance: JsValue) -> String {
        let Some(instance) = bounded_string(&instance, 20).ok() else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.release_instance_handle(&instance)
    }
}

impl BrowserRuntime {
    fn with_limits(
        max_source_bytes: u32,
        max_compiled_programs: u32,
        max_instances: u32,
        max_input_bytes: u32,
        max_initial_memory_entries: u32,
        max_request_bytes: u32,
        max_response_bytes: u32,
        max_instance_state_bytes: u32,
        max_run_ticks_per_call: &str,
        max_total_ticks_per_instance: &str,
        max_work_units_per_call: &str,
    ) -> Result<BrowserRuntime, String> {
        let max_run_ticks_per_call = parse_tick_limit(max_run_ticks_per_call).ok_or_else(|| {
            "max_run_ticks_per_call must be a positive u64 decimal string".to_owned()
        })?;
        let max_total_ticks_per_instance = parse_tick_limit(max_total_ticks_per_instance)
            .ok_or_else(|| {
                "max_total_ticks_per_instance must be a positive u64 decimal string".to_owned()
            })?;
        let max_work_units_per_call =
            parse_tick_limit(max_work_units_per_call).ok_or_else(|| {
                "max_work_units_per_call must be a positive u64 decimal string".to_owned()
            })?;

        if max_response_bytes < MIN_RESPONSE_BYTES {
            return Err("max_response_bytes must be at least 256".to_owned());
        }
        if max_instance_state_bytes < MIN_INSTANCE_STATE_BYTES {
            return Err("max_instance_state_bytes must be at least 256".to_owned());
        }

        Ok(Self {
            runtime: RuntimeApi::new(HostLimits::new(
                max_source_bytes as usize,
                max_compiled_programs as usize,
                max_instances as usize,
                max_input_bytes as usize,
                max_initial_memory_entries as usize,
                max_run_ticks_per_call,
                max_total_ticks_per_instance,
                max_work_units_per_call,
            )),
            max_request_bytes,
            max_response_bytes,
            max_instance_state_bytes,
        })
    }

    fn json_response(&self, value: Value) -> String {
        serialize_bounded_json(&value, self.max_response_bytes as usize)
    }

    fn api_error(&self, error: ApiError) -> String {
        api_error_json(error, self.max_response_bytes as usize)
    }

    fn instance_state_error(&self) -> String {
        serialize_bounded_json(
            &json!({
                "api_version": RUNTIME_API_VERSION,
                "error": {
                    "code": "instance_state_limit_exceeded",
                    "error_number": error_number("browser", "instance_state_limit_exceeded"),
                    "details": { "maximum_bytes": self.max_instance_state_bytes.to_string() },
                },
            }),
            self.max_response_bytes as usize,
        )
    }

    fn instance_snapshot_view_fits(&self, snapshot: &RuntimeSnapshotView<'_>) -> bool {
        json_fits_byte_limit(
            &SnapshotProjection::new(snapshot),
            self.max_instance_state_bytes as usize,
        )
    }

    fn discard_instance(&mut self, instance: InstanceHandle) {
        let _ = self.runtime.release_instance(ReleaseInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        });
    }

    fn discard_program(&mut self, program: ProgramHandle) {
        let _ = self.runtime.release_program(ReleaseProgramRequest {
            api_version: RUNTIME_API_VERSION,
            program,
        });
    }

    fn check_source(&self, source: String) -> String {
        match self.runtime.check(CheckRequest {
            api_version: RUNTIME_API_VERSION,
            source,
        }) {
            Ok(response) => serialize_bounded_json(
                &CheckResponseProjection {
                    api_version: response.api_version,
                    diagnostics: &response.diagnostics,
                },
                self.max_response_bytes as usize,
            ),
            Err(error) => self.api_error(error),
        }
    }

    fn compile_source(&mut self, source: String) -> String {
        match self.runtime.compile(CompileRequest {
            api_version: RUNTIME_API_VERSION,
            source,
        }) {
            Ok(response) => match response.outcome {
                CompileOutcome::Compiled { program } => {
                    let projection = CompileProjection {
                        api_version: response.api_version,
                        program: program.get(),
                    };
                    if json_fits_byte_limit(&projection, self.max_response_bytes as usize) {
                        serialize_bounded_json(&projection, self.max_response_bytes as usize)
                    } else {
                        self.discard_program(program);
                        RESPONSE_LIMIT_ERROR.to_owned()
                    }
                }
                CompileOutcome::Diagnostics { items } => serialize_bounded_json(
                    &CompileDiagnosticsProjection {
                        api_version: response.api_version,
                        diagnostics: &items,
                    },
                    self.max_response_bytes as usize,
                ),
            },
            Err(error) => self.api_error(error),
        }
    }

    fn program_view_handle(&self, program: &str) -> String {
        let Some(program) = parse_program_handle(program) else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        match self.runtime.program_view(ProgramViewRequest {
            api_version: RUNTIME_API_VERSION,
            program,
        }) {
            Ok(response) => serialize_bounded_json(
                &ProgramViewResponseProjection {
                    api_version: response.api_version,
                    view: &response.view,
                },
                self.max_response_bytes as usize,
            ),
            Err(error) => self.api_error(error),
        }
    }

    fn create_instance_data(
        &mut self,
        program: String,
        input: Vec<u8>,
        initial_memory_json: String,
        configuration_json: String,
    ) -> String {
        let Some(program) = parse_program_handle(&program) else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let wire_memory: Vec<MemoryEntryWire> = match serde_json::from_str(&initial_memory_json) {
            Ok(entries) => entries,
            Err(_) => {
                return input_error_json(
                    "invalid_initial_memory",
                    "initial_memory must be a JSON array of {address, value} entries",
                    self.max_response_bytes as usize,
                )
            }
        };
        let initial_memory_limit = self.runtime.limits().max_initial_memory_entries();
        if wire_memory.len() > initial_memory_limit {
            return self.api_error(ApiError::InitialMemoryLimitExceeded {
                received_entries: wire_memory.len(),
                maximum: initial_memory_limit,
            });
        }
        let mut initial_memory = Vec::with_capacity(wire_memory.len());
        for entry in wire_memory {
            let Some(address) = parse_canonical_memory_address(&entry.address) else {
                return input_error_json(
                    "invalid_memory_address",
                    "memory addresses must be canonical signed decimal strings",
                    self.max_response_bytes as usize,
                );
            };
            initial_memory.push(MemoryEntry {
                address,
                value: entry.value,
            });
        }
        let configuration: ConfigurationWire =
            match serde_json::from_str(&configuration_json) {
                Ok(configuration) => configuration,
                Err(_) => return input_error_json(
                    "invalid_configuration",
                    "configuration must contain boundary_mode, seed, and custom_execution_limit",
                    self.max_response_bytes as usize,
                ),
            };
        let boundary_mode = match configuration.boundary_mode.as_str() {
            "exit" => BoundaryMode::Exit,
            "wrap" => BoundaryMode::Wrap,
            _ => {
                return input_error_json(
                    "invalid_boundary_mode",
                    "boundary_mode must be exit or wrap",
                    self.max_response_bytes as usize,
                )
            }
        };
        let (Some(seed), Some(custom_execution_limit)) = (
            parse_canonical_u64(&configuration.seed),
            parse_canonical_u64(&configuration.custom_execution_limit),
        ) else {
            return input_error_json(
                "invalid_configuration_integer",
                "seed and custom_execution_limit must be canonical u64 decimal strings",
                self.max_response_bytes as usize,
            );
        };
        match self.runtime.create_instance(CreateInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            program,
            input,
            initial_memory,
            configuration: RuntimeConfiguration {
                boundary_mode,
                seed,
                custom_execution_limit,
            },
        }) {
            Ok(response) => {
                let snapshot_fits = self
                    .runtime
                    .snapshot_projection_view(SnapshotRequest {
                        api_version: RUNTIME_API_VERSION,
                        instance: response.instance,
                    })
                    .map(|snapshot| self.instance_snapshot_view_fits(&snapshot));
                match snapshot_fits {
                    Ok(true) => self.json_response(json!({
                        "api_version": response.api_version,
                        "instance": response.instance.get().to_string(),
                    })),
                    Ok(false) => {
                        self.discard_instance(response.instance);
                        self.instance_state_error()
                    }
                    Err(error) => {
                        self.discard_instance(response.instance);
                        self.api_error(error)
                    }
                }
            }
            Err(error) => self.api_error(error),
        }
    }

    fn step_handle(&mut self, instance: &str) -> String {
        let Some(instance) = parse_instance_handle(instance) else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let max_state_bytes = self.max_instance_state_bytes as usize;
        let max_response_bytes = self.max_response_bytes as usize;
        match self.runtime.step_view(StepRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        }) {
            Ok(response) => {
                if !json_fits_byte_limit(
                    &SnapshotProjection::new(response.snapshot()),
                    max_state_bytes,
                ) {
                    drop(response);
                    self.discard_instance(instance);
                    return self.instance_state_error();
                }
                let projection = StepResponseProjection {
                    response: &response,
                };
                serialize_bounded_json(&projection, max_response_bytes)
            }
            Err(error) => self.api_error(error),
        }
    }

    fn run_handle(&mut self, instance: &str, max_ticks: &str) -> String {
        let Some(instance) = parse_instance_handle(instance) else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let Some(max_ticks) = parse_u64(max_ticks) else {
            return input_error_json(
                "invalid_tick_limit",
                "max_ticks must be a u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let max_state_bytes = self.max_instance_state_bytes as usize;
        let max_response_bytes = self.max_response_bytes as usize;
        match self.runtime.run_view(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_ticks,
        }) {
            Ok(response)
                if !json_fits_byte_limit(
                    &SnapshotProjection::new(&response.snapshot),
                    max_state_bytes,
                ) =>
            {
                drop(response);
                self.discard_instance(instance);
                self.instance_state_error()
            }
            Ok(response) => serialize_bounded_json(
                &RunProjection {
                    api_version: response.api_version,
                    status: response.status,
                    events: &response.events,
                    newly_emitted_output: &response.newly_emitted_output,
                    snapshot: SnapshotProjection::new(&response.snapshot),
                },
                max_response_bytes,
            ),
            Err(error) => self.api_error(error),
        }
    }

    fn snapshot_handle(&mut self, instance: &str) -> String {
        let Some(instance) = parse_instance_handle(instance) else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let snapshot = self.runtime.snapshot_projection_view(SnapshotRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        });
        match snapshot {
            Ok(snapshot) => {
                if self.instance_snapshot_view_fits(&snapshot) {
                    let projection = SnapshotResponseProjection {
                        api_version: RUNTIME_API_VERSION,
                        snapshot: SnapshotProjection::new(&snapshot),
                    };
                    serialize_bounded_json(&projection, self.max_response_bytes as usize)
                } else {
                    self.discard_instance(instance);
                    self.instance_state_error()
                }
            }
            Err(error) => self.api_error(error),
        }
    }

    fn release_program_handle(&mut self, program: &str) -> String {
        let Some(program) = parse_program_handle(program) else {
            return input_error_json(
                "invalid_program_handle",
                "program must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        match self.runtime.release_program(ReleaseProgramRequest {
            api_version: RUNTIME_API_VERSION,
            program,
        }) {
            Ok(response) => self.json_response(json!({ "api_version": response.api_version })),
            Err(error) => self.api_error(error),
        }
    }

    fn release_instance_handle(&mut self, instance: &str) -> String {
        let Some(instance) = parse_instance_handle(instance) else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        match self.runtime.release_instance(ReleaseInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        }) {
            Ok(response) => self.json_response(json!({ "api_version": response.api_version })),
            Err(error) => self.api_error(error),
        }
    }
}

fn parse_tick_limit(value: &str) -> Option<NonZeroU64> {
    parse_canonical_u64(value).and_then(NonZeroU64::new)
}

fn parse_u64(value: &str) -> Option<u64> {
    parse_canonical_u64(value)
}

fn parse_canonical_u64(value: &str) -> Option<u64> {
    if value == "0" {
        return Some(0);
    }
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse::<u64>().ok()
}

fn parse_canonical_memory_address(value: &str) -> Option<MemoryAddress> {
    if value == "0" {
        return Some(MemoryAddress::from(0));
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || value == "-0"
    {
        return None;
    }
    MemoryAddress::from_str(value).ok()
}

fn parse_program_handle(value: &str) -> Option<ProgramHandle> {
    ProgramHandle::from_raw(parse_u64(value)?)
}

fn parse_instance_handle(value: &str) -> Option<InstanceHandle> {
    InstanceHandle::from_raw(parse_u64(value)?)
}

fn input_error_json(code: &str, message: &str, maximum: usize) -> String {
    serialize_bounded_json(
        &json!({
            "api_version": RUNTIME_API_VERSION,
            "error": { "code": code, "error_number": error_number("browser", code), "message": message },
        }),
        maximum,
    )
}

macro_rules! serialize_object {
    ($serializer:expr, { $($key:literal => $value:expr),+ $(,)? }) => {{
        let mut object = $serializer.serialize_map(None)?;
        $(object.serialize_entry($key, &$value)?;)+
        object.end()
    }};
}

struct CheckResponseProjection<'a> {
    api_version: u32,
    diagnostics: &'a [Diagnostic],
}

impl Serialize for CheckResponseProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "diagnostics" => DiagnosticSequence(self.diagnostics),
        })
    }
}

struct CompileDiagnosticsProjection<'a> {
    api_version: u32,
    diagnostics: &'a [Diagnostic],
}

impl Serialize for CompileDiagnosticsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "outcome" => DiagnosticOutcomeProjection(self.diagnostics),
        })
    }
}

struct DiagnosticOutcomeProjection<'a>(&'a [Diagnostic]);

impl Serialize for DiagnosticOutcomeProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "kind" => "diagnostics",
            "items" => DiagnosticSequence(self.0),
        })
    }
}

struct DiagnosticSequence<'a>(&'a [Diagnostic]);

impl Serialize for DiagnosticSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for diagnostic in self.0 {
            sequence.serialize_element(&DiagnosticProjection(diagnostic))?;
        }
        sequence.end()
    }
}

struct DiagnosticProjection<'a>(&'a Diagnostic);

impl Serialize for DiagnosticProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code" => self.0.code,
            "error_number" => codegrid_runtime_api::error_number("source", self.0.code).or_else(|| codegrid_runtime_api::error_number("ir", self.0.code)),
            "severity" => match self.0.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            "message" => &self.0.message,
            "span" => DiagnosticSpanProjection {
                start: self.0.span.start,
                end: self.0.span.end,
            },
        })
    }
}

struct DiagnosticSpanProjection {
    start: usize,
    end: usize,
}

impl Serialize for DiagnosticSpanProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "start" => DisplayValue(self.start),
            "end" => DisplayValue(self.end),
        })
    }
}

struct DisplayValue<T>(T);

impl<T: Display> Serialize for DisplayValue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

struct CompileProjection {
    api_version: u32,
    program: u64,
}

impl Serialize for CompileProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(2))?;
        object.serialize_entry("api_version", &self.api_version)?;
        object.serialize_entry("outcome", &CompiledOutcomeProjection(self.program))?;
        object.end()
    }
}

struct CompiledOutcomeProjection(u64);

impl Serialize for CompiledOutcomeProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "kind" => "compiled",
            "program" => DisplayValue(self.0),
        })
    }
}

struct ProgramViewResponseProjection<'a> {
    api_version: u32,
    view: &'a ProgramView,
}

impl Serialize for ProgramViewResponseProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "view" => ProgramViewProjection(self.view),
        })
    }
}

struct ProgramViewProjection<'a>(&'a ProgramView);

impl Serialize for ProgramViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(3))?;
        object.serialize_entry("ir_format_version", &self.0.ir_format_version)?;
        object.serialize_entry("outer", &CodeGridViewProjection(&self.0.outer))?;
        object.serialize_entry("customs", &CodeGridMapProjection(&self.0.customs))?;
        object.end()
    }
}

struct CodeGridViewProjection<'a>(&'a CodeGridView);

impl Serialize for CodeGridViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "main" => BoardViewProjection(&self.0.main),
            "functions" => BoardViewMapProjection(&self.0.functions),
        })
    }
}

struct CodeGridMapProjection<'a>(&'a std::collections::BTreeMap<u8, CodeGridView>);

impl Serialize for CodeGridMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, grid) in self.0 {
            object.serialize_entry(&id.to_string(), &CodeGridViewProjection(grid))?;
        }
        object.end()
    }
}

struct BoardViewMapProjection<'a>(&'a std::collections::BTreeMap<u8, BoardView>);

impl Serialize for BoardViewMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, board) in self.0 {
            object.serialize_entry(&id.to_string(), &BoardViewProjection(board))?;
        }
        object.end()
    }
}

struct BoardViewProjection<'a>(&'a BoardView);

impl Serialize for BoardViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "width" => DisplayValue(self.0.width),
            "height" => DisplayValue(self.0.height),
            "cells" => CellViewSequence(&self.0.cells),
            "folded_blocks" => FoldedViewMapProjection(&self.0.folded_blocks),
        })
    }
}

struct CellViewSequence<'a>(&'a [codegrid_runtime_api::CellView]);

impl Serialize for CellViewSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for cell in self.0 {
            sequence.serialize_element(&CellViewProjection(cell))?;
        }
        sequence.end()
    }
}

struct CellViewProjection<'a>(&'a codegrid_runtime_api::CellView);

impl Serialize for CellViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "prefix" => &self.0.prefix,
            "entry" => &self.0.entry,
            "primary" => &self.0.primary,
            "attachment" => &self.0.attachment,
        })
    }
}

struct FoldedViewMapProjection<'a>(&'a std::collections::BTreeMap<u8, Vec<Option<String>>>);

impl Serialize for FoldedViewMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, cells) in self.0 {
            object.serialize_entry(&id.to_string(), cells)?;
        }
        object.end()
    }
}

struct SnapshotProjection<'snapshot, 'runtime> {
    snapshot: &'snapshot RuntimeSnapshotView<'runtime>,
}

impl<'snapshot, 'runtime> SnapshotProjection<'snapshot, 'runtime> {
    fn new(snapshot: &'snapshot RuntimeSnapshotView<'runtime>) -> Self {
        Self { snapshot }
    }
}

impl Serialize for SnapshotProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let snapshot = self.snapshot;
        let runtime_program = CodeGridView::from_scoped(snapshot.runtime_program());
        let mut object = serializer.serialize_map(Some(11))?;
        object.serialize_entry("status", vm_status_name(snapshot.status()))?;
        object.serialize_entry("committed_ticks", &DisplayValue(snapshot.committed_ticks()))?;
        object.serialize_entry("registers", snapshot.registers())?;
        object.serialize_entry("memory", &SnapshotMemorySequence(snapshot))?;
        object.serialize_entry("remaining_input", &InputSequence(snapshot))?;
        object.serialize_entry("output", snapshot.output())?;
        object.serialize_entry("runtime_program", &CodeGridViewProjection(&runtime_program))?;
        object.serialize_entry("threads", &SnapshotThreadSequence(snapshot))?;
        object.serialize_entry("metrics", &RuntimeMetricsProjection(snapshot.metrics()))?;
        object.serialize_entry("errors", &RuntimeErrorSequence(snapshot.errors()))?;
        object.serialize_entry("fault", &OptionalFaultProjection(snapshot.fault()))?;
        object.end()
    }
}

struct SnapshotMemorySequence<'a, 'runtime>(&'a RuntimeSnapshotView<'runtime>);

impl Serialize for SnapshotMemorySequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let memory = self.0.memory();
        let mut sequence = serializer.serialize_seq(None)?;
        for address in memory.allocated_addresses() {
            sequence.serialize_element(&MemoryEntryProjection {
                address,
                value: memory.read(address),
            })?;
        }
        sequence.end()
    }
}

struct MemoryEntryProjection<'a> {
    address: &'a MemoryAddress,
    value: u8,
}

impl Serialize for MemoryEntryProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "address" => DisplayValue(self.address),
            "value" => self.value,
        })
    }
}

struct SnapshotThreadSequence<'a, 'runtime>(&'a RuntimeSnapshotView<'runtime>);

impl Serialize for SnapshotThreadSequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for thread in self.0.threads() {
            sequence.serialize_element(&RuntimeThreadProjection(thread))?;
        }
        sequence.end()
    }
}

struct InstructionCodes<I>(I);

impl<I> Serialize for InstructionCodes<I>
where
    I: Iterator<Item = u8> + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for code in self.0.clone() {
            sequence.serialize_element(&code)?;
        }
        sequence.end()
    }
}

struct CallFrameSequence<'a>(&'a [CallFrameSnapshot]);

impl Serialize for CallFrameSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for frame in self.0 {
            sequence.serialize_element(&CallFrameProjection(*frame))?;
        }
        sequence.end()
    }
}

struct ThreadProjection<'a, I> {
    code_grid: CodeGridId,
    id: u64,
    board: BoardId,
    position: Coordinate,
    direction: Direction,
    register_pointer: u8,
    page: &'a codegrid_runtime_api::Page,
    data_stack: &'a [u8],
    instruction_stack: InstructionCodes<I>,
    call_frames: Vec<CallFrameSnapshot>,
    phase: ThreadPhaseSnapshot,
    random_state: u64,
}

impl<'a, I> ThreadProjection<'a, I>
where
    I: Iterator<Item = u8> + Clone,
{
    fn from_fields(
        code_grid: CodeGridId,
        id: u64,
        board: BoardId,
        position: Coordinate,
        direction: Direction,
        register_pointer: u8,
        page: &'a codegrid_runtime_api::Page,
        data_stack: &'a [u8],
        instruction_stack: I,
        call_frames: Vec<CallFrameSnapshot>,
        phase: ThreadPhaseSnapshot,
        random_state: u64,
    ) -> Self {
        Self {
            code_grid,
            id,
            board,
            position,
            direction,
            register_pointer,
            page,
            data_stack,
            instruction_stack: InstructionCodes(instruction_stack),
            call_frames,
            phase,
            random_state,
        }
    }
}

struct RuntimeThreadProjection<'a>(RuntimeThreadSnapshotView<'a>);

impl Serialize for RuntimeThreadProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let thread = &self.0;
        let call_frames = thread.call_frames().collect::<Vec<_>>();
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(thread.code_grid()),
            "id" => DisplayValue(thread.id()),
            "board" => BoardIdProjection(thread.board()),
            "position" => CoordinateProjection(thread.position()),
            "direction" => direction_name(thread.direction()),
            "register_pointer" => thread.register_pointer(),
            "page" => DisplayValue(thread.page()),
            "data_stack" => thread.data_stack(),
            "instruction_stack" => InstructionCodes(thread.instruction_stack().iter().map(|item| item.code())),
            "call_frames" => CallFrameSequence(&call_frames),
            "phase" => ThreadPhaseProjection(thread.phase()),
            "random_state" => DisplayValue(thread.random_state()),
        })
    }
}

impl<I> Serialize for ThreadProjection<'_, I>
where
    I: Iterator<Item = u8> + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(self.code_grid),
            "id" => DisplayValue(self.id),
            "board" => BoardIdProjection(self.board),
            "position" => CoordinateProjection(self.position),
            "direction" => direction_name(self.direction),
            "register_pointer" => self.register_pointer,
            "page" => DisplayValue(self.page),
            "data_stack" => self.data_stack,
            "instruction_stack" => &self.instruction_stack,
            "call_frames" => CallFrameSequence(&self.call_frames),
            "phase" => ThreadPhaseProjection(self.phase),
            "random_state" => DisplayValue(self.random_state),
        })
    }
}

struct CallFrameProjection(CallFrameSnapshot);

impl Serialize for CallFrameProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "caller_board" => BoardIdProjection(self.0.caller_board),
            "call_position" => CoordinateProjection(self.0.call_position),
            "saved_direction" => direction_name(self.0.saved_direction),
        })
    }
}

struct ThreadPhaseProjection(ThreadPhaseSnapshot);

impl Serialize for ThreadPhaseProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ThreadPhaseSnapshot::Normal => serialize_object!(serializer, { "kind" => "normal" }),
            ThreadPhaseSnapshot::AfterCall => {
                serialize_object!(serializer, { "kind" => "after_call" })
            }
            ThreadPhaseSnapshot::Repeat { total, completed } => serialize_object!(serializer, {
                "kind" => "repeat",
                "total" => total,
                "completed" => completed,
            }),
            ThreadPhaseSnapshot::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold",
                "fold_id" => fold_id.get(),
                "internal_position" => CoordinateProjection(internal_position),
                "internal_direction" => direction_name(internal_direction),
                "saved_outer_direction" => direction_name(saved_outer_direction),
            }),
            ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold_resume",
                "saved_outer_direction" => direction_name(saved_outer_direction),
            }),
            ThreadPhaseSnapshot::Terminated => {
                serialize_object!(serializer, { "kind" => "terminated" })
            }
        }
    }
}
struct SnapshotResponseProjection<'snapshot, 'runtime> {
    api_version: u32,
    snapshot: SnapshotProjection<'snapshot, 'runtime>,
}

impl Serialize for SnapshotResponseProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "snapshot" => &self.snapshot,
        })
    }
}

struct RunProjection<'snapshot, 'runtime> {
    api_version: u32,
    status: RunStatus,
    events: &'snapshot [VmEvent],
    newly_emitted_output: &'snapshot [u8],
    snapshot: SnapshotProjection<'snapshot, 'runtime>,
}

impl Serialize for RunProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(5))?;
        object.serialize_entry("api_version", &self.api_version)?;
        object.serialize_entry("status", &RunStatusProjection(self.status))?;
        object.serialize_entry("events", &VmEventSequence(self.events))?;
        object.serialize_entry("newly_emitted_output", self.newly_emitted_output)?;
        object.serialize_entry("snapshot", &self.snapshot)?;
        object.end()
    }
}

struct RunStatusProjection(RunStatus);

impl Serialize for RunStatusProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            RunStatus::Halted => serialize_object!(serializer, { "kind" => "halted" }),
            RunStatus::Error => serialize_object!(serializer, { "kind" => "error" }),
            RunStatus::Yielded { reason } => {
                let reason = match reason {
                    YieldReason::TickSliceExhausted => "tick_slice_exhausted",
                    YieldReason::WorkUnitBudgetExhausted => "work_unit_budget_exhausted",
                };
                serialize_object!(serializer, {
                    "kind" => "yielded",
                    "reason" => reason,
                })
            }
        }
    }
}

struct StepResponseProjection<'a, 'view> {
    response: &'a StepResponseView<'view>,
}

impl Serialize for StepResponseProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.response.api_version(),
            "result" => StepResultProjection(self.response),
        })
    }
}

struct StepResultProjection<'a, 'view>(&'a StepResponseView<'view>);

impl Serialize for StepResultProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let result = self.0;
        serialize_object!(serializer, {
            "attempted_tick" => DisplayValue(result.attempted_tick()),
            "committed_ticks" => DisplayValue(result.committed_ticks()),
            "status" => vm_status_name(result.status()),
            "metrics" => RuntimeMetricSummaryProjection(result.metrics()),
            "events" => VmEventSequence(result.events()),
            "newly_emitted_output" => result.newly_emitted_output(),
            "errors" => RuntimeErrorSequence(result.errors()),
            "fault" => OptionalFaultProjection(result.fault()),
            "snapshot" => SnapshotProjection::new(result.snapshot()),
        })
    }
}

struct InputSequence<'snapshot, 'runtime>(&'snapshot RuntimeSnapshotView<'runtime>);

impl Serialize for InputSequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in self.0.remaining_input() {
            sequence.serialize_element(value)?;
        }
        sequence.end()
    }
}

struct CoordinateProjection(Coordinate);

impl Serialize for CoordinateProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "x" => self.0.x,
            "y" => self.0.y,
        })
    }
}

struct CodeGridIdProjection(CodeGridId);

impl Serialize for CodeGridIdProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            CodeGridId::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            CodeGridId::Custom(id) => serialize_object!(serializer, {
                "kind" => "custom",
                "id" => id.get(),
            }),
        }
    }
}

struct BoardIdProjection(BoardId);

impl Serialize for BoardIdProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            BoardId::Main => serialize_object!(serializer, { "kind" => "main" }),
            BoardId::Function(id) => serialize_object!(serializer, {
                "kind" => "function",
                "id" => id.get(),
            }),
        }
    }
}

struct ScopeProjection(ExecutionScope);

impl Serialize for ScopeProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ExecutionScope::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            ExecutionScope::Custom {
                caller_thread_id,
                custom_id,
                internal_tick,
            } => serialize_object!(serializer, {
                "kind" => "custom",
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get(),
                "internal_tick" => DisplayValue(internal_tick),
            }),
        }
    }
}

struct StaticCellProjection(StaticCellId);

impl Serialize for StaticCellProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(self.0.code_grid),
            "board" => BoardIdProjection(self.0.board),
            "folded_block" => self.0.folded_block.map(|id| id.get()),
            "position" => CoordinateProjection(self.0.position),
        })
    }
}

struct MemorySpaceProjection(MemorySpaceId);

impl Serialize for MemorySpaceProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            MemorySpaceId::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            MemorySpaceId::CustomInvocation {
                global_tick,
                caller_thread_id,
                custom_id,
            } => serialize_object!(serializer, {
                "kind" => "custom_invocation",
                "global_tick" => DisplayValue(global_tick),
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get(),
            }),
        }
    }
}

struct MemoryLocationProjection<'a>(&'a MemoryLocationId);

impl Serialize for MemoryLocationProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "space" => MemorySpaceProjection(self.0.space),
            "address" => DisplayValue(&self.0.address),
        })
    }
}

struct SnapshotMemoryMetricSequence<'a>(&'a RuntimeMetrics);

impl Serialize for SnapshotMemoryMetricSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let addresses = self.0.used_memory_addresses();
        let mut sequence = serializer.serialize_seq(Some(addresses.len()))?;
        for location in addresses {
            sequence.serialize_element(&MemoryLocationProjection(location))?;
        }
        sequence.end()
    }
}

struct VmEventSequence<'a>(&'a [VmEvent]);

impl Serialize for VmEventSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for event in self.0 {
            sequence.serialize_element(&VmEventProjection(event))?;
        }
        sequence.end()
    }
}

struct VmEventProjection<'a>(&'a VmEvent);

impl Serialize for VmEventProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            VmEvent::CellReached {
                scope,
                thread_id,
                cell,
            } => serialize_object!(serializer, {
                "kind" => "cell_reached",
                "scope" => ScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "cell" => StaticCellProjection(*cell),
            }),
            VmEvent::InputConsumed {
                scope,
                thread_id,
                value,
            } => serialize_object!(serializer, {
                "kind" => "input_consumed",
                "scope" => ScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "value" => value,
            }),
            VmEvent::RegisterChanged {
                scope,
                register,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "register_changed",
                "scope" => ScopeProjection(*scope),
                "register" => register,
                "old" => old,
                "new" => new,
            }),
            VmEvent::MemoryChanged {
                scope,
                location,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "memory_changed",
                "scope" => ScopeProjection(*scope),
                "location" => MemoryLocationProjection(location),
                "old" => old,
                "new" => new,
            }),
            VmEvent::CodeChanged {
                scope,
                cell,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "code_changed",
                "scope" => ScopeProjection(*scope),
                "cell" => StaticCellProjection(*cell),
                "old" => old.map(|primary| primary.token()),
                "new" => new.map(|primary| primary.token()),
            }),
            VmEvent::ThreadChanged {
                scope,
                before,
                after,
            } => {
                let code_grid = match scope {
                    ExecutionScope::Outer => CodeGridId::Outer,
                    ExecutionScope::Custom { custom_id, .. } => CodeGridId::Custom(*custom_id),
                };
                let before = ThreadProjection::from_fields(
                    code_grid,
                    before.id,
                    before.board,
                    before.position,
                    before.direction,
                    before.register_pointer,
                    &before.page,
                    &before.data_stack,
                    before.instruction_stack.iter().map(|item| item.code()),
                    before.call_stack.clone(),
                    before.phase,
                    before.random_state,
                );
                let after = ThreadProjection::from_fields(
                    code_grid,
                    after.id,
                    after.board,
                    after.position,
                    after.direction,
                    after.register_pointer,
                    &after.page,
                    &after.data_stack,
                    after.instruction_stack.iter().map(|item| item.code()),
                    after.call_stack.clone(),
                    after.phase,
                    after.random_state,
                );
                serialize_object!(serializer, {
                    "kind" => "thread_changed",
                    "scope" => ScopeProjection(*scope),
                    "before" => before,
                    "after" => after,
                })
            }
        }
    }
}

struct RuntimeMetricsProjection<'a>(&'a RuntimeMetrics);

impl Serialize for RuntimeMetricsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "global_tick" => DisplayValue(metrics.global_tick()),
            "operation_count" => DisplayValue(metrics.operation_count()),
            "used_cell_count" => DisplayValue(metrics.used_cell_count()),
            "used_cells" => SnapshotUsedCellSequence(metrics),
            "used_memory_address_count" => DisplayValue(metrics.used_memory_address_count()),
            "used_memory_addresses" => SnapshotMemoryMetricSequence(metrics),
            "peak_data_stack_usage" => DisplayValue(metrics.peak_data_stack_usage()),
            "peak_instruction_stack_usage" => DisplayValue(metrics.peak_instruction_stack_usage()),
            "peak_call_stack_usage" => DisplayValue(metrics.peak_call_stack_usage()),
            "instruction_variety" => InstructionVarietySetSequence(metrics.instruction_variety()),
        })
    }
}

struct RuntimeMetricSummaryProjection<'a>(&'a RuntimeMetricSummary);

impl Serialize for RuntimeMetricSummaryProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "global_tick" => DisplayValue(metrics.global_tick()),
            "operation_count" => DisplayValue(metrics.operation_count()),
            "used_cell_count" => DisplayValue(metrics.used_cell_count()),
            "used_memory_address_count" => DisplayValue(metrics.used_memory_address_count()),
            "peak_data_stack_usage" => DisplayValue(metrics.peak_data_stack_usage()),
            "peak_instruction_stack_usage" => DisplayValue(metrics.peak_instruction_stack_usage()),
            "peak_call_stack_usage" => DisplayValue(metrics.peak_call_stack_usage()),
            "instruction_variety" => InstructionVarietySetSequence(metrics.instruction_variety()),
        })
    }
}

struct SnapshotUsedCellSequence<'a>(&'a RuntimeMetrics);

impl Serialize for SnapshotUsedCellSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let cells = self.0.used_cells();
        let mut sequence = serializer.serialize_seq(Some(cells.len()))?;
        for cell in cells {
            sequence.serialize_element(&StaticCellProjection(*cell))?;
        }
        sequence.end()
    }
}

struct InstructionVarietySetSequence<'a>(&'a std::collections::BTreeSet<InstructionKind>);

impl Serialize for InstructionVarietySetSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for instruction in self.0 {
            sequence.serialize_element(instruction.name())?;
        }
        sequence.end()
    }
}

struct RuntimeErrorSequence<'a>(&'a [RuntimeError]);

impl Serialize for RuntimeErrorSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for error in self.0 {
            sequence.serialize_element(&RuntimeErrorProjection(error))?;
        }
        sequence.end()
    }
}

struct RuntimeErrorProjection<'a>(&'a RuntimeError);

impl Serialize for RuntimeErrorProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code" => self.0.code(),
            "error_number" => codegrid_runtime_api::error_number("vm", self.0.code()),
            "global_tick" => DisplayValue(self.0.global_tick()),
            "scope" => ScopeProjection(self.0.scope()),
            "details" => RuntimeErrorDetailsProjection(self.0.kind()),
        })
    }
}

struct RuntimeErrorDetailsProjection<'a>(&'a RuntimeErrorKind);

impl Serialize for RuntimeErrorDetailsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            RuntimeErrorKind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCodeWriteConflict { cell, thread_ids } => {
                serialize_object!(serializer, {
                    "cell" => StaticCellProjection(*cell),
                    "thread_ids" => ThreadIdSequence(thread_ids),
                })
            }
            RuntimeErrorKind::ConcurrentInputConflict { thread_ids }
            | RuntimeErrorKind::ConcurrentOutputConflict { thread_ids } => {
                serialize_object!(serializer, {
                    "thread_ids" => ThreadIdSequence(thread_ids),
                })
            }
            RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                address,
                thread_ids,
            } => serialize_object!(serializer, {
                "address" => DisplayValue(address),
                "thread_ids" => ThreadIdSequence(thread_ids),
            }),
            RuntimeErrorKind::ConcurrentWriteConflict {
                register,
                thread_ids,
            } => serialize_object!(serializer, {
                "register" => register,
                "thread_ids" => ThreadIdSequence(thread_ids),
            }),
            RuntimeErrorKind::CustomExecutionLimitExceeded { limit } => {
                serialize_object!(serializer, { "limit" => DisplayValue(limit) })
            }
            RuntimeErrorKind::OutOfBounds {
                thread_id,
                board,
                position,
                direction,
            } => serialize_object!(serializer, {
                "thread_id" => DisplayValue(thread_id),
                "board" => BoardIdProjection(*board),
                "position" => CoordinateProjection(*position),
                "direction" => direction_name(*direction),
            }),
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id,
                board,
                position,
            } => serialize_object!(serializer, {
                "thread_id" => DisplayValue(thread_id),
                "board" => BoardIdProjection(*board),
                "position" => CoordinateProjection(*position),
            }),
        }
    }
}

struct ThreadIdSequence<'a>(&'a [u64]);

impl Serialize for ThreadIdSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for id in self.0 {
            sequence.serialize_element(&DisplayValue(*id))?;
        }
        sequence.end()
    }
}

struct OptionalFaultProjection(Option<VmFault>);

impl Serialize for OptionalFaultProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Some(fault) => serializer.serialize_some(&FaultProjection(fault)),
            None => serializer.serialize_none(),
        }
    }
}

struct FaultProjection(VmFault);

impl Serialize for FaultProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            VmFault::MetricCounterOverflow(counter) => serialize_object!(serializer, {
                "kind" => "metric_counter_overflow", "error_number" => codegrid_runtime_api::error_number("fault", "metric_counter_overflow"),
                "counter" => metric_counter_name(counter),
            }),
            VmFault::GlobalTickOverflow => {
                serialize_object!(serializer, { "kind" => "global_tick_overflow", "error_number" => codegrid_runtime_api::error_number("fault", "global_tick_overflow") })
            }
            VmFault::InternalInvariantViolation => {
                serialize_object!(serializer, { "kind" => "internal_invariant_violation", "error_number" => codegrid_runtime_api::error_number("fault", "internal_invariant_violation") })
            }
        }
    }
}

#[cfg(test)]
fn program_view_json(view: &ProgramView) -> Value {
    let customs = view
        .customs
        .iter()
        .map(|(id, grid)| (id.to_string(), code_grid_view_json(grid)))
        .collect::<Map<String, Value>>();

    json!({
        "ir_format_version": view.ir_format_version,
        "outer": code_grid_view_json(&view.outer),
        "customs": customs,
    })
}

#[cfg(test)]
fn code_grid_view_json(view: &CodeGridView) -> Value {
    let functions = view
        .functions
        .iter()
        .map(|(id, board)| (id.to_string(), board_view_json(board)))
        .collect::<Map<String, Value>>();
    json!({
        "main": board_view_json(&view.main),
        "functions": functions,
    })
}

#[cfg(test)]
fn board_view_json(board: &BoardView) -> Value {
    let folded_blocks = board
        .folded_blocks
        .iter()
        .map(|(id, cells)| (id.to_string(), json!(cells)))
        .collect::<Map<String, Value>>();
    json!({
        "width": board.width.to_string(),
        "height": board.height.to_string(),
        "cells": board.cells.iter().map(|cell| json!({
            "entry": cell.entry,
            "primary": cell.primary,
            "prefix": cell.prefix,
            "attachment": cell.attachment,
        })).collect::<Vec<_>>(),
        "folded_blocks": folded_blocks,
    })
}

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

fn vm_status_name(status: VmStatus) -> &'static str {
    match status {
        VmStatus::Running => "running",
        VmStatus::Halted => "halted",
        VmStatus::Error => "error",
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

fn api_error_json(error: ApiError, maximum: usize) -> String {
    let (code, details) = match error {
        ApiError::UnsupportedVersion {
            received,
            supported,
        } => (
            "unsupported_version",
            json!({ "received": received, "supported": supported }),
        ),
        ApiError::SourceLimitExceeded {
            received_bytes,
            maximum,
        } => (
            "source_limit_exceeded",
            json!({ "received_bytes": received_bytes.to_string(), "maximum": maximum.to_string() }),
        ),
        ApiError::ProgramLimitReached { maximum } => (
            "program_limit_reached",
            json!({ "maximum": maximum.to_string() }),
        ),
        ApiError::InstanceLimitReached { maximum } => (
            "instance_limit_reached",
            json!({ "maximum": maximum.to_string() }),
        ),
        ApiError::InputLimitExceeded {
            received_bytes,
            maximum,
        } => (
            "input_limit_exceeded",
            json!({ "received_bytes": received_bytes.to_string(), "maximum": maximum.to_string() }),
        ),
        ApiError::InitialMemoryLimitExceeded {
            received_entries,
            maximum,
        } => (
            "initial_memory_limit_exceeded",
            json!({
                "received_entries": received_entries.to_string(),
                "maximum": maximum.to_string(),
            }),
        ),
        ApiError::DuplicateInitialMemoryAddress { address } => (
            "duplicate_initial_memory_address",
            json!({ "address": DisplayValue(address) }),
        ),
        ApiError::InvalidConfiguration { field } => {
            ("invalid_configuration", json!({ "field": field }))
        }
        ApiError::UnknownProgramHandle { handle } => (
            "unknown_program_handle",
            json!({ "handle": handle.get().to_string() }),
        ),
        ApiError::UnknownInstanceHandle { handle } => (
            "unknown_instance_handle",
            json!({ "handle": handle.get().to_string() }),
        ),
        ApiError::ZeroTickBudget => ("zero_tick_budget", Value::Null),
        ApiError::RunTickBudgetExceeded { requested, maximum } => (
            "run_tick_budget_exceeded",
            json!({
                "requested": requested.to_string(),
                "maximum": maximum.to_string(),
            }),
        ),
        ApiError::InstanceTickBudgetExceeded {
            requested,
            remaining,
            maximum,
        } => (
            "instance_tick_budget_exceeded",
            json!({
                "requested": requested.to_string(),
                "remaining": remaining.to_string(),
                "maximum": maximum.to_string(),
            }),
        ),
        ApiError::WorkUnitBudgetExceeded { maximum } => (
            "work_unit_budget_exceeded",
            json!({ "maximum": maximum.to_string() }),
        ),
        ApiError::HandleSpaceExhausted => ("handle_space_exhausted", Value::Null),
        ApiError::VmInitialization(error) => (
            "vm_initialization_failed",
            json!({ "reason": format!("{error:?}") }),
        ),
    };

    serialize_bounded_json(
        &json!({
            "api_version": RUNTIME_API_VERSION,
            "error": { "code": code, "error_number": error_number("api", code), "details": details },
        }),
        maximum,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_input_error_json, bounded_zeroed_buffer, parse_instance_handle, parse_tick_limit,
        program_view_json, serialize_bounded_json, BoundedInputError, BrowserRuntime,
        SnapshotRequest, RESPONSE_LIMIT_ERROR, RUNTIME_API_VERSION,
    };
    use serde_json::{json, Value};

    fn runtime() -> BrowserRuntime {
        runtime_with("10000", "10000", "1000000", 1_048_576, 1_048_576)
    }

    fn runtime_with(
        max_run_ticks: &str,
        max_total_ticks: &str,
        max_work_units: &str,
        max_response_bytes: u32,
        max_state_bytes: u32,
    ) -> BrowserRuntime {
        BrowserRuntime::with_limits(
            1_048_576,
            128,
            128,
            1_048_576,
            65_536,
            1_048_576,
            max_response_bytes,
            max_state_bytes,
            max_run_ticks,
            max_total_ticks,
            max_work_units,
        )
        .expect("test limits are valid")
    }

    fn parse(value: &str) -> Value {
        serde_json::from_str(value).expect("adapter responses must be valid JSON")
    }

    fn fixture_run_configuration(run: &Value) -> String {
        json!({
            "boundary_mode": run["boundary"],
            "seed": run["seed"],
            "custom_execution_limit": run["custom_execution_limit"],
        })
        .to_string()
    }

    fn fixture_input(run: &Value) -> Vec<u8> {
        run["input"]
            .as_array()
            .expect("fixture input is an array")
            .iter()
            .map(|value| u8::try_from(value.as_u64().expect("input is a byte")).unwrap())
            .collect()
    }

    fn assert_scalar_or_array(actual: Vec<Value>, expected: &Value, context: &str) {
        let actual = if expected.is_array() {
            Value::Array(actual)
        } else {
            assert_eq!(
                actual.len(),
                1,
                "{context}: scalar fixture requires one value"
            );
            actual.into_iter().next().unwrap()
        };
        assert_eq!(&actual, expected, "{context}");
    }

    fn assert_full_fixture_snapshot(snapshot: &Value, expected: &Value, id: &str) {
        assert_eq!(snapshot["status"], expected["vm_status"], "{id}: VM status");
        for key in ["committed_ticks", "registers", "output"] {
            assert_eq!(snapshot[key], expected[key], "{id}: {key}");
        }
        for key in [
            "operation_count",
            "used_cell_count",
            "used_memory_address_count",
            "peak_data_stack_usage",
            "peak_instruction_stack_usage",
            "peak_call_stack_usage",
            "instruction_variety",
        ] {
            assert_eq!(
                snapshot["metrics"][key], expected[key],
                "{id}: metrics.{key}"
            );
        }
        let error_codes = snapshot["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|error| error["code"].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            Value::Array(error_codes),
            expected["error_codes"],
            "{id}: error codes"
        );

        if let Some(value) = expected.get("remaining_input") {
            assert_eq!(snapshot["remaining_input"], *value, "{id}: remaining input");
        }
        if let Some(value) = expected.get("memory") {
            assert_eq!(snapshot["memory"], *value, "{id}: memory");
        }
        if let Some(value) = expected.get("thread_direction") {
            assert_eq!(
                snapshot["threads"][0]["direction"], *value,
                "{id}: direction"
            );
        }
        if let Some(value) = expected.get("thread_random_state") {
            assert_eq!(
                snapshot["threads"][0]["random_state"], *value,
                "{id}: random state"
            );
        }
        if let Some(value) = expected.get("threads") {
            let actual = snapshot["threads"]
                .as_array()
                .unwrap()
                .iter()
                .map(|thread| {
                    json!({
                        "id": thread["id"],
                        "position": thread["position"],
                        "direction": thread["direction"],
                    })
                })
                .collect::<Vec<_>>();
            assert_scalar_or_array(actual, value, &format!("{id}: threads"));
        }
        for (expected_key, field) in [
            ("thread_call_stack_sizes", "call_frames"),
            ("thread_data_stack_sizes", "data_stack"),
            ("thread_instruction_stack_sizes", "instruction_stack"),
        ] {
            if let Some(value) = expected.get(expected_key) {
                let actual = snapshot["threads"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|thread| json!(thread[field].as_array().unwrap().len()))
                    .collect();
                assert_scalar_or_array(actual, value, &format!("{id}: {expected_key}"));
            }
        }
        let cells = snapshot["runtime_program"]["main"]["cells"]
            .as_array()
            .unwrap();
        for (key, cell_field) in [
            ("runtime_main_primary_tokens", "primary"),
            ("runtime_main_attachment_tokens", "attachment"),
        ] {
            if let Some(value) = expected.get(key) {
                let actual = cells
                    .iter()
                    .map(|cell| cell[cell_field].clone())
                    .collect::<Vec<_>>();
                assert_eq!(Value::Array(actual), *value, "{id}: {key}");
            }
        }
        let errors = snapshot["errors"].as_array().unwrap();
        for (key, field) in [
            ("error_global_ticks", "global_tick"),
            ("error_custom_internal_ticks", "scope.internal_tick"),
            ("error_thread_ids", "details.thread_id"),
        ] {
            if let Some(value) = expected.get(key) {
                let actual = errors
                    .iter()
                    .filter_map(|error| match field {
                        "global_tick" => Some(error["global_tick"].clone()),
                        "scope.internal_tick" => error["scope"].get("internal_tick").cloned(),
                        "details.thread_id" => error["details"].get("thread_id").cloned(),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_scalar_or_array(actual, value, &format!("{id}: {key}"));
            }
        }
        if let Some(value) = expected.get("error_thread_id_groups") {
            let actual = errors
                .iter()
                .filter_map(|error| {
                    error["details"]
                        .get("thread_ids")
                        .or_else(|| error["details"].get("internal_thread_ids"))
                        .cloned()
                })
                .collect::<Vec<_>>();
            assert_eq!(Value::Array(actual), *value, "{id}: error thread-id groups");
        }
    }

    #[test]
    fn browser_input_allocation_is_fallible_and_returns_a_structured_error() {
        assert_eq!(
            bounded_zeroed_buffer(4, 4).expect("input allocation should succeed"),
            vec![0, 0, 0, 0]
        );
        assert!(matches!(
            bounded_zeroed_buffer(5, 4),
            Err(BoundedInputError::TooLarge)
        ));
        assert!(matches!(
            bounded_zeroed_buffer(usize::MAX, usize::MAX),
            Err(BoundedInputError::AllocationFailed)
        ));

        let allocation_error =
            bounded_input_error_json("input", BoundedInputError::AllocationFailed, 512);
        let allocation_error = parse(&allocation_error);
        assert_eq!(allocation_error["error"]["code"], "input_allocation_failed");
    }

    #[test]
    fn bounded_json_writer_returns_a_valid_limit_error_instead_of_partial_json() {
        let response = serialize_bounded_json(&json!({ "payload": "é".repeat(256) }), 256);
        assert_eq!(response, RESPONSE_LIMIT_ERROR);
        assert!(response.len() <= 256);
        assert!(serde_json::from_str::<Value>(&response).is_ok());
        assert_eq!(
            serde_json::from_str::<Value>(&response).unwrap()["error"]["error_number"],
            "7019"
        );
    }

    #[test]
    fn check_and_compile_use_runtime_api_v3_for_full_source() {
        let mut adapter = runtime();
        assert_eq!(adapter.runtime.api_version(), 3);

        let checked = parse(&adapter.check_source("~x\n".to_owned()));
        assert_eq!(checked["api_version"], RUNTIME_API_VERSION);
        assert_eq!(checked["diagnostics"][0]["severity"], "error");
        assert_eq!(checked["diagnostics"][0]["code"], "source.invalid_entry");
        assert!(checked["diagnostics"][0]["span"]["start"].is_string());

        let compiled =
            parse(&adapter.compile_source(include_str!("../../../examples/echo.cg").to_owned()));
        assert_eq!(compiled["api_version"], 3);
        assert_eq!(compiled["outcome"]["kind"], "compiled");
        assert_eq!(compiled["outcome"]["program"], "1");
        assert!(compiled["outcome"].get("view").is_none());
        let view = parse(&adapter.program_view_handle("1"));
        assert_eq!(view["view"]["outer"]["main"]["width"], "4");
    }

    #[test]
    fn full_conformance_fixtures_match_snapshots_metrics_and_errors() {
        let suite: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/conformance-v1.json"))
                .expect("shared Full conformance suite must be valid JSON");
        assert_eq!(suite["schema_version"], 1);
        let mut adapter = runtime();

        for fixture in suite["cases"]
            .as_array()
            .expect("fixture cases are an array")
        {
            let id = fixture["id"].as_str().expect("fixture id is a string");
            let source = fixture["source"]
                .as_str()
                .expect("fixture source is inline");
            let compiled = parse(&adapter.compile_source(source.to_owned()));
            assert_eq!(compiled["outcome"]["kind"], "compiled", "{id}: compile");
            let program = compiled["outcome"]["program"].as_str().unwrap().to_owned();
            let run = &fixture["run"];
            let initial_memory = run.get("initial_memory").cloned().unwrap_or(json!([]));
            let created = parse(&adapter.create_instance_data(
                program.clone(),
                fixture_input(run),
                initial_memory.to_string(),
                fixture_run_configuration(run),
            ));
            let instance = created["instance"].as_str().unwrap().to_owned();
            let result = parse(&adapter.run_handle(&instance, run["max_ticks"].as_str().unwrap()));
            assert_eq!(result["api_version"], 3, "{id}: API version");
            let expected = &fixture["expected"];
            let expected_status = match expected["status"].as_str().unwrap() {
                "tick_limit_reached" => {
                    json!({ "kind": "yielded", "reason": "tick_slice_exhausted" })
                }
                status => json!({ "kind": status }),
            };
            assert_eq!(result["status"], expected_status, "{id}: run status");
            assert_eq!(
                result["newly_emitted_output"], expected["output"],
                "{id}: output delta"
            );
            assert!(result["events"].is_array(), "{id}: events are structured");
            assert_full_fixture_snapshot(&result["snapshot"], expected, id);

            adapter.release_instance_handle(&instance);
            adapter.release_program_handle(&program);
        }
    }

    #[test]
    fn program_view_is_the_canonical_full_view_without_wide_integer_loss() {
        let mut adapter = runtime();
        let response = adapter
            .runtime
            .compile(super::CompileRequest {
                api_version: RUNTIME_API_VERSION,
                source: include_str!("../../../examples/echo.cg").to_owned(),
            })
            .expect("Full projection source compiles");
        let program = match response.outcome {
            super::CompileOutcome::Compiled { program } => program,
            super::CompileOutcome::Diagnostics { items } => {
                panic!("Full source has diagnostics: {items:?}")
            }
        };
        let response = adapter
            .runtime
            .program_view(super::ProgramViewRequest {
                api_version: RUNTIME_API_VERSION,
                program,
            })
            .expect("live program view exists");
        let actual = parse(&adapter.program_view_handle(&program.get().to_string()));
        assert_eq!(actual["api_version"], 3);
        assert_eq!(actual["view"], program_view_json(&response.view));
    }

    #[test]
    fn browser_lifecycle_preserves_exact_handles_and_returns_full_step_and_run_data() {
        let mut adapter = runtime();
        let compiled =
            parse(&adapter.compile_source(include_str!("../../../examples/echo.cg").to_owned()));
        let program = compiled["outcome"]["program"].as_str().unwrap().to_owned();
        let created = parse(&adapter.create_instance_data(
            program.clone(),
            vec![73],
            "[]".to_owned(),
            json!({ "boundary_mode": "exit", "seed": "9007199254740993", "custom_execution_limit": "100" }).to_string(),
        ));
        let instance = created["instance"].as_str().unwrap().to_owned();

        let first_step = parse(&adapter.step_handle(&instance));
        assert_eq!(first_step["api_version"], 3);
        assert_eq!(first_step["result"]["attempted_tick"], "1");
        assert_eq!(first_step["result"]["committed_ticks"], "1");
        assert_eq!(first_step["result"]["status"], "running");
        assert_eq!(first_step["result"]["newly_emitted_output"], json!([]));
        assert!(first_step["result"]["metrics"]["used_cell_count"].is_string());
        assert!(first_step["result"]["events"].is_array());
        assert!(first_step["result"].get("snapshot").is_some());

        let slice = parse(&adapter.run_handle(&instance, "1"));
        assert_eq!(slice["status"]["kind"], "yielded");
        let completed = parse(&adapter.run_handle(&instance, "10"));
        assert_eq!(completed["status"]["kind"], "halted");
        assert_eq!(completed["snapshot"]["committed_ticks"], "4");
        assert_eq!(completed["snapshot"]["output"], json!([73]));
        assert_eq!(completed["newly_emitted_output"], json!([73]));
        assert!(completed["snapshot"]["memory"].is_array());
        assert!(completed["snapshot"]["runtime_program"]["main"].is_object());
        assert!(completed["snapshot"]["threads"].is_array());
        assert!(completed["snapshot"]["metrics"]["used_memory_addresses"].is_array());

        assert_eq!(
            parse(&adapter.release_instance_handle(&instance))["api_version"],
            3
        );
        assert_eq!(
            parse(&adapter.release_program_handle(&program))["api_version"],
            3
        );
        assert_eq!(
            parse(&adapter.snapshot_handle(&instance))["error"]["code"],
            "unknown_instance_handle"
        );
    }

    #[test]
    fn bounded_tick_limits_reject_without_partial_progress() {
        let mut adapter = runtime_with("2", "2", "100", 1_048_576, 1_048_576);
        let compiled =
            parse(&adapter.compile_source(include_str!("../../../examples/echo.cg").to_owned()));
        let program = compiled["outcome"]["program"].as_str().unwrap().to_owned();
        let created = parse(
            &adapter.create_instance_data(
                program.clone(),
                vec![9],
                "[]".to_owned(),
                json!({ "boundary_mode": "exit", "seed": "0", "custom_execution_limit": "100" })
                    .to_string(),
            ),
        );
        let instance = created["instance"].as_str().unwrap().to_owned();

        let rejected = parse(&adapter.run_handle(&instance, "3"));
        assert_eq!(rejected["error"]["code"], "run_tick_budget_exceeded");
        assert_eq!(
            parse(&adapter.snapshot_handle(&instance))["snapshot"]["committed_ticks"],
            "0"
        );
        let bounded = parse(&adapter.run_handle(&instance, "2"));
        assert_eq!(bounded["snapshot"]["committed_ticks"], "2");
        assert_eq!(
            parse(&adapter.step_handle(&instance))["error"]["code"],
            "instance_tick_budget_exceeded"
        );
        adapter.release_instance_handle(&instance);
        adapter.release_program_handle(&program);
    }

    #[test]
    fn oversized_compile_response_does_not_consume_program_capacity() {
        let mut adapter = runtime_with("100", "100", "1000", 256, 4096);
        let compiled = parse(&adapter.compile_source(format!("~> {}", "_ ".repeat(300))));
        assert_eq!(compiled["outcome"]["kind"], "compiled");
        let oversized =
            parse(&adapter.program_view_handle(compiled["outcome"]["program"].as_str().unwrap()));
        assert_eq!(
            oversized["error"]["code"],
            "response_payload_limit_exceeded"
        );
    }

    #[test]
    fn oversized_retained_snapshot_releases_its_instance() {
        let source = "@main\n@size 2x1\n~> .\n@end\n";
        let mut reference = runtime();
        let reference_program = parse(&reference.compile_source(source.to_owned()))["outcome"]
            ["program"]
            .as_str()
            .unwrap()
            .to_owned();
        let reference_instance = parse(
            &reference.create_instance_data(
                reference_program,
                Vec::new(),
                "[]".to_owned(),
                json!({ "boundary_mode": "wrap", "seed": "0", "custom_execution_limit": "100" })
                    .to_string(),
            ),
        )["instance"]
            .as_str()
            .unwrap()
            .to_owned();
        let handle = parse_instance_handle(&reference_instance).unwrap();
        let snapshot = reference
            .runtime
            .snapshot_projection_view(SnapshotRequest {
                api_version: RUNTIME_API_VERSION,
                instance: handle,
            })
            .unwrap();
        let baseline_bytes = serde_json::to_vec(&super::SnapshotProjection::new(&snapshot))
            .unwrap()
            .len();
        let state_limit = u32::try_from(baseline_bytes + 8).unwrap();
        assert!(state_limit >= super::MIN_INSTANCE_STATE_BYTES);

        let mut adapter = runtime_with("1000", "1000", "1000", 1_048_576, state_limit);
        let program = parse(&adapter.compile_source(source.to_owned()))["outcome"]["program"]
            .as_str()
            .unwrap()
            .to_owned();
        let instance = parse(
            &adapter.create_instance_data(
                program.clone(),
                Vec::new(),
                "[]".to_owned(),
                json!({ "boundary_mode": "wrap", "seed": "0", "custom_execution_limit": "100" })
                    .to_string(),
            ),
        )["instance"]
            .as_str()
            .unwrap()
            .to_owned();
        let overflow = parse(&adapter.run_handle(&instance, "100"));
        assert_eq!(overflow["error"]["code"], "instance_state_limit_exceeded");
        assert_eq!(
            parse(&adapter.snapshot_handle(&instance))["error"]["code"],
            "unknown_instance_handle"
        );
    }

    #[test]
    fn tick_limit_parser_preserves_u64_values_and_rejects_zero() {
        assert_eq!(
            parse_tick_limit("9007199254740993").map(|value| value.get()),
            Some(9_007_199_254_740_993)
        );
        assert!(parse_tick_limit("9007199254740993.0").is_none());
        assert!(parse_tick_limit("0").is_none());
    }

    #[test]
    fn work_limit_yield_is_structured_and_keeps_a_valid_snapshot() {
        let mut adapter = runtime_with("10", "100", "1", 1_048_576, 1_048_576);
        let program = parse(&adapter.compile_source("~> + ;\n".to_owned()))["outcome"]["program"]
            .as_str()
            .unwrap()
            .to_owned();
        let created = parse(
            &adapter.create_instance_data(
                program.clone(),
                vec![],
                "[]".to_owned(),
                json!({ "boundary_mode": "wrap", "seed": "0", "custom_execution_limit": "10" })
                    .to_string(),
            ),
        );
        let instance = created["instance"].as_str().unwrap();
        let response = parse(&adapter.run_handle(instance, "10"));
        assert_eq!(
            response["status"],
            json!({ "kind": "yielded", "reason": "work_unit_budget_exhausted" })
        );
        assert!(response["snapshot"]["committed_ticks"].is_string());
        assert!(response["events"].is_array());
        assert!(response["newly_emitted_output"].is_array());
    }
}
