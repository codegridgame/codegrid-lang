use super::projection::{
    CheckResponse, CompileDiagnosticsResponse, CompiledResponse, ProgramViewResponse, RunResponse,
    SnapshotProjection, SnapshotResponse, StepResponse,
};
use super::{
    api_error_response, bounded_decimal_u64, bounded_usize, decimal_u64, error_response,
    parse_input, parse_memory_address, required_string, required_u64, MAX_INITIAL_MEMORY_ENTRIES,
    MAX_INPUT_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, MAX_RUN_TICKS_PER_CALL,
    MAX_SOURCE_BYTES, MAX_TOTAL_TICKS_PER_INSTANCE, MAX_WORK_UNITS_PER_CALL,
};
use codegrid_runtime_api::{
    ApiError, CheckRequest, CompileOutcome, CompileRequest, CreateInstanceRequest, HostLimits,
    InstanceHandle, MemoryEntry, ProgramHandle, ProgramViewRequest, ReleaseInstanceRequest,
    ReleaseProgramRequest, RunRequest, RuntimeApi, RuntimeConfiguration, SnapshotRequest,
    StepRequest,
};
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroU64;

const MAX_COMPILED_PROGRAMS: usize = 256;
const MAX_INSTANCES: usize = 64;
const MIN_RESPONSE_BYTES: usize = 256;

enum ParsedRequest {
    Object(RequestObject),
    NonObject,
}

struct RequestObject {
    fields: Map<String, Value>,
    input: Option<ParsedInput>,
    initial_memory: Option<ParsedInitialMemory>,
}

enum ParsedInput {
    NotArray,
    Array {
        bytes: Vec<u8>,
        too_many: bool,
        invalid_byte: bool,
    },
}

enum ParsedInitialMemory {
    NotArray,
    Array {
        entries: Vec<ParsedMemoryEntry>,
        too_many: bool,
        invalid_entry: bool,
    },
}

#[derive(Default)]
struct ParsedMemoryEntry {
    address: Option<String>,
    value: Option<u8>,
    invalid_field: bool,
}

impl<'de> Deserialize<'de> for ParsedMemoryEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(MemoryEntryVisitor)
    }
}

struct MemoryEntryVisitor;

impl<'de> Visitor<'de> for MemoryEntryVisitor {
    type Value = ParsedMemoryEntry;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an initial-memory entry object")
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entry = ParsedMemoryEntry::default();
        let mut saw_address = false;
        let mut saw_value = false;
        while let Some(key) = object.next_key::<String>()? {
            match key.as_str() {
                "address" => {
                    saw_address = true;
                    let value = object.next_value::<ScalarValue>()?.0;
                    if let Value::String(text) = value {
                        entry.address = Some(text);
                    } else {
                        entry.invalid_field = true;
                    }
                }
                "value" => {
                    saw_value = true;
                    if let Some(value) = object.next_value::<InputByte>()?.0 {
                        entry.value = Some(value);
                    } else {
                        entry.invalid_field = true;
                    }
                }
                _ => {
                    object.next_value::<IgnoredAny>()?;
                }
            }
        }
        entry.invalid_field |= !saw_address || !saw_value;
        Ok(entry)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedMemoryEntry {
            invalid_field: true,
            ..Default::default()
        })
    }
}

impl<'de> Deserialize<'de> for ParsedInitialMemory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(InitialMemoryVisitor)
    }
}

struct InitialMemoryVisitor;

impl<'de> Visitor<'de> for InitialMemoryVisitor {
    type Value = ParsedInitialMemory;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an array of initial-memory entries")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut entries = Vec::new();
        let mut too_many = false;
        let mut invalid_entry = false;
        let mut count = 0usize;
        while let Some(entry) = sequence.next_element::<ParsedMemoryEntry>()? {
            count = count.saturating_add(1);
            if count > MAX_INITIAL_MEMORY_ENTRIES {
                too_many = true;
            } else {
                invalid_entry |= entry.invalid_field;
                entries.push(entry);
            }
        }
        Ok(ParsedInitialMemory::Array {
            entries,
            too_many,
            invalid_entry,
        })
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while object.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInitialMemory::NotArray)
    }
}

struct ScalarValue(Value);

impl<'de> Deserialize<'de> for ScalarValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ScalarValueVisitor)
    }
}

struct ScalarValueVisitor;

impl<'de> Visitor<'de> for ScalarValueVisitor {
    type Value = ScalarValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON scalar or an ignored compound value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(ScalarValue)
            .ok_or_else(|| E::custom("JSON number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::String(value)))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ScalarValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(ScalarValue(Value::Null))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while object.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(ScalarValue(Value::Null))
    }
}

impl<'de> Deserialize<'de> for ParsedInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ParsedInputVisitor)
    }
}

struct ParsedInputVisitor;

impl<'de> Visitor<'de> for ParsedInputVisitor {
    type Value = ParsedInput;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON array of input bytes")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut bytes = Vec::new();
        let mut too_many = false;
        let mut invalid_byte = false;
        let mut count = 0usize;

        while let Some(value) = sequence.next_element::<InputByte>()? {
            count = count.saturating_add(1);
            if count > MAX_INPUT_BYTES {
                too_many = true;
            } else if let Some(value) = value.0 {
                bytes.push(value);
            } else {
                invalid_byte = true;
            }
        }

        Ok(ParsedInput::Array {
            bytes,
            too_many,
            invalid_byte,
        })
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while object.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(ParsedInput::NotArray)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedInput::NotArray)
    }
}

struct InputByte(Option<u8>);

impl<'de> Deserialize<'de> for InputByte {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(InputByteVisitor)
    }
}

struct InputByteVisitor;

impl<'de> Visitor<'de> for InputByteVisitor {
    type Value = InputByte;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an integer byte or a rejected JSON value")
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(u8::try_from(value).ok()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(u8::try_from(value).ok()))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(None))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(None))
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(None))
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(None))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InputByte(None))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(InputByte(None))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while object.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(InputByte(None))
    }
}

struct HostLimitsValue(Value);

impl<'de> Deserialize<'de> for HostLimitsValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(HostLimitsVisitor)
    }
}

struct HostLimitsVisitor;

impl<'de> Visitor<'de> for HostLimitsVisitor {
    type Value = HostLimitsValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a host-limits object")
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut fields = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if is_host_limit_field(&key) {
                let value = object.next_value::<ScalarValue>()?;
                fields.insert(key, value.0);
            } else {
                object.next_value::<IgnoredAny>()?;
            }
        }
        Ok(HostLimitsValue(Value::Object(fields)))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HostLimitsValue(Value::Null))
    }
}

impl<'de> Deserialize<'de> for ParsedRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(RequestVisitor)
    }
}

struct RequestVisitor;

impl<'de> Visitor<'de> for RequestVisitor {
    type Value = ParsedRequest;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON request object")
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut fields = Map::new();
        let mut input = None;
        let mut initial_memory = None;
        while let Some(key) = object.next_key::<String>()? {
            match key.as_str() {
                "input" => input = Some(object.next_value::<ParsedInput>()?),
                "initial_memory" => {
                    initial_memory = Some(object.next_value::<ParsedInitialMemory>()?)
                }
                "host_limits" => {
                    let value = object.next_value::<HostLimitsValue>()?;
                    fields.insert(key, value.0);
                }
                "abi_version"
                | "api_version"
                | "operation"
                | "source"
                | "program"
                | "instance"
                | "max_ticks"
                | "seed"
                | "custom_execution_limit" => {
                    let value = object.next_value::<ScalarValue>()?;
                    fields.insert(key, value.0);
                }
                _ => {
                    object.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(ParsedRequest::Object(RequestObject {
            fields,
            input,
            initial_memory,
        }))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(ParsedRequest::NonObject)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ParsedRequest::NonObject)
    }
}

fn is_host_limit_field(field: &str) -> bool {
    matches!(
        field,
        "max_source_bytes"
            | "max_compiled_programs"
            | "max_instances"
            | "max_input_bytes"
            | "max_initial_memory_entries"
            | "max_run_ticks_per_call"
            | "max_total_ticks_per_instance"
            | "max_work_units_per_call"
            | "max_response_bytes"
            | "max_instance_state_bytes"
    )
}

fn decode_input(input: Option<ParsedInput>) -> Result<Vec<u8>, (&'static str, &'static str)> {
    match input {
        None => parse_input(None),
        Some(ParsedInput::NotArray) => parse_input(Some(&Value::Null)),
        Some(ParsedInput::Array { too_many: true, .. }) => Err((
            "input_payload_limit_exceeded",
            "input exceeds the adapter byte limit",
        )),
        Some(ParsedInput::Array {
            invalid_byte: true, ..
        }) => Err((
            "invalid_input_byte",
            "each input value must be an integer from 0 to 255",
        )),
        Some(ParsedInput::Array { bytes, .. }) => Ok(bytes),
    }
}

fn decode_initial_memory(
    memory: Option<ParsedInitialMemory>,
) -> Result<Vec<MemoryEntry>, (&'static str, &'static str)> {
    let Some(memory) = memory else {
        return Ok(Vec::new());
    };
    let ParsedInitialMemory::Array {
        entries,
        too_many,
        invalid_entry,
    } = memory
    else {
        return Err((
            "invalid_initial_memory",
            "initial_memory must be an array of {address, value} objects",
        ));
    };
    if too_many {
        return Err((
            "initial_memory_payload_limit_exceeded",
            "initial_memory exceeds the adapter entry limit",
        ));
    }
    if invalid_entry {
        return Err((
            "invalid_initial_memory_entry",
            "each initial-memory entry must contain a decimal-string address and a byte value",
        ));
    }
    entries
        .into_iter()
        .map(|entry| {
            let address = entry.address.ok_or((
                "invalid_initial_memory_entry",
                "each initial-memory entry must contain a decimal-string address and a byte value",
            ))?;
            let value = entry.value.ok_or((
                "invalid_initial_memory_entry",
                "each initial-memory entry must contain a decimal-string address and a byte value",
            ))?;
            Ok(MemoryEntry {
                address: parse_memory_address(&address)?,
                value,
            })
        })
        .collect()
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
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
                "serialized JSON exceeds its configured byte limit",
            ));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_json_bounded<T: Serialize + ?Sized>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut writer = BoundedJsonWriter {
        maximum,
        bytes: Vec::new(),
    };
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.bytes)
}

fn json_fits_byte_limit<T: Serialize + ?Sized>(value: &T, maximum: usize) -> bool {
    serde_json::to_writer(&mut BoundedJsonCounter { maximum, bytes: 0 }, value).is_ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServerLimits {
    runtime: HostLimits,
    max_response_bytes: usize,
    max_instance_state_bytes: usize,
}

pub struct ServerAdapter {
    runtime: Option<RuntimeApi>,
    limits: Option<ServerLimits>,
    closed: bool,
}

impl ServerAdapter {
    pub const fn new() -> Self {
        Self {
            runtime: None,
            limits: None,
            closed: false,
        }
    }

    /// Returns the largest complete response that can be serialized for the
    /// current session. Before initialization and after shutdown, the adapter
    /// uses the validated minimum response limit for its fixed control replies.
    pub fn response_buffer_capacity(&self) -> usize {
        self.limits
            .map(|limits| limits.max_response_bytes)
            .unwrap_or(MIN_RESPONSE_BYTES)
    }

    pub fn process_request(&mut self, request_bytes: &[u8]) -> Vec<u8> {
        if request_bytes.len() > MAX_REQUEST_BYTES {
            return self.respond(error_response(
                "request_payload_limit_exceeded",
                "request exceeds the server adapter byte limit",
                Value::Null,
            ));
        }
        let request: ParsedRequest = match serde_json::from_slice(request_bytes) {
            Ok(request) => request,
            Err(_) => {
                return self.respond(error_response(
                    "invalid_request_json",
                    "request must be valid UTF-8 JSON",
                    Value::Null,
                ));
            }
        };
        let ParsedRequest::Object(RequestObject {
            fields: object,
            input,
            initial_memory,
        }) = request
        else {
            return self.respond(error_response(
                "invalid_request",
                "request must be a JSON object",
                Value::Null,
            ));
        };

        let abi_version = match required_u64(object.get("abi_version")) {
            Ok(version) => version,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        if abi_version != u64::from(super::SERVER_ABI_VERSION) {
            return self.respond(error_response(
                "unsupported_abi_version",
                "server adapter ABI version is not supported",
                json!({"received": abi_version.to_string(), "supported": super::SERVER_ABI_VERSION}),
            ));
        }
        let api_version = match required_u64(object.get("api_version")) {
            Ok(version) => match u32::try_from(version) {
                Ok(version) => version,
                Err(_) => {
                    return self.respond(error_response(
                        "unsupported_api_version",
                        "Runtime API version is outside the supported range",
                        Value::Null,
                    ));
                }
            },
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        if api_version != super::RUNTIME_API_VERSION {
            return self.respond(error_response(
                "unsupported_api_version",
                "Runtime API version is not supported",
                json!({"received": api_version, "supported": super::RUNTIME_API_VERSION}),
            ));
        }
        let operation = match required_string(object.get("operation"), "operation") {
            Ok(operation) => operation.to_owned(),
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };

        match operation.as_str() {
            "initialize" => self.initialize(&object, api_version),
            "shutdown" => self.shutdown(api_version),
            _ if self.closed => self.respond(error_response(
                "runtime_closed",
                "this WebAssembly runtime has been shut down; create a new module instance",
                Value::Null,
            )),
            _ if self.runtime.is_none() => self.respond(error_response(
                "runtime_not_initialized",
                "call initialize before using Runtime API operations",
                Value::Null,
            )),
            "check" => self.check(&object, api_version),
            "compile" => self.compile(&object, api_version),
            "create_instance" => self.create_instance(&object, input, initial_memory, api_version),
            "program_view" => self.program_view(&object, api_version),
            "step" => self.step(&object, api_version),
            "run" => self.run(&object, api_version),
            "snapshot" => self.snapshot(&object, api_version),
            "release_program" => self.release_program(&object, api_version),
            "release_instance" => self.release_instance(&object, api_version),
            _ => self.respond(error_response(
                "unsupported_operation",
                "operation is not supported by server adapter ABI v4",
                json!({"operation": operation}),
            )),
        }
    }

    fn initialize(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        if self.closed {
            return self.respond(error_response(
                "runtime_closed",
                "this WebAssembly runtime has been shut down; create a new module instance",
                Value::Null,
            ));
        }
        let limits = match parse_server_limits(request.get("host_limits")) {
            Ok(limits) => limits,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        if let Some(current) = self.limits {
            if current != limits {
                return self.respond(error_response(
                    "runtime_already_initialized",
                    "host limits are immutable for the lifetime of a WebAssembly module instance",
                    Value::Null,
                ));
            }
        } else {
            self.runtime = Some(RuntimeApi::new(limits.runtime));
            self.limits = Some(limits);
        }
        self.respond(json!({
            "abi_version": super::SERVER_ABI_VERSION,
            "api_version": api_version,
            "operation": "initialize",
            "status": "initialized",
        }))
    }

    fn shutdown(&mut self, api_version: u32) -> Vec<u8> {
        if !self.closed {
            self.runtime = None;
            self.limits = None;
            self.closed = true;
        }
        self.respond(json!({
            "abi_version": super::SERVER_ABI_VERSION,
            "api_version": api_version,
            "operation": "shutdown",
            "status": "closed",
        }))
    }

    fn check(&self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let source = match required_string(request.get("source"), "source") {
            Ok(source) => source.to_owned(),
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let response =
            match self
                .runtime
                .as_ref()
                .expect("initialized runtime")
                .check(CheckRequest {
                    api_version,
                    source,
                }) {
                Ok(response) => response,
                Err(error) => return self.respond(api_error_response(api_version, error)),
            };
        self.respond_serialized(&CheckResponse {
            api_version: response.api_version,
            diagnostics: &response.diagnostics,
        })
    }

    fn compile(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let source = match required_string(request.get("source"), "source") {
            Ok(source) => source.to_owned(),
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let response =
            match self
                .runtime
                .as_mut()
                .expect("initialized runtime")
                .compile(CompileRequest {
                    api_version,
                    source,
                }) {
                Ok(response) => response,
                Err(error) => return self.respond(api_error_response(api_version, error)),
            };
        match response.outcome {
            CompileOutcome::Compiled { program } => {
                let view = match self
                    .runtime
                    .as_ref()
                    .expect("initialized runtime")
                    .program_view(ProgramViewRequest {
                        api_version,
                        program,
                    }) {
                    Ok(response) => response.view,
                    Err(error) => {
                        let _ = self
                            .runtime
                            .as_mut()
                            .expect("initialized runtime")
                            .release_program(ReleaseProgramRequest {
                                api_version,
                                program,
                            });
                        return self.respond(api_error_response(api_version, error));
                    }
                };
                match self.try_respond_serialized(&CompiledResponse {
                    api_version: response.api_version,
                    program: program.get(),
                    view: &view,
                }) {
                    Ok(bytes) => bytes,
                    Err(()) => {
                        let _ = self
                            .runtime
                            .as_mut()
                            .expect("initialized runtime")
                            .release_program(ReleaseProgramRequest {
                                api_version,
                                program,
                            });
                        self.response_limit_error()
                    }
                }
            }
            CompileOutcome::Diagnostics { items } => {
                self.respond_serialized(&CompileDiagnosticsResponse {
                    api_version: response.api_version,
                    diagnostics: &items,
                })
            }
        }
    }

    fn program_view(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let program = match parse_program(request.get("program")) {
            Ok(program) => program,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let view = match self
            .runtime
            .as_ref()
            .expect("initialized runtime")
            .program_view(ProgramViewRequest {
                api_version,
                program,
            }) {
            Ok(response) => response,
            Err(error) => return self.respond(api_error_response(api_version, error)),
        };
        self.respond_serialized(&ProgramViewResponse {
            api_version: view.api_version,
            program: program.get(),
            view: &view.view,
        })
    }

    fn create_instance(
        &mut self,
        request: &Map<String, Value>,
        input: Option<ParsedInput>,
        initial_memory: Option<ParsedInitialMemory>,
        api_version: u32,
    ) -> Vec<u8> {
        let program = match decimal_u64(request.get("program"), "program")
            .ok()
            .and_then(ProgramHandle::from_raw)
        {
            Some(program) => program,
            None => {
                return self.respond(error_response(
                    "invalid_program_handle",
                    "program must be a nonzero u64 decimal string",
                    Value::Null,
                ));
            }
        };
        let input = match decode_input(input) {
            Ok(input) => input,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let initial_memory = match decode_initial_memory(initial_memory) {
            Ok(memory) => memory,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let seed = match decimal_u64(request.get("seed"), "seed") {
            Ok(seed) => seed,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let custom_execution_limit = match decimal_u64(
            request.get("custom_execution_limit"),
            "custom_execution_limit",
        ) {
            Ok(limit) if limit > 0 => limit,
            Ok(_) => {
                return self.respond(error_response(
                    "invalid_configuration",
                    "custom_execution_limit must be positive",
                    json!({"field":"custom_execution_limit"}),
                ))
            }
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let response = match self
            .runtime
            .as_mut()
            .expect("initialized runtime")
            .create_instance(CreateInstanceRequest {
                api_version,
                program,
                input,
                initial_memory,
                configuration: RuntimeConfiguration {
                    seed,
                    custom_execution_limit,
                },
            }) {
            Ok(response) => response,
            Err(error) => return self.respond(api_error_response(api_version, error)),
        };
        if !self.instance_snapshot_fits(response.instance, api_version) {
            let _ = self
                .runtime
                .as_mut()
                .expect("initialized runtime")
                .release_instance(ReleaseInstanceRequest {
                    api_version,
                    instance: response.instance,
                });
            return self.instance_state_limit_error();
        }
        self.respond(json!({
            "abi_version": super::SERVER_ABI_VERSION,
            "api_version": response.api_version,
            "operation": "create_instance",
            "instance": response.instance.get().to_string(),
        }))
    }

    fn step(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let instance = match parse_instance(request.get("instance")) {
            Ok(instance) => instance,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let limits = self.limits.expect("initialized limits");
        let response = match self
            .runtime
            .as_mut()
            .expect("initialized runtime")
            .step_view(StepRequest {
                api_version,
                instance,
            }) {
            Ok(response) => response,
            Err(error) => return self.respond(api_error_response(api_version, error)),
        };
        if !json_fits_byte_limit(
            &SnapshotProjection(response.snapshot()),
            limits.max_instance_state_bytes,
        ) {
            drop(response);
            let _ = self
                .runtime
                .as_mut()
                .expect("initialized runtime")
                .release_instance(ReleaseInstanceRequest {
                    api_version,
                    instance,
                });
            return self.instance_state_limit_error();
        }
        let serialized = {
            let projection = StepResponse {
                api_version: response.api_version(),
                result: &response,
            };
            serialize_json_bounded(&projection, limits.max_response_bytes)
        };
        drop(response);
        match serialized {
            Ok(bytes) => bytes,
            Err(_) => self.response_limit_error(),
        }
    }

    fn run(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let instance = match parse_instance(request.get("instance")) {
            Ok(instance) => instance,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let max_ticks = match decimal_u64(request.get("max_ticks"), "max_ticks") {
            Ok(ticks) => ticks,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let response = match self
            .runtime
            .as_mut()
            .expect("initialized runtime")
            .run_view(RunRequest {
                api_version,
                instance,
                max_ticks,
            }) {
            Ok(response) => response,
            Err(error) => return self.respond(api_error_response(api_version, error)),
        };
        let maximum_state = self
            .limits
            .expect("initialized limits")
            .max_instance_state_bytes;
        if !json_fits_byte_limit(&SnapshotProjection(&response.snapshot), maximum_state) {
            drop(response);
            let _ = self
                .runtime
                .as_mut()
                .expect("initialized runtime")
                .release_instance(ReleaseInstanceRequest {
                    api_version,
                    instance,
                });
            return self.instance_state_limit_error();
        }
        let serialized = serialize_json_bounded(
            &RunResponse {
                api_version: response.api_version,
                result: &response,
            },
            self.limits.expect("initialized limits").max_response_bytes,
        );
        drop(response);
        match serialized {
            Ok(bytes) => bytes,
            Err(_) => self.response_limit_error(),
        }
    }

    fn snapshot(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let instance = match parse_instance(request.get("instance")) {
            Ok(instance) => instance,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        let snapshot = match self.snapshot_projection_view(instance, api_version) {
            Ok(snapshot) => snapshot,
            Err(error) => return self.respond(api_error_response(api_version, error)),
        };
        if !self.snapshot_fits_state_limit(&snapshot) {
            let _ = self
                .runtime
                .as_mut()
                .expect("initialized runtime")
                .release_instance(ReleaseInstanceRequest {
                    api_version,
                    instance,
                });
            return self.instance_state_limit_error();
        }
        self.respond_serialized(&SnapshotResponse {
            api_version,
            snapshot: &snapshot,
        })
    }

    fn release_program(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let program = match parse_program(request.get("program")) {
            Ok(program) => program,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        match self
            .runtime
            .as_mut()
            .expect("initialized runtime")
            .release_program(ReleaseProgramRequest {
                api_version,
                program,
            }) {
            Ok(response) => self.respond(json!({
                "abi_version": super::SERVER_ABI_VERSION,
                "api_version": response.api_version,
                "operation": "release_program",
            })),
            Err(error) => self.respond(api_error_response(api_version, error)),
        }
    }

    fn release_instance(&mut self, request: &Map<String, Value>, api_version: u32) -> Vec<u8> {
        let instance = match parse_instance(request.get("instance")) {
            Ok(instance) => instance,
            Err(error) => return self.respond(error_response(error.0, error.1, Value::Null)),
        };
        match self
            .runtime
            .as_mut()
            .expect("initialized runtime")
            .release_instance(ReleaseInstanceRequest {
                api_version,
                instance,
            }) {
            Ok(response) => self.respond(json!({
                "abi_version": super::SERVER_ABI_VERSION,
                "api_version": response.api_version,
                "operation": "release_instance",
            })),
            Err(error) => self.respond(api_error_response(api_version, error)),
        }
    }

    fn snapshot_projection_view(
        &self,
        instance: InstanceHandle,
        api_version: u32,
    ) -> Result<codegrid_runtime_api::RuntimeSnapshotView<'_>, ApiError> {
        let view = self
            .runtime
            .as_ref()
            .expect("initialized runtime")
            .snapshot_projection_view(SnapshotRequest {
                api_version,
                instance,
            })?;
        Ok(view)
    }

    fn instance_snapshot_fits(&self, instance: InstanceHandle, api_version: u32) -> bool {
        self.snapshot_projection_view(instance, api_version)
            .is_ok_and(|snapshot| self.snapshot_fits_state_limit(&snapshot))
    }

    fn snapshot_fits_state_limit(
        &self,
        snapshot: &codegrid_runtime_api::RuntimeSnapshotView<'_>,
    ) -> bool {
        let maximum = self
            .limits
            .expect("initialized limits")
            .max_instance_state_bytes;
        json_fits_byte_limit(&SnapshotProjection(snapshot), maximum)
    }

    fn try_respond_serialized<T: Serialize + ?Sized>(&self, value: &T) -> Result<Vec<u8>, ()> {
        let maximum = self
            .limits
            .map(|limits| limits.max_response_bytes)
            .unwrap_or(MAX_RESPONSE_BYTES);
        serialize_json_bounded(value, maximum).map_err(|_| ())
    }

    fn respond_serialized<T: Serialize + ?Sized>(&self, value: &T) -> Vec<u8> {
        match self.try_respond_serialized(value) {
            Ok(bytes) => bytes,
            Err(()) => self.response_limit_error(),
        }
    }

    fn respond(&self, value: Value) -> Vec<u8> {
        let maximum = self
            .limits
            .map(|limits| limits.max_response_bytes)
            .unwrap_or(MAX_RESPONSE_BYTES);
        match serialize_json_bounded(&value, maximum) {
            Ok(bytes) => bytes,
            Err(_) => serialize_json_bounded(
                &error_response(
                    "response_payload_limit_exceeded",
                    "serialized response exceeds the configured server byte limit",
                    Value::Null,
                ),
                maximum,
            )
            .unwrap_or_default(),
        }
    }

    fn response_limit_error(&self) -> Vec<u8> {
        self.respond(error_response(
            "response_payload_limit_exceeded",
            "serialized response exceeds the configured server byte limit",
            Value::Null,
        ))
    }

    fn instance_state_limit_error(&self) -> Vec<u8> {
        self.respond(error_response(
            "instance_state_limit_exceeded",
            "serialized instance state exceeds its configured byte limit; the instance was released",
            Value::Null,
        ))
    }
}

fn parse_server_limits(
    value: Option<&Value>,
) -> Result<ServerLimits, (&'static str, &'static str)> {
    let object = value
        .and_then(Value::as_object)
        .ok_or(("invalid_host_limits", "host_limits must be a JSON object"))?;
    let max_source_bytes = bounded_usize(
        object.get("max_source_bytes"),
        "max_source_bytes",
        MAX_SOURCE_BYTES,
    )?;
    let max_compiled_programs = bounded_usize(
        object.get("max_compiled_programs"),
        "max_compiled_programs",
        MAX_COMPILED_PROGRAMS,
    )?;
    let max_instances = bounded_usize(object.get("max_instances"), "max_instances", MAX_INSTANCES)?;
    let max_input_bytes = bounded_usize(
        object.get("max_input_bytes"),
        "max_input_bytes",
        MAX_INPUT_BYTES,
    )?;
    let max_initial_memory_entries = bounded_usize(
        object.get("max_initial_memory_entries"),
        "max_initial_memory_entries",
        MAX_INITIAL_MEMORY_ENTRIES,
    )?;
    let max_run_ticks = bounded_decimal_u64(
        object.get("max_run_ticks_per_call"),
        "max_run_ticks_per_call",
        MAX_RUN_TICKS_PER_CALL,
        true,
    )?;
    let max_total_ticks = bounded_decimal_u64(
        object.get("max_total_ticks_per_instance"),
        "max_total_ticks_per_instance",
        MAX_TOTAL_TICKS_PER_INSTANCE,
        true,
    )?;
    let max_work_units = bounded_decimal_u64(
        object.get("max_work_units_per_call"),
        "max_work_units_per_call",
        MAX_WORK_UNITS_PER_CALL,
        true,
    )?;
    let max_response_bytes = bounded_usize(
        object.get("max_response_bytes"),
        "max_response_bytes",
        MAX_RESPONSE_BYTES,
    )?;
    let max_instance_state_bytes = bounded_usize(
        object.get("max_instance_state_bytes"),
        "max_instance_state_bytes",
        MAX_RESPONSE_BYTES,
    )?;
    if max_response_bytes < MIN_RESPONSE_BYTES
        || max_instance_state_bytes < MIN_RESPONSE_BYTES
        || max_instance_state_bytes > max_response_bytes
    {
        return Err((
            "invalid_host_limit",
            "response and instance-state limits must be at least 256 bytes, and instance state must not exceed the response limit",
        ));
    }
    Ok(ServerLimits {
        runtime: HostLimits::new(
            max_source_bytes,
            max_compiled_programs,
            max_instances,
            max_input_bytes,
            max_initial_memory_entries,
            NonZeroU64::new(max_run_ticks).expect("validated positive limit"),
            NonZeroU64::new(max_total_ticks).expect("validated positive limit"),
            NonZeroU64::new(max_work_units).expect("validated positive limit"),
        ),
        max_response_bytes,
        max_instance_state_bytes,
    })
}

fn parse_program(value: Option<&Value>) -> Result<ProgramHandle, (&'static str, &'static str)> {
    ProgramHandle::from_raw(decimal_u64(value, "program")?).ok_or((
        "invalid_program_handle",
        "program must be a nonzero u64 decimal string",
    ))
}

fn parse_instance(value: Option<&Value>) -> Result<InstanceHandle, (&'static str, &'static str)> {
    InstanceHandle::from_raw(decimal_u64(value, "instance")?).ok_or((
        "invalid_instance_handle",
        "instance must be a nonzero u64 decimal string",
    ))
}
#[cfg(test)]
mod tests {
    use super::{
        decode_input, json_fits_byte_limit, serialize_json_bounded, ParsedInput, ParsedRequest,
        RequestObject, MAX_INPUT_BYTES,
    };
    use serde_json::json;

    #[test]
    fn bounded_json_counter_and_writer_stop_at_the_configured_limit() {
        let small = json!({"message": "fits"});
        let expected = serde_json::to_vec(&small).expect("test JSON must serialize");
        assert!(json_fits_byte_limit(&small, expected.len()));
        assert!(!json_fits_byte_limit(&small, expected.len() - 1));
        assert_eq!(
            serialize_json_bounded(&small, expected.len()).expect("JSON fits exactly"),
            expected
        );

        let large = json!({"message": "x".repeat(1024)});
        assert!(!json_fits_byte_limit(&large, 256));
        assert!(serialize_json_bounded(&large, 256).is_err());
    }

    #[test]
    fn oversized_input_arrays_are_rejected_while_parsing_without_value_nodes() {
        let mut request = String::with_capacity(MAX_INPUT_BYTES * 2 + 96);
        request.push_str(
            r#"{"abi_version":4,"api_version":3,"operation":"create_instance","input":["#,
        );
        for index in 0..=MAX_INPUT_BYTES {
            if index != 0 {
                request.push(',');
            }
            request.push('0');
        }
        request.push_str("]}");

        let ParsedRequest::Object(RequestObject { fields, input, .. }) =
            serde_json::from_str(&request).expect("oversized array is valid JSON")
        else {
            panic!("top-level request object must be retained");
        };
        assert_eq!(fields.len(), 3, "only known scalar fields are retained");
        let Some(ParsedInput::Array {
            bytes,
            too_many,
            invalid_byte,
        }) = input
        else {
            panic!("input array shape must be recorded");
        };
        assert_eq!(bytes.len(), MAX_INPUT_BYTES);
        assert!(too_many);
        assert!(!invalid_byte);
        assert_eq!(
            decode_input(Some(ParsedInput::Array {
                bytes,
                too_many,
                invalid_byte,
            }))
            .expect_err("input length must be rejected before Runtime API validation")
            .0,
            "input_payload_limit_exceeded"
        );
    }

    #[test]
    fn request_parser_preserves_input_errors_and_ignores_nested_unknown_values() {
        let nested = "0,".repeat(20_000);
        let request = format!(
            r#"{{"abi_version":4,"api_version":3,"operation":"check","source":"~> ;","input":{{"ignored":[{nested}0]}},"unknown":{{"deep":[[{nested}0]]}}}}"#
        );
        let ParsedRequest::Object(RequestObject { fields, input, .. }) =
            serde_json::from_str(&request).expect("nested unknown values are valid JSON")
        else {
            panic!("top-level request object must be retained");
        };
        assert_eq!(fields.len(), 4, "unknown nested fields are not retained");
        assert!(matches!(input.as_ref(), Some(ParsedInput::NotArray)));
        assert_eq!(
            decode_input(input)
                .expect_err("object input must remain invalid")
                .0,
            "invalid_input"
        );

        let request =
            r#"{"abi_version":4,"api_version":3,"operation":"create_instance","input":[0,256]}"#;
        let ParsedRequest::Object(RequestObject { input, .. }) =
            serde_json::from_str(request).expect("byte value is valid JSON")
        else {
            panic!("top-level request object must be retained");
        };
        let Some(ParsedInput::Array {
            bytes,
            too_many,
            invalid_byte,
        }) = input
        else {
            panic!("input array shape must be recorded");
        };
        assert_eq!(bytes, [0]);
        assert!(!too_many);
        assert!(invalid_byte);
        assert_eq!(
            decode_input(Some(ParsedInput::Array {
                bytes,
                too_many,
                invalid_byte,
            }))
            .expect_err("out-of-range byte must remain invalid")
            .0,
            "invalid_input_byte"
        );
    }

    #[test]
    fn host_limit_parser_skips_unrecognized_nested_data() {
        let nested = "0,".repeat(20_000);
        let request = format!(
            r#"{{"abi_version":4,"api_version":3,"operation":"initialize","host_limits":{{"max_source_bytes":4096,"max_compiled_programs":8,"max_instances":8,"max_input_bytes":1024,"max_initial_memory_entries":65536,"max_run_ticks_per_call":"100","max_total_ticks_per_instance":"1000","max_work_units_per_call":"1000","max_response_bytes":1048576,"max_instance_state_bytes":524288,"ignored":{{"deep":[[{nested}0]]}}}}}}"#
        );
        let ParsedRequest::Object(RequestObject { fields, .. }) = serde_json::from_str(&request)
            .expect("host limits with nested extension are valid JSON")
        else {
            panic!("top-level request object must be retained");
        };
        let host_limits = fields["host_limits"]
            .as_object()
            .expect("host limits object must be retained");
        assert_eq!(
            host_limits.len(),
            10,
            "only accepted limit fields are retained"
        );
    }
}
