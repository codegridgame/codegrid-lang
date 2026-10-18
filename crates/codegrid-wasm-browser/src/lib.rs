//! Browser-facing WebAssembly bindings over the shared runtime API.
//!
//! This adapter contains no language rules. It owns only the JavaScript
//! binding and bounded transport of shared Full Runtime API JSON projections.

use codegrid_runtime_api::error_number;
use codegrid_runtime_api::json::*;
use codegrid_runtime_api::{
    ApiError, CheckRequest, CompileOutcome, CompileRequest, CreateInstanceRequest, HostLimits,
    InstanceHandle, MemoryAddress, MemoryEntry, ProgramHandle, ProgramViewRequest,
    ReleaseInstanceRequest, ReleaseProgramRequest, RunRequest, RuntimeApi, RuntimeConfiguration,
    RuntimeSnapshotView, SnapshotRequest, StepRequest, RUNTIME_API_VERSION,
};
#[cfg(test)]
use codegrid_runtime_api::{BoardView, CodeGridView, ProgramView};
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Map;
use serde_json::{json, Value};
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
const RESPONSE_LIMIT_ERROR: &str = r#"{"api_version":4,"error":{"code":"response_payload_limit_exceeded","error_number":"7019","message":"serialized response exceeds the configured byte limit"}}"#;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigurationWire {
    seed: String,
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
        max_run_steps_per_call: JsValue,
        max_total_steps_per_instance: JsValue,
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
        let max_run_steps_per_call = bounded_string(&max_run_steps_per_call, 20)
            .map_err(|_| host_limit_error("max_run_steps_per_call must be a decimal string"))?;
        let max_total_steps_per_instance = bounded_string(&max_total_steps_per_instance, 20)
            .map_err(|_| {
                host_limit_error("max_total_steps_per_instance must be a decimal string")
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
            &max_run_steps_per_call,
            &max_total_steps_per_instance,
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

    /// Executes exactly one VM transition.
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

    /// Runs a bounded step slice and returns a full resynchronization snapshot.
    pub fn run(&mut self, instance: JsValue, max_steps: JsValue) -> String {
        let Some(instance) = bounded_string(&instance, 20).ok() else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let Some(max_steps) = bounded_string(&max_steps, 20).ok() else {
            return input_error_json(
                "invalid_step_limit",
                "max_steps must be a u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        self.run_handle(&instance, &max_steps)
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
        max_run_steps_per_call: &str,
        max_total_steps_per_instance: &str,
        max_work_units_per_call: &str,
    ) -> Result<BrowserRuntime, String> {
        let max_run_steps_per_call =
            parse_positive_u64(max_run_steps_per_call).ok_or_else(|| {
                "max_run_steps_per_call must be a positive u64 decimal string".to_owned()
            })?;
        let max_total_steps_per_instance = parse_positive_u64(max_total_steps_per_instance)
            .ok_or_else(|| {
                "max_total_steps_per_instance must be a positive u64 decimal string".to_owned()
            })?;
        let max_work_units_per_call =
            parse_positive_u64(max_work_units_per_call).ok_or_else(|| {
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
                max_run_steps_per_call,
                max_total_steps_per_instance,
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
        let configuration: ConfigurationWire = match serde_json::from_str(&configuration_json) {
            Ok(configuration) => configuration,
            Err(_) => {
                return input_error_json(
                    "invalid_configuration",
                    "configuration must contain only seed",
                    self.max_response_bytes as usize,
                )
            }
        };
        let Some(seed) = parse_canonical_u64(&configuration.seed) else {
            return input_error_json(
                "invalid_configuration_integer",
                "seed must be a canonical u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        match self.runtime.create_instance(CreateInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            program,
            input,
            initial_memory,
            configuration: RuntimeConfiguration { seed },
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

    fn run_handle(&mut self, instance: &str, max_steps: &str) -> String {
        let Some(instance) = parse_instance_handle(instance) else {
            return input_error_json(
                "invalid_instance_handle",
                "instance must be a nonzero u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let Some(max_steps) = parse_u64(max_steps) else {
            return input_error_json(
                "invalid_step_limit",
                "max_steps must be a u64 decimal string",
                self.max_response_bytes as usize,
            );
        };
        let max_state_bytes = self.max_instance_state_bytes as usize;
        let max_response_bytes = self.max_response_bytes as usize;
        match self.runtime.run_view(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_steps,
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

fn parse_positive_u64(value: &str) -> Option<NonZeroU64> {
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
        "width": board.width,
        "height": board.height,
        "cells": board.cells.iter().map(|cell| json!({
            "entry": cell.entry,
            "primary": cell.primary,
            "prefix": cell.prefix,
            "attachment": cell.attachment,
        })).collect::<Vec<_>>(),
        "folded_blocks": folded_blocks,
    })
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
        ApiError::ZeroStepBudget => ("zero_step_budget", Value::Null),
        ApiError::RunStepBudgetExceeded { requested, maximum } => (
            "run_step_budget_exceeded",
            json!({
                "requested": requested.to_string(),
                "maximum": maximum.to_string(),
            }),
        ),
        ApiError::InstanceStepBudgetExceeded {
            requested,
            remaining,
            maximum,
        } => (
            "instance_step_budget_exceeded",
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
        bounded_input_error_json, bounded_zeroed_buffer, parse_instance_handle, parse_positive_u64,
        program_view_json, serialize_bounded_json, BoundedInputError, BrowserRuntime,
        SnapshotRequest, RESPONSE_LIMIT_ERROR, RUNTIME_API_VERSION,
    };
    use serde_json::{json, Value};

    fn runtime() -> BrowserRuntime {
        runtime_with("10000", "10000", "1000000", 1_048_576, 1_048_576)
    }

    fn runtime_with(
        max_run_steps: &str,
        max_total_steps: &str,
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
            max_run_steps,
            max_total_steps,
            max_work_units,
        )
        .expect("test limits are valid")
    }

    fn parse(value: &str) -> Value {
        serde_json::from_str(value).expect("adapter responses must be valid JSON")
    }

    fn fixture_run_configuration(run: &Value) -> String {
        json!({ "seed": run["seed"] }).to_string()
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
        assert_eq!(
            snapshot["committed_steps"], expected["committed_steps"],
            "{id}: committed steps"
        );
        for key in ["registers", "output"] {
            assert_eq!(snapshot[key], expected[key], "{id}: {key}");
        }
        assert_eq!(
            snapshot["metrics"],
            json!({"committed_steps": expected["committed_steps"]}),
            "{id}: language metrics expose step count only"
        );
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
            ("error_transition_ids", "transition_id"),
            ("error_custom_internal_steps", "scope.internal_step"),
            ("error_thread_ids", "details.thread_id"),
        ] {
            if let Some(value) = expected.get(key) {
                let actual = errors
                    .iter()
                    .filter_map(|error| match field {
                        "transition_id" => Some(error["transition_id"].clone()),
                        "scope.internal_step" => error["scope"].get("internal_step").cloned(),
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
    fn check_and_compile_use_runtime_api_v4_for_full_source() {
        let mut adapter = runtime();
        assert_eq!(adapter.runtime.api_version(), 4);

        let checked = parse(&adapter.check_source("~x\n".to_owned()));
        assert_eq!(checked["api_version"], RUNTIME_API_VERSION);
        assert_eq!(checked["diagnostics"][0]["severity"], "error");
        assert_eq!(checked["diagnostics"][0]["code"], "source.invalid_entry");
        assert!(checked["diagnostics"][0]["span"]["start"].is_string());

        let compiled =
            parse(&adapter.compile_source(include_str!("../../../examples/echo.cg").to_owned()));
        assert_eq!(compiled["api_version"], 4);
        assert_eq!(compiled["outcome"]["kind"], "compiled");
        assert_eq!(compiled["outcome"]["program"], "1");
        assert!(compiled["outcome"].get("view").is_none());
        let view = parse(&adapter.program_view_handle("1"));
        assert_eq!(view["view"]["outer"]["main"]["width"], 4);
        assert!(view["view"]["outer"]["main"]["width"].is_number());
    }

    #[test]
    fn full_conformance_fixtures_match_snapshots_and_errors() {
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
            let result = parse(&adapter.run_handle(&instance, run["max_steps"].as_str().unwrap()));
            assert_eq!(result["api_version"], 4, "{id}: API version");
            let expected = &fixture["expected"];
            let expected_status = match expected["status"].as_str().unwrap() {
                "step_limit_reached" => {
                    json!({ "kind": "yielded", "reason": "step_slice_exhausted" })
                }
                "work_limit_reached" => {
                    json!({ "kind": "yielded", "reason": "work_unit_budget_exhausted" })
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
        assert_eq!(actual["api_version"], 4);
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
            json!({ "seed": "9007199254740993" }).to_string(),
        ));
        let instance = created["instance"].as_str().unwrap().to_owned();

        let first_step = parse(&adapter.step_handle(&instance));
        assert_eq!(first_step["api_version"], 4);
        assert_eq!(first_step["result"]["attempted_transition_id"], "1");
        assert_eq!(first_step["result"]["committed_steps"], "1");
        assert_eq!(first_step["result"]["status"], "running");
        assert_eq!(first_step["result"]["newly_emitted_output"], json!([]));
        assert_eq!(first_step["result"]["metrics"]["committed_steps"], "1");
        assert!(first_step["result"]["events"].is_array());
        assert!(first_step["result"].get("snapshot").is_some());

        let slice = parse(&adapter.run_handle(&instance, "1"));
        assert_eq!(slice["status"]["kind"], "yielded");
        let completed = parse(&adapter.run_handle(&instance, "10"));
        assert_eq!(completed["status"]["kind"], "halted");
        assert_eq!(completed["snapshot"]["committed_steps"], "4");
        assert_eq!(completed["snapshot"]["output"], json!([73]));
        assert_eq!(completed["newly_emitted_output"], json!([73]));
        assert!(completed["snapshot"]["memory"].is_array());
        assert!(completed["snapshot"]["runtime_program"]["main"].is_object());
        assert!(completed["snapshot"]["threads"].is_array());
        assert_eq!(
            completed["snapshot"]["metrics"],
            json!({"committed_steps": "4"})
        );

        assert_eq!(
            parse(&adapter.release_instance_handle(&instance))["api_version"],
            4
        );
        assert_eq!(
            parse(&adapter.release_program_handle(&program))["api_version"],
            4
        );
        assert_eq!(
            parse(&adapter.snapshot_handle(&instance))["error"]["code"],
            "unknown_instance_handle"
        );
    }

    #[test]
    fn bounded_step_limits_reject_without_partial_progress() {
        let mut adapter = runtime_with("2", "2", "100", 1_048_576, 1_048_576);
        let compiled =
            parse(&adapter.compile_source(include_str!("../../../examples/echo.cg").to_owned()));
        let program = compiled["outcome"]["program"].as_str().unwrap().to_owned();
        let created = parse(&adapter.create_instance_data(
            program.clone(),
            vec![9],
            "[]".to_owned(),
            json!({ "seed": "0" }).to_string(),
        ));
        let instance = created["instance"].as_str().unwrap().to_owned();

        let rejected = parse(&adapter.run_handle(&instance, "3"));
        assert_eq!(rejected["error"]["code"], "run_step_budget_exceeded");
        assert_eq!(
            parse(&adapter.snapshot_handle(&instance))["snapshot"]["committed_steps"],
            "0"
        );
        let bounded = parse(&adapter.run_handle(&instance, "2"));
        assert_eq!(bounded["snapshot"]["committed_steps"], "2");
        assert_eq!(
            parse(&adapter.step_handle(&instance))["error"]["code"],
            "instance_step_budget_exceeded"
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
        let reference_instance = parse(&reference.create_instance_data(
            reference_program,
            Vec::new(),
            "[]".to_owned(),
            json!({ "seed": "0" }).to_string(),
        ))["instance"]
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
        let instance = parse(&adapter.create_instance_data(
            program.clone(),
            Vec::new(),
            "[]".to_owned(),
            json!({ "seed": "0" }).to_string(),
        ))["instance"]
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
    fn step_limit_parser_preserves_u64_values_and_rejects_zero() {
        assert_eq!(
            parse_positive_u64("9007199254740993").map(|value| value.get()),
            Some(9_007_199_254_740_993)
        );
        assert!(parse_positive_u64("9007199254740993.0").is_none());
        assert!(parse_positive_u64("0").is_none());
    }

    #[test]
    fn work_limit_yield_is_structured_and_keeps_a_valid_snapshot() {
        let mut adapter = runtime_with("10", "100", "1", 1_048_576, 1_048_576);
        let program = parse(&adapter.compile_source("~> + ;\n".to_owned()))["outcome"]["program"]
            .as_str()
            .unwrap()
            .to_owned();
        let created = parse(&adapter.create_instance_data(
            program.clone(),
            vec![],
            "[]".to_owned(),
            json!({ "seed": "0" }).to_string(),
        ));
        let instance = created["instance"].as_str().unwrap();
        let response = parse(&adapter.run_handle(instance, "10"));
        assert_eq!(
            response["status"],
            json!({ "kind": "yielded", "reason": "work_unit_budget_exhausted" })
        );
        assert!(response["snapshot"]["committed_steps"].is_string());
        assert!(response["events"].is_array());
        assert!(response["newly_emitted_output"].is_array());
    }
}
