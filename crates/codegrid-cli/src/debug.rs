//! JSON-lines process boundary for editor debugging over the shared VM.
use super::*;
use std::io::{self, BufRead, Write};

const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, serde::Serialize)]
struct DebugError {
    code: &'static str,
    error_number: Option<&'static str>,
    message: String,
}

impl DebugError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            error_number: error_number("debug", code),
            message: message.into(),
        }
    }
}

impl From<&str> for DebugError {
    fn from(message: &str) -> Self {
        Self::new("debug.invalid_configuration", message)
    }
}

impl From<String> for DebugError {
    fn from(message: String) -> Self {
        Self::new("debug.invalid_configuration", message)
    }
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Launch {
        source: String,
        input: Vec<u8>,
        seed: String,
        custom_limit: String,
        max_work_units: String,
        max_ticks: String,
    },
    Step,
    Snapshot,
    Disconnect,
}

#[derive(Default)]
struct Session {
    vm: Option<Vm>,
    work_limit: u64,
    tick_limit: u64,
}

impl Session {
    fn request(&mut self, request: Request) -> Result<JsonValue, DebugError> {
        match request {
            Request::Launch {
                source,
                input,
                seed,
                custom_limit,
                max_work_units,
                max_ticks,
            } => {
                if self.vm.is_some() {
                    return Err(DebugError::new(
                        "debug.already_loaded",
                        "a program is already loaded",
                    ));
                }
                let seed = decimal(&seed, false)?;
                let custom_limit = decimal(&custom_limit, true)?;
                let work_limit = decimal(&max_work_units, true)?;
                let tick_limit = decimal(&max_ticks, true)?;
                let compilation = match codegrid_compiler::compile_with_symbols(&source) {
                    Ok(compilation) => compilation,
                    Err(diagnostics) => {
                        return Ok(
                            json!({"diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>() }),
                        )
                    }
                };
                let lines = LineIndex::new(&source);
                let locations: Vec<_> = compilation.source_map.iter().map(|(path, span)| {
                    let (line, column) = position(&source, &lines, span.start);
                    let (end_line, end_column) = position(&source, &lines, span.end);
                    json!({"path":path,"line":line,"column":column,"endLine":end_line,"endColumn":end_column})
                }).collect();
                self.vm = Some(
                    Vm::new(
                        compilation.program,
                        input,
                        VmConfig::new(
                            seed,
                            NonZeroU64::new(custom_limit).ok_or("custom limit must be positive")?,
                        ),
                    )
                    .map_err(|error| {
                        DebugError::new(
                            "debug.vm_initialization_failed",
                            format!("VM initialization failed: {error:?}"),
                        )
                    })?,
                );
                self.work_limit = work_limit;
                self.tick_limit = tick_limit;
                Ok(
                    json!({"locations":locations,"snapshot":snapshot_json(&self.vm.as_ref().ok_or_else(|| DebugError::new("debug.no_program", "no program loaded"))?.snapshot())}),
                )
            }
            Request::Step => {
                let vm = self
                    .vm
                    .as_mut()
                    .ok_or_else(|| DebugError::new("debug.no_program", "no program loaded"))?;
                if vm.status() == VmStatus::Running && vm.committed_ticks() >= self.tick_limit {
                    return Err(DebugError::new(
                        "debug.tick_limit_exceeded",
                        "configured global tick limit reached",
                    ));
                }
                let step = vm.step_with_work_limit(NonZeroU64::new(self.work_limit).ok_or("work limit must be positive")?).map_err(|_| DebugError::new("debug.work_limit_exceeded", "global tick exceeded the configured work-unit limit; the tick was rolled back"))?;
                Ok(
                    json!({"snapshot":snapshot_json(&vm.snapshot()),"events":step.events.iter().map(event_json).collect::<Vec<_>>(),"newly_emitted_output":step.newly_emitted_output}),
                )
            }
            Request::Snapshot => Ok(
                json!({"snapshot":snapshot_json(&self.vm.as_ref().ok_or_else(|| DebugError::new("debug.no_program", "no program loaded"))?.snapshot())}),
            ),
            Request::Disconnect => {
                self.vm = None;
                Ok(json!({"disconnected":true}))
            }
        }
    }
}

fn decimal(value: &str, positive: bool) -> Result<u64, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("limits and seed must be canonical unsigned decimal strings".to_owned());
    }
    let number = value
        .parse::<u64>()
        .map_err(|_| "integer exceeds u64".to_owned())?;
    if positive && number == 0 {
        return Err("execution limits must be positive".to_owned());
    }
    Ok(number)
}

fn position(source: &str, lines: &LineIndex, offset: usize) -> (usize, usize) {
    let (line, _) = lines.line_and_byte_column(source, offset);
    let start = lines.line_start(line).unwrap_or(0);
    (line + 1, source[start..offset].encode_utf16().count() + 1)
}

pub(super) fn serve() -> i32 {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut session = Session::default();
    loop {
        let mut bytes = Vec::new();
        let mut bounded = std::io::Read::take(&mut reader, (MAX_REQUEST_BYTES + 1) as u64);
        let count = match bounded.read_until(b'\n', &mut bytes) {
            Ok(count) => count,
            Err(error) => {
                eprintln!(
                    "error: [{error_number}] [debug.transport_io] {error}",
                    error_number = codegrid_model::error_number("debug", "debug.transport_io")
                        .expect("Registered process error")
                );
                return EXIT_IO_ERROR;
            }
        };
        if count == 0 {
            return EXIT_SUCCESS;
        }
        if bytes.len() > MAX_REQUEST_BYTES {
            let response = json!({"debug_protocol_version":2,"error":DebugError::new("debug.request_limit_exceeded", "request line exceeds 4 MiB")});
            let _ = serde_json::to_writer(&mut writer, &response);
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
            return EXIT_INVALID_ARGUMENTS;
        }
        let request = serde_json::from_slice::<Request>(&bytes);
        let disconnect = matches!(request, Ok(Request::Disconnect));
        let result = request
            .map_err(|error| DebugError::new("debug.invalid_request", error.to_string()))
            .and_then(|request| session.request(request));
        let response = match result {
            Ok(body) => json!({"debug_protocol_version":2,"body":body}),
            Err(error) => json!({"debug_protocol_version":2,"error":error}),
        };
        if serde_json::to_writer(&mut writer, &response).is_err()
            || writer.write_all(b"\n").is_err()
            || writer.flush().is_err()
        {
            eprintln!(
                "error: [{error_number}] [debug.transport_io] cannot write debug response",
                error_number = codegrid_model::error_number("debug", "debug.transport_io")
                    .expect("Registered process error")
            );
            return EXIT_IO_ERROR;
        }
        if disconnect {
            return EXIT_SUCCESS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(source: &str) -> Request {
        Request::Launch {
            source: source.to_owned(),
            input: vec![65],
            seed: "18446744073709551615".into(),
            custom_limit: "1000".into(),
            max_work_units: "100000".into(),
            max_ticks: "1000".into(),
        }
    }

    #[test]
    fn debug_steps_use_shared_vm_and_preserve_utf16_source_locations() {
        let mut session = Session::default();
        let loaded = session
            .request(launch("/*\u{9983}\u{6aaa}*/ ~> , . ;\r\n"))
            .unwrap();
        assert_eq!(
            loaded["locations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["path"] == "@main[0]")
                .unwrap()["column"],
            8
        );
        assert_eq!(loaded["snapshot"]["committed_ticks"], "0");
        for _ in 0..4 {
            session.request(Request::Step).unwrap();
        }
        let snapshot = session.request(Request::Snapshot).unwrap();
        assert_eq!(snapshot["snapshot"]["status"], "halted");
        assert_eq!(snapshot["snapshot"]["output"], json!([65]));
    }

    #[test]
    fn debug_rejects_bad_requests_and_reports_compiler_diagnostics() {
        let mut session = Session::default();
        assert!(session.request(Request::Step).is_err());
        assert!(decimal("01", false).is_err());
        assert!(decimal("0", true).is_err());
        assert!(!session.request(launch("~x")).unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
