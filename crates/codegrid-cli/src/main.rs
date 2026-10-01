use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::num::NonZeroU64;
use std::path::PathBuf;

use codegrid_compiler::compile;
use codegrid_ir::{Board, BoardId, CodeGridId, ScopedProgram};
use codegrid_level_api::{LevelApi, SafetyProfile, LEVEL_API_VERSION};
use codegrid_model::error_number;
use codegrid_model::{
    AttachmentInstruction, BoundaryMode, Direction, InstructionStackItem, PrimaryInstruction, Slot,
};
use codegrid_syntax::{Diagnostic, LineIndex, Severity};
use codegrid_vm::{
    ExecutionScope, InstructionKind, MemoryAddress, MemoryLocationId, MemorySpaceId,
    MetricCounterOverflow, RuntimeError, RuntimeErrorKind, RuntimeMetrics, StaticCellId,
    ThreadPhaseSnapshot, ThreadSnapshot, Vm, VmConfig, VmEvent, VmFault, VmSnapshot, VmStatus,
};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

mod debug;

const EXIT_SUCCESS: i32 = 0;
const EXIT_INVALID_ARGUMENTS: i32 = 2;
const EXIT_IO_ERROR: i32 = 3;
const EXIT_STATIC_ERROR: i32 = 4;
const EXIT_RUNTIME_ERROR: i32 = 5;
const EXIT_TICK_LIMIT: i32 = 6;
const EXIT_WORK_LIMIT: i32 = 7;
const EXIT_VM_FAULT: i32 = 8;
const EXIT_EVALUATION_FAILED: i32 = 9;
const EXIT_LEVEL_REJECTED: i32 = 10;
const EXIT_RESOURCE_LIMIT: i32 = 11;
const MAX_PROFILE_FILE_BYTES: usize = 65_536;
const USAGE: &str = concat!(
    "Usage:\n",
    "  codegrid check <program.cg>\n",
    "  codegrid debug --stdio\n",
    "  codegrid run <program.cg> --boundary <exit|wrap> --seed <u64>\n",
    "    --custom-limit <positive-u64> --max-ticks <positive-u64>\n",
    "    --max-work-units <positive-u64>\n",
    "    [--input <byte,byte,...> | --input-file <json-file>]\n",
    "    [--initial-memory-file <json-file>]\n",
    "  codegrid evaluate <level.json> <program.cg>\n",
    "    --mode <debug|official> --boundary <exit|wrap> --seed <u64>\n",
    "    --custom-limit <positive-u64> --limits-file <trusted-profile.json>\n",
    "    [--format <json|human>]\n",
);

struct RunOptions {
    source_path: PathBuf,
    input: InputSource,
    initial_memory_file: Option<PathBuf>,
    boundary: BoundaryMode,
    seed: u64,
    custom_limit: NonZeroU64,
    max_ticks: NonZeroU64,
    max_work_units: NonZeroU64,
}

struct EvaluateOptions {
    level_path: PathBuf,
    source_path: PathBuf,
    mode: EvaluationModeArg,
    boundary: BoundaryMode,
    seed: u64,
    custom_limit: NonZeroU64,
    limits_path: PathBuf,
    format: OutputFormat,
}

#[derive(Clone, Copy)]
enum EvaluationModeArg {
    Debug,
    Official,
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Json,
    Human,
}

enum InputSource {
    Bytes(Vec<u8>),
    JsonFile(PathBuf),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialMemoryEntry {
    address: String,
    value: u8,
}

enum InputFileError {
    Io(std::io::Error),
    InvalidJson(serde_json::Error),
    HostData(String),
}

enum EvaluationFileError {
    Io(std::io::Error),
    TooLarge { limit: u64, actual: usize },
    InvalidUtf8(std::string::FromUtf8Error),
    BomNotAllowed,
}

enum Command {
    Debug,
    Check(PathBuf),
    Run(RunOptions),
    Evaluate(EvaluateOptions),
    Help,
}

fn main() {
    let exit_code = run(std::env::args_os().skip(1).collect());
    std::process::exit(exit_code);
}

fn run(arguments: Vec<OsString>) -> i32 {
    let command = match parse_arguments(arguments) {
        Ok(command) => command,
        Err(message) => {
            eprintln!(
                "error: [{error_number}] [cli.invalid_arguments] {message}",
                error_number = codegrid_model::error_number("cli", "cli.invalid_arguments")
                    .expect("Registered process error")
            );
            print_usage();
            return EXIT_INVALID_ARGUMENTS;
        }
    };

    match command {
        Command::Debug => debug::serve(),
        Command::Help => {
            println!("{USAGE}");
            EXIT_SUCCESS
        }
        Command::Check(path) => check_file(&path),
        Command::Run(options) => run_file(options),
        Command::Evaluate(options) => evaluate_files(options),
    }
}

fn parse_arguments(arguments: Vec<OsString>) -> Result<Command, String> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Err("expected a subcommand: check, run, evaluate or debug".to_owned());
    };
    let command = command
        .to_str()
        .ok_or_else(|| "subcommand must be valid UTF-8".to_owned())?;
    if matches!(command, "--help" | "-h" | "help") {
        return Ok(Command::Help);
    }

    match command {
        "debug" => {
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--stdio"))
                || arguments.next().is_some()
            {
                return Err("debug requires exactly --stdio".to_owned());
            }
            Ok(Command::Debug)
        }
        "check" => {
            let Some(path) = arguments.next() else {
                return Err("check requires a source file path".to_owned());
            };
            if arguments.next().is_some() {
                return Err("check accepts exactly one source file path".to_owned());
            }
            Ok(Command::Check(PathBuf::from(path)))
        }
        "run" => parse_run_arguments(&mut arguments),
        "evaluate" => parse_evaluate_arguments(&mut arguments),
        _ => Err(format!("unknown subcommand: {command}")),
    }
}

fn parse_run_arguments(arguments: &mut impl Iterator<Item = OsString>) -> Result<Command, String> {
    let Some(source_path) = arguments.next() else {
        return Err("run requires a source file path".to_owned());
    };
    let mut input = None;
    let mut input_file = None;
    let mut initial_memory_file = None;
    let mut boundary = None;
    let mut seed = None;
    let mut custom_limit = None;
    let mut max_ticks = None;
    let mut max_work_units = None;

    while let Some(argument) = arguments.next() {
        let option = argument
            .to_str()
            .ok_or_else(|| "option names must be valid UTF-8".to_owned())?;
        match option {
            "--input" => {
                if input.is_some() {
                    return Err("--input may be specified only once".to_owned());
                }
                let value = next_option_value(arguments, option)?
                    .into_string()
                    .map_err(|_| "--input must be valid UTF-8".to_owned())?;
                input = Some(parse_byte_list(&value)?);
            }
            "--input-file" => {
                if input_file.is_some() {
                    return Err("--input-file may be specified only once".to_owned());
                }
                input_file = Some(PathBuf::from(next_option_value(arguments, option)?));
            }
            "--initial-memory-file" => {
                if initial_memory_file.is_some() {
                    return Err("--initial-memory-file may be specified only once".to_owned());
                }
                initial_memory_file = Some(PathBuf::from(next_option_value(arguments, option)?));
            }
            "--boundary" => {
                if boundary.is_some() {
                    return Err("--boundary may be specified only once".to_owned());
                }
                boundary = Some(match option_value_text(arguments, option)?.as_str() {
                    "exit" => BoundaryMode::Exit,
                    "wrap" => BoundaryMode::Wrap,
                    _ => return Err("--boundary must be exit or wrap".to_owned()),
                });
            }
            "--seed" => {
                if seed.is_some() {
                    return Err("--seed may be specified only once".to_owned());
                }
                seed = Some(parse_unsigned_option(arguments, option)?);
            }
            "--custom-limit" => {
                if custom_limit.is_some() {
                    return Err("--custom-limit may be specified only once".to_owned());
                }
                custom_limit = Some(parse_positive_option(arguments, option)?);
            }
            "--max-ticks" => {
                if max_ticks.is_some() {
                    return Err("--max-ticks may be specified only once".to_owned());
                }
                max_ticks = Some(parse_positive_option(arguments, option)?);
            }
            "--max-work-units" => {
                if max_work_units.is_some() {
                    return Err("--max-work-units may be specified only once".to_owned());
                }
                max_work_units = Some(parse_positive_option(arguments, option)?);
            }
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(format!("unknown run option: {option}")),
        }
    }

    if input.is_some() && input_file.is_some() {
        return Err("--input and --input-file are mutually exclusive".to_owned());
    }
    let input = match input_file {
        Some(path) => InputSource::JsonFile(path),
        None => InputSource::Bytes(input.unwrap_or_default()),
    };
    Ok(Command::Run(RunOptions {
        source_path: PathBuf::from(source_path),
        input,
        initial_memory_file,
        boundary: boundary.ok_or_else(|| "run requires --boundary".to_owned())?,
        seed: seed.ok_or_else(|| "run requires --seed".to_owned())?,
        custom_limit: custom_limit.ok_or_else(|| "run requires --custom-limit".to_owned())?,
        max_ticks: max_ticks.ok_or_else(|| "run requires --max-ticks".to_owned())?,
        max_work_units: max_work_units.ok_or_else(|| "run requires --max-work-units".to_owned())?,
    }))
}

fn parse_evaluate_arguments(
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<Command, String> {
    let Some(level_path) = arguments.next() else {
        return Err("evaluate requires a level JSON file path".to_owned());
    };
    let Some(source_path) = arguments.next() else {
        return Err("evaluate requires a source file path after the level path".to_owned());
    };
    let mut mode = None;
    let mut boundary = None;
    let mut seed = None;
    let mut custom_limit = None;
    let mut limits_path = None;
    let mut format = None;

    while let Some(argument) = arguments.next() {
        let option = argument
            .to_str()
            .ok_or_else(|| "option names must be valid UTF-8".to_owned())?;
        match option {
            "--mode" => {
                if mode.is_some() {
                    return Err("--mode may be specified only once".to_owned());
                }
                mode = Some(match option_value_text(arguments, option)?.as_str() {
                    "debug" => EvaluationModeArg::Debug,
                    "official" => EvaluationModeArg::Official,
                    _ => return Err("--mode must be debug or official".to_owned()),
                });
            }
            "--boundary" => {
                if boundary.is_some() {
                    return Err("--boundary may be specified only once".to_owned());
                }
                boundary = Some(match option_value_text(arguments, option)?.as_str() {
                    "exit" => BoundaryMode::Exit,
                    "wrap" => BoundaryMode::Wrap,
                    _ => return Err("--boundary must be exit or wrap".to_owned()),
                });
            }
            "--seed" => {
                if seed.is_some() {
                    return Err("--seed may be specified only once".to_owned());
                }
                seed = Some(parse_unsigned_option(arguments, option)?);
            }
            "--custom-limit" => {
                if custom_limit.is_some() {
                    return Err("--custom-limit may be specified only once".to_owned());
                }
                custom_limit = Some(parse_positive_option(arguments, option)?);
            }
            "--limits-file" => {
                if limits_path.is_some() {
                    return Err("--limits-file may be specified only once".to_owned());
                }
                limits_path = Some(PathBuf::from(next_option_value(arguments, option)?));
            }
            "--format" => {
                if format.is_some() {
                    return Err("--format may be specified only once".to_owned());
                }
                format = Some(match option_value_text(arguments, option)?.as_str() {
                    "json" => OutputFormat::Json,
                    "human" => OutputFormat::Human,
                    _ => return Err("--format must be json or human".to_owned()),
                });
            }
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(format!("unknown evaluate option: {option}")),
        }
    }

    Ok(Command::Evaluate(EvaluateOptions {
        level_path: PathBuf::from(level_path),
        source_path: PathBuf::from(source_path),
        mode: mode.ok_or_else(|| "evaluate requires --mode".to_owned())?,
        boundary: boundary.ok_or_else(|| "evaluate requires --boundary".to_owned())?,
        seed: seed.ok_or_else(|| "evaluate requires --seed".to_owned())?,
        custom_limit: custom_limit.ok_or_else(|| "evaluate requires --custom-limit".to_owned())?,
        limits_path: limits_path.ok_or_else(|| "evaluate requires --limits-file".to_owned())?,
        format: format.unwrap_or(OutputFormat::Json),
    }))
}

fn next_option_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn option_value_text(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<String, String> {
    next_option_value(arguments, option)?
        .into_string()
        .map_err(|_| format!("{option} value must be valid UTF-8"))
}

fn parse_unsigned_option(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<u64, String> {
    let value = option_value_text(arguments, option)?;
    parse_canonical_u64(&value).ok_or_else(|| {
        format!("{option} must use canonical unsigned decimal notation within the u64 range")
    })
}

fn parse_positive_option(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<NonZeroU64, String> {
    let value = parse_unsigned_option(arguments, option)?;
    NonZeroU64::new(value).ok_or_else(|| format!("{option} must be greater than zero"))
}

fn parse_canonical_u64(value: &str) -> Option<u64> {
    if value == "0"
        || (!value.is_empty()
            && value.as_bytes()[0].is_ascii_digit()
            && value.as_bytes()[0] != b'0'
            && value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        value.parse().ok()
    } else {
        None
    }
}

fn parse_canonical_signed(value: &str) -> Option<MemoryAddress> {
    let canonical = if value == "0" {
        true
    } else if let Some(magnitude) = value.strip_prefix('-') {
        !magnitude.is_empty()
            && magnitude.as_bytes()[0].is_ascii_digit()
            && magnitude.as_bytes()[0] != b'0'
            && magnitude.bytes().all(|byte| byte.is_ascii_digit())
    } else {
        !value.is_empty()
            && value.as_bytes()[0].is_ascii_digit()
            && value.as_bytes()[0] != b'0'
            && value.bytes().all(|byte| byte.is_ascii_digit())
    };
    canonical.then(|| value.parse().ok()).flatten()
}

fn parse_byte_list(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .enumerate()
        .map(|(index, item)| {
            let item = item.trim();
            parse_canonical_u64(item)
                .filter(|value| *value <= u8::MAX as u64)
                .map(|value| value as u8)
                .ok_or_else(|| {
                    format!(
                        "--input item {} must be a canonical integer from 0 through 255",
                        index + 1
                    )
                })
        })
        .collect()
}

fn read_input_file(path: &PathBuf) -> Result<Vec<u8>, InputFileError> {
    let bytes = std::fs::read(path).map_err(InputFileError::Io)?;
    serde_json::from_slice::<Vec<u8>>(&bytes).map_err(InputFileError::InvalidJson)
}

fn read_initial_memory_file(path: &PathBuf) -> Result<BTreeMap<MemoryAddress, u8>, InputFileError> {
    let bytes = std::fs::read(path).map_err(InputFileError::Io)?;
    let entries = serde_json::from_slice::<Vec<InitialMemoryEntry>>(&bytes)
        .map_err(InputFileError::InvalidJson)?;
    let mut memory = BTreeMap::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let address = parse_canonical_signed(&entry.address).ok_or_else(|| {
            InputFileError::HostData(format!(
                "initial-memory entry {} has a non-canonical signed address",
                index + 1
            ))
        })?;
        if memory.insert(address, entry.value).is_some() {
            return Err(InputFileError::HostData(format!(
                "initial-memory entry {} duplicates an address",
                index + 1
            )));
        }
    }
    Ok(memory
        .into_iter()
        .filter(|(_, value)| *value != 0)
        .collect())
}

fn check_file(path: &PathBuf) -> i32 {
    let source = match read_source(path) {
        Ok(source) => source,
        Err(message) => {
            eprintln!(
                "error: [{error_number}] [cli.source_io] {message}",
                error_number = codegrid_model::error_number("cli", "cli.source_io")
                    .expect("Registered process error")
            );
            return EXIT_IO_ERROR;
        }
    };
    let diagnostics = codegrid_compiler::check(&source);
    if diagnostics.is_empty() {
        println!("{}: valid", path.display());
        return EXIT_SUCCESS;
    }
    print_diagnostics(path, &source, &diagnostics);
    EXIT_STATIC_ERROR
}

fn run_file(options: RunOptions) -> i32 {
    let input = match &options.input {
        InputSource::Bytes(bytes) => bytes.clone(),
        InputSource::JsonFile(path) => {
            match read_input_file(path) {
                Ok(bytes) => bytes,
                Err(InputFileError::Io(error)) => {
                    eprintln!(
                    "error: [{error_number}] [cli.input_io] cannot read input file '{}': {error}",
                    path.display(), error_number = codegrid_model::error_number("cli", "cli.input_io").expect("Registered process error"));
                    return EXIT_IO_ERROR;
                }
                Err(InputFileError::InvalidJson(error)) => {
                    eprintln!(
                    "error: [{error_number}] [cli.input_json] input file '{}' must contain one JSON byte array: {error}",
                    path.display(), error_number = codegrid_model::error_number("cli", "cli.input_json").expect("Registered process error"));
                    return EXIT_INVALID_ARGUMENTS;
                }
                Err(InputFileError::HostData(message)) => {
                    eprintln!(
                        "error: [{error_number}] [cli.input_data] {message}",
                        error_number = codegrid_model::error_number("cli", "cli.input_data")
                            .expect("Registered process error")
                    );
                    return EXIT_INVALID_ARGUMENTS;
                }
            }
        }
    };
    let initial_memory = match options.initial_memory_file.as_ref() {
        Some(path) => match read_initial_memory_file(path) {
            Ok(memory) => memory,
            Err(InputFileError::Io(error)) => {
                eprintln!(
                    "error: [{error_number}] [cli.memory_io] cannot read initial memory file '{}': {error}",
                    path.display(), error_number = codegrid_model::error_number("cli", "cli.memory_io").expect("Registered process error"));
                return EXIT_IO_ERROR;
            }
            Err(InputFileError::InvalidJson(error)) => {
                eprintln!(
                    "error: [{error_number}] [cli.memory_json] initial memory file '{}' must contain one valid JSON memory array: {error}",
                    path.display(), error_number = codegrid_model::error_number("cli", "cli.memory_json").expect("Registered process error"));
                return EXIT_INVALID_ARGUMENTS;
            }
            Err(InputFileError::HostData(message)) => {
                eprintln!(
                    "error: [{error_number}] [cli.memory_data] {message}",
                    error_number = codegrid_model::error_number("cli", "cli.memory_data")
                        .expect("Registered process error")
                );
                return EXIT_INVALID_ARGUMENTS;
            }
        },
        None => BTreeMap::new(),
    };
    let source = match read_source(&options.source_path) {
        Ok(source) => source,
        Err(message) => {
            eprintln!(
                "error: [{error_number}] [cli.source_io] {message}",
                error_number = codegrid_model::error_number("cli", "cli.source_io")
                    .expect("Registered process error")
            );
            return EXIT_IO_ERROR;
        }
    };
    let configuration = configuration_json(&options, &input, &initial_memory);
    let program = match compile(&source) {
        Ok(program) => program,
        Err(diagnostics) => {
            print_diagnostics(&options.source_path, &source, &diagnostics);
            let result = result_json(
                "source_error",
                JsonValue::Null,
                configuration,
                &diagnostics,
                &[],
                &[],
                None,
            );
            if let Err(error) = print_run_result(&result) {
                eprintln!(
                    "error: [{error_number}] [cli.serialization_failed] failed to serialize run result: {error}", error_number = codegrid_model::error_number("cli", "cli.serialization_failed").expect("Registered process error"));
            }
            return EXIT_STATIC_ERROR;
        }
    };
    let config = VmConfig::new(options.boundary, options.seed, options.custom_limit);
    let mut vm = match Vm::with_initial_memory(program, input, initial_memory, config) {
        Ok(vm) => vm,
        Err(error) => {
            eprintln!("error: [{error_number}] [cli.vm_initialization_failed] VM initialization failed: {error:?}", error_number = codegrid_model::error_number("cli", "cli.vm_initialization_failed").expect("Registered process error"));
            return EXIT_RUNTIME_ERROR;
        }
    };
    let run_result =
        vm.run_with_work_limit_detailed(options.max_ticks.get(), options.max_work_units);
    let snapshot = vm.snapshot();
    let (status, yield_reason, exit_code) = match run_result.outcome {
        codegrid_vm::RunOutcome::Halted => ("halted", JsonValue::Null, EXIT_SUCCESS),
        codegrid_vm::RunOutcome::Error if snapshot.fault.is_some() => {
            ("vm_fault", JsonValue::Null, EXIT_VM_FAULT)
        }
        codegrid_vm::RunOutcome::Error => ("runtime_error", JsonValue::Null, EXIT_RUNTIME_ERROR),
        codegrid_vm::RunOutcome::TickLimitReached => {
            ("yielded", json!("tick_slice_exhausted"), EXIT_TICK_LIMIT)
        }
        codegrid_vm::RunOutcome::WorkLimitReached => (
            "yielded",
            json!("work_unit_budget_exhausted"),
            EXIT_WORK_LIMIT,
        ),
        codegrid_vm::RunOutcome::InvalidTickLimit => {
            eprintln!("error: [{error_number}] [cli.invalid_tick_limit] max tick limit must be greater than zero", error_number = codegrid_model::error_number("cli", "cli.invalid_tick_limit").expect("Registered process error"));
            return EXIT_INVALID_ARGUMENTS;
        }
    };
    let events: Vec<_> = run_result.events.iter().map(event_json).collect();
    let newly_emitted_output = run_result.newly_emitted_output;
    let result = result_json(
        status,
        yield_reason,
        configuration,
        &[],
        &events,
        &newly_emitted_output,
        Some(snapshot_json(&snapshot)),
    );
    if let Err(error) = print_run_result(&result) {
        eprintln!("error: [{error_number}] [cli.serialization_failed] failed to serialize run result: {error}", error_number = codegrid_model::error_number("cli", "cli.serialization_failed").expect("Registered process error"));
        return EXIT_RUNTIME_ERROR;
    }
    for error in &snapshot.errors {
        eprintln!("{}", runtime_error_summary(error));
    }
    exit_code
}

fn evaluate_files(options: EvaluateOptions) -> i32 {
    let profile_text = match read_limited_utf8_file(
        &options.limits_path,
        MAX_PROFILE_FILE_BYTES as u64,
    ) {
        Ok(text) => text,
        Err(EvaluationFileError::Io(error)) => {
            eprintln!(
                "error: [{error_number}] [cli.limits_io] cannot read trusted safety profile '{}': {error}",
                options.limits_path.display(), error_number = codegrid_model::error_number("cli", "cli.limits_io").expect("Registered process error"));
            return EXIT_IO_ERROR;
        }
        Err(EvaluationFileError::TooLarge { limit, actual }) => {
            eprintln!("error: [{error_number}] [cli.profile_too_large] trusted safety profile contains at least {actual} bytes; the CLI limit is {limit} bytes", error_number = codegrid_model::error_number("cli", "cli.profile_too_large").expect("Registered process error"));
            return EXIT_INVALID_ARGUMENTS;
        }
        Err(EvaluationFileError::InvalidUtf8(error)) => {
            eprintln!(
                "error: [cli.profile_utf8] trusted safety profile '{}' is not UTF-8: {error}",
                options.limits_path.display()
            );
            return EXIT_INVALID_ARGUMENTS;
        }
        Err(EvaluationFileError::BomNotAllowed) => {
            eprintln!("error: [{error_number}] [cli.profile_bom] trusted safety profile '{}' must not start with a UTF-8 BOM", options.limits_path.display(), error_number = codegrid_model::error_number("cli", "cli.profile_bom").expect("Registered process error"));
            return EXIT_INVALID_ARGUMENTS;
        }
    };
    let profile = match SafetyProfile::from_json(&profile_text) {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!(
                "error: [{}] [{}] invalid trusted safety profile '{}': {}",
                error_number("level", error.code).expect("Registered level API error"),
                error.code,
                options.limits_path.display(),
                error.message
            );
            return EXIT_INVALID_ARGUMENTS;
        }
    };
    let level_text = match read_limited_utf8_file(&options.level_path, profile.max_level_bytes) {
        Ok(text) => text,
        Err(error) => return report_evaluation_file_error(&options.level_path, "level", error),
    };
    let source_text = match read_limited_utf8_file(&options.source_path, profile.max_source_bytes) {
        Ok(text) => text,
        Err(error) => return report_evaluation_file_error(&options.source_path, "source", error),
    };

    let work_budget = profile.max_work_per_call;
    let mut api = match LevelApi::new(profile) {
        Ok(api) => api,
        Err(error) => {
            eprintln!(
                "error: [{}] [{}] cannot initialize level evaluator: {}",
                error_number("level", error.code).expect("Registered level API error"),
                error.code,
                error.message
            );
            return EXIT_INVALID_ARGUMENTS;
        }
    };
    let mut handles: Vec<(&'static str, String)> = Vec::new();
    let evaluation = (|| -> Result<(JsonValue, i32), String> {
        let response =
            level_api_request(&mut api, "load_level", json!({ "level_json": level_text }))?;
        match response_status(&response) {
            Some("level_rejected") => return Ok((response, EXIT_LEVEL_REJECTED)),
            Some("error") => return Ok((response.clone(), level_api_error_exit(&response))),
            Some("ok") => {}
            other => return Err(format!("unexpected load_level response status: {other:?}")),
        }
        let level_handle = response_handle(&response)?;
        handles.push(("level", level_handle.clone()));

        let response = level_api_request(
            &mut api,
            "compile_program",
            json!({ "source": source_text }),
        )?;
        match response_status(&response) {
            Some("source_rejected") => return Ok((response, EXIT_STATIC_ERROR)),
            Some("error") => return Ok((response.clone(), level_api_error_exit(&response))),
            Some("ok") => {}
            other => {
                return Err(format!(
                    "unexpected compile_program response status: {other:?}"
                ))
            }
        }
        let program_handle = response_handle(&response)?;
        handles.push(("program", program_handle.clone()));

        let response = level_api_request(
            &mut api,
            "start_evaluation",
            json!({
                "level": level_handle,
                "program": program_handle,
                "mode": match options.mode { EvaluationModeArg::Debug => "Debug", EvaluationModeArg::Official => "Official" },
                "boundary_mode": boundary_api_name(options.boundary),
                "shuffle_seed": options.seed.to_string(),
                "custom_execution_limit": options.custom_limit.get().to_string(),
            }),
        )?;
        match response_status(&response) {
            Some("source_rejected") => return Ok((response, EXIT_STATIC_ERROR)),
            Some("level_rejected") => return Ok((response, EXIT_LEVEL_REJECTED)),
            Some("error") => return Ok((response.clone(), level_api_error_exit(&response))),
            Some("ok") => {}
            other => {
                return Err(format!(
                    "unexpected start_evaluation response status: {other:?}"
                ))
            }
        }
        let evaluation_handle = response_handle(&response)?;
        handles.push(("evaluation", evaluation_handle.clone()));

        loop {
            let response = level_api_request(
                &mut api,
                "advance_evaluation",
                json!({
                    "evaluation": evaluation_handle,
                    "work_budget": work_budget.to_string(),
                }),
            )?;
            match response_status(&response) {
                Some("pending") => continue,
                Some("result") => {
                    let exit_code = evaluation_result_exit(&response);
                    return Ok((response, exit_code));
                }
                Some("level_rejected") => return Ok((response, EXIT_LEVEL_REJECTED)),
                Some("source_rejected") => return Ok((response, EXIT_STATIC_ERROR)),
                Some("error") => return Ok((response.clone(), level_api_error_exit(&response))),
                other => {
                    return Err(format!(
                        "unexpected advance_evaluation response status: {other:?}"
                    ))
                }
            }
        }
    })();

    for (kind, handle) in handles.into_iter().rev() {
        let _ = level_api_request(
            &mut api,
            "release",
            json!({ "kind": kind, "handle": handle }),
        );
    }
    let _ = level_api_request(&mut api, "shutdown", json!({}));

    match evaluation {
        Ok((response, exit_code)) => {
            match options.format {
                OutputFormat::Json => match serde_json::to_string_pretty(&response) {
                    Ok(serialized) => println!("{serialized}"),
                    Err(error) => {
                        eprintln!("error: [{error_number}] [cli.serialization_failed] failed to serialize level evaluation result: {error}", error_number = codegrid_model::error_number("cli", "cli.serialization_failed").expect("Registered process error"));
                        return EXIT_VM_FAULT;
                    }
                },
                OutputFormat::Human => print_evaluation_human(&response),
            }
            exit_code
        }
        Err(message) => {
            eprintln!(
                "error: [{error_number}] [cli.level_api_protocol] {message}",
                error_number = codegrid_model::error_number("cli", "cli.level_api_protocol")
                    .expect("Registered process error")
            );
            EXIT_VM_FAULT
        }
    }
}

fn read_limited_utf8_file(path: &PathBuf, limit: u64) -> Result<String, EvaluationFileError> {
    let file = std::fs::File::open(path).map_err(EvaluationFileError::Io)?;
    read_limited_utf8(file, limit)
}

fn read_limited_utf8(reader: impl Read, limit: u64) -> Result<String, EvaluationFileError> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(EvaluationFileError::Io)?;
    if bytes.len() as u128 > limit as u128 {
        return Err(EvaluationFileError::TooLarge {
            limit,
            actual: bytes.len(),
        });
    }
    let text = String::from_utf8(bytes).map_err(EvaluationFileError::InvalidUtf8)?;
    if text.starts_with('\u{feff}') {
        return Err(EvaluationFileError::BomNotAllowed);
    }
    Ok(text)
}

fn report_evaluation_file_error(path: &PathBuf, label: &str, error: EvaluationFileError) -> i32 {
    match error {
        EvaluationFileError::Io(error) => {
            eprintln!(
                "error: [{error_number}] [cli.{label}_io] cannot read {label} file '{}': {error}",
                path.display(),
                error_number =
                    error_number("cli", &format!("cli.{label}_io")).expect("Registered file error")
            );
            EXIT_IO_ERROR
        }
        EvaluationFileError::TooLarge { limit, actual } => {
            eprintln!("error: [{error_number}] [cli.{label}_too_large] {label} file '{}' contains at least {actual} bytes; the trusted profile limit is {limit} bytes", path.display(), error_number = error_number("cli", &format!("cli.{label}_too_large")).expect("Registered file error"));
            EXIT_RESOURCE_LIMIT
        }
        EvaluationFileError::InvalidUtf8(error) => {
            eprintln!(
                "error: [{error_number}] [cli.{label}_utf8] {label} file '{}' is not UTF-8: {error}",
                path.display(),
                error_number = error_number("cli", &format!("cli.{label}_utf8")).expect("Registered file error")
            );
            EXIT_IO_ERROR
        }
        EvaluationFileError::BomNotAllowed => {
            eprintln!(
                "error: [{error_number}] [cli.{label}_bom] {label} file '{}' must not start with a UTF-8 BOM",
                path.display(), error_number = error_number("cli", &format!("cli.{label}_bom")).expect("Registered file error"));
            EXIT_IO_ERROR
        }
    }
}

fn level_api_request(
    api: &mut LevelApi,
    operation: &str,
    fields: JsonValue,
) -> Result<JsonValue, String> {
    let mut request = json!({ "api_version": LEVEL_API_VERSION, "operation": operation });
    let request_object = request
        .as_object_mut()
        .ok_or_else(|| "internal request construction did not produce a JSON object".to_owned())?;
    let fields_object = fields
        .as_object()
        .ok_or_else(|| "internal API fields did not produce a JSON object".to_owned())?;
    request_object.extend(fields_object.clone());
    let request_text = serde_json::to_string(&request).map_err(|error| error.to_string())?;
    let response_text = api.request_json(&request_text);
    serde_json::from_str(&response_text).map_err(|error| {
        format!(
            "level API returned invalid JSON ({error}): {}",
            response_text.chars().take(400).collect::<String>()
        )
    })
}

fn response_status(response: &JsonValue) -> Option<&str> {
    response.get("status").and_then(JsonValue::as_str)
}

fn boundary_api_name(boundary: BoundaryMode) -> &'static str {
    match boundary {
        BoundaryMode::Exit => "Exit",
        BoundaryMode::Wrap => "Wrap",
    }
}

fn response_handle(response: &JsonValue) -> Result<String, String> {
    response
        .get("handle")
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "successful level API operation omitted its handle".to_owned())
}

fn level_api_error_exit(response: &JsonValue) -> i32 {
    match response.pointer("/error/code").and_then(JsonValue::as_str) {
        Some("level_api.resource_limit" | "level_api.response_too_large") => EXIT_RESOURCE_LIMIT,
        Some(code) if code.contains("fault") || code.contains("invariant") => EXIT_VM_FAULT,
        Some(_) | None => EXIT_INVALID_ARGUMENTS,
    }
}

fn evaluation_result_exit(response: &JsonValue) -> i32 {
    match response
        .pointer("/result/status")
        .and_then(JsonValue::as_str)
    {
        Some("Passed") => EXIT_SUCCESS,
        Some("ProgramRejected" | "TestFailed" | "RuntimeError" | "ConstraintExceeded") => {
            EXIT_EVALUATION_FAILED
        }
        Some("ResourceLimitExceeded" | "Cancelled") => EXIT_RESOURCE_LIMIT,
        Some("Fault") => EXIT_VM_FAULT,
        Some(other) => {
            eprintln!("error: [{error_number}] [cli.unknown_evaluation_status] unrecognized level API result status '{other}'", error_number = codegrid_model::error_number("cli", "cli.unknown_evaluation_status").expect("Registered process error"));
            EXIT_VM_FAULT
        }
        None => {
            eprintln!(
                "error: [{error_number}] [cli.malformed_evaluation_result] level API result omitted its status", error_number = codegrid_model::error_number("cli", "cli.malformed_evaluation_result").expect("Registered process error"));
            EXIT_VM_FAULT
        }
    }
}

fn print_evaluation_human(response: &JsonValue) {
    let response_status = response_status(response).unwrap_or("unknown");
    let result = response.get("result").unwrap_or(response);
    let result_status = result
        .get("status")
        .and_then(JsonValue::as_str)
        .unwrap_or(response_status);
    println!("Evaluation: {result_status}");

    for (label, keys) in [
        ("Failure", &["failure", "error"][..]),
        ("Source diagnostics", &["diagnostics"][..]),
        ("Visible tests", &["visible_tests"][..]),
        ("Hidden failure category", &["hidden_failure"][..]),
        ("Constraints", &["constraint_results", "constraints"][..]),
        ("Partial metrics", &["partial_metrics"][..]),
        ("Final metrics", &["final_metrics"][..]),
        ("Scoring", &["scoring"][..]),
        ("Rating", &["rating"][..]),
    ] {
        if let Some(value) = keys
            .iter()
            .find_map(|key| result.get(*key))
            .filter(|value| !value.is_null())
        {
            if value == &JsonValue::Array(vec![]) || value == &json!({}) {
                continue;
            }
            println!("{label}:");
            match serde_json::to_string_pretty(value) {
                Ok(serialized) => println!("{serialized}"),
                Err(_) => println!("{value}"),
            }
        }
    }
}

fn configuration_json(
    options: &RunOptions,
    input: &[u8],
    initial_memory: &BTreeMap<MemoryAddress, u8>,
) -> JsonValue {
    json!({
        "boundary_mode": boundary_name(options.boundary),
        "seed": options.seed.to_string(),
        "custom_execution_limit": options.custom_limit.get().to_string(),
        "max_ticks": options.max_ticks.get().to_string(),
        "max_work_units": options.max_work_units.get().to_string(),
        "input": input,
        "initial_memory": initial_memory.iter().map(|(address, value)| json!({
            "address": address.to_string(),
            "value": value,
        })).collect::<Vec<_>>(),
    })
}

fn result_json(
    status: &str,
    yield_reason: JsonValue,
    configuration: JsonValue,
    diagnostics: &[Diagnostic],
    events: &[JsonValue],
    newly_emitted_output: &[u8],
    snapshot: Option<JsonValue>,
) -> JsonValue {
    json!({
        "schema": "codegrid.cli.run-result",
        "schema_version": 1,
        "status": status,
        "yield_reason": yield_reason,
        "configuration": configuration,
        "diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>(),
        "events": events,
        "newly_emitted_output": newly_emitted_output,
        "snapshot": snapshot,
    })
}

fn diagnostic_json(diagnostic: &Diagnostic) -> JsonValue {
    json!({
        "code": diagnostic.code,
        "error_number": error_number("source", diagnostic.code).or_else(|| error_number("ir", diagnostic.code)),
        "severity": severity_name(diagnostic.severity),
        "message": diagnostic.message,
        "span": {
            "start": diagnostic.span.start.to_string(),
            "end": diagnostic.span.end.to_string(),
        },
    })
}

fn print_run_result(result: &JsonValue) -> Result<(), serde_json::Error> {
    let serialized = serde_json::to_string_pretty(result)?;
    println!("{serialized}");
    Ok(())
}

fn read_source(path: &PathBuf) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read UTF-8 source '{}': {error}", path.display()))
}

fn print_diagnostics(path: &PathBuf, source: &str, diagnostics: &[Diagnostic]) {
    let line_index = LineIndex::new(source);
    for diagnostic in diagnostics {
        let (line, column) = diagnostic_location(source, &line_index, diagnostic.span.start);
        eprintln!(
            "{}:{}:{}: {}: [{}] [{}] {}",
            path.display(),
            line,
            column,
            severity_name(diagnostic.severity),
            error_number("source", diagnostic.code)
                .or_else(|| error_number("ir", diagnostic.code))
                .expect("Registered diagnostic"),
            diagnostic.code,
            diagnostic.message
        );
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn diagnostic_location(source: &str, line_index: &LineIndex, byte_offset: usize) -> (usize, usize) {
    let (line, byte_column) = line_index.line_and_byte_column(source, byte_offset);
    let line_start = line_index.line_start(line).unwrap_or_default();
    let column = source
        .get(line_start..line_start + byte_column)
        .map(|prefix| prefix.chars().count() + 1)
        .unwrap_or(byte_column + 1);
    (line + 1, column)
}

fn snapshot_json(snapshot: &VmSnapshot) -> JsonValue {
    json!({
        "status": vm_status_name(snapshot.status),
        "committed_ticks": snapshot.committed_ticks.to_string(),
        "registers": snapshot.registers,
        "memory": snapshot.memory.allocated_addresses().map(|address| json!({
            "address": address.to_string(),
            "value": snapshot.memory.read(address),
        })).collect::<Vec<_>>(),
        "remaining_input": snapshot.input,
        "output": snapshot.output,
        "runtime_program": scoped_program_json(&snapshot.runtime_program),
        "threads": snapshot.threads.iter().map(|thread| thread_json(thread, true)).collect::<Vec<_>>(),
        "metrics": metrics_json(&snapshot.metrics),
        "errors": snapshot.errors.iter().map(runtime_error_json).collect::<Vec<_>>(),
        "fault": snapshot.fault.map(fault_json),
    })
}

fn scoped_program_json(program: &ScopedProgram) -> JsonValue {
    json!({
        "main": board_json(&program.main),
        "functions": program.functions.iter().map(|(slot, board)| {
            (slot.get().to_string(), board_json(board))
        }).collect::<serde_json::Map<_, _>>(),
    })
}

fn board_json(board: &Board) -> JsonValue {
    json!({
        "width": board.width,
        "height": board.height,
        "cells": board.cells.iter().map(|cell| json!({
            "entry": cell.entry.map(direction_token),
            "primary": cell.primary.map(PrimaryInstruction::token),
            "attachment": cell.attachment.map(AttachmentInstruction::token),
        })).collect::<Vec<_>>(),
        "folded_blocks": board.folded_blocks.iter().map(|(slot, block)| {
            (slot.get().to_string(), json!(
                block.cells.iter().map(|cell| cell.map(PrimaryInstruction::token)).collect::<Vec<_>>()
            ))
        }).collect::<serde_json::Map<_, _>>(),
    })
}

fn thread_json(thread: &ThreadSnapshot, include_code_grid: bool) -> JsonValue {
    let mut object = serde_json::Map::new();
    if include_code_grid {
        object.insert("code_grid".to_owned(), json!({"kind": "outer"}));
    }
    object.extend([
        ("id".to_owned(), json!(thread.id.to_string())),
        ("board".to_owned(), board_id_json(thread.board)),
        ("position".to_owned(), coordinate_json(thread.position)),
        (
            "direction".to_owned(),
            json!(direction_name(thread.direction)),
        ),
        (
            "register_pointer".to_owned(),
            json!(thread.register_pointer),
        ),
        ("page".to_owned(), json!(thread.page.to_string())),
        ("data_stack".to_owned(), json!(thread.data_stack)),
        (
            "instruction_stack".to_owned(),
            json!(thread
                .instruction_stack
                .iter()
                .map(|item| instruction_stack_item_json(*item))
                .collect::<Vec<_>>()),
        ),
        (
            "call_stack".to_owned(),
            json!(thread
                .call_stack
                .iter()
                .map(|frame| json!({
                    "caller_board": board_id_json(frame.caller_board),
                    "call_position": coordinate_json(frame.call_position),
                    "saved_direction": direction_name(frame.saved_direction),
                }))
                .collect::<Vec<_>>()),
        ),
        ("phase".to_owned(), phase_json(thread.phase)),
        (
            "random_state".to_owned(),
            json!(thread.random_state.to_string()),
        ),
    ]);
    JsonValue::Object(object)
}

fn instruction_stack_item_json(item: InstructionStackItem) -> JsonValue {
    match item {
        InstructionStackItem::Empty => json!({"kind": "empty"}),
        InstructionStackItem::Primary(primary) => json!({
            "kind": "primary",
            "token": primary.primary().token(),
            "instruction_code": primary.code(),
        }),
    }
}

fn phase_json(phase: ThreadPhaseSnapshot) -> JsonValue {
    match phase {
        ThreadPhaseSnapshot::Normal => json!({"kind": "normal"}),
        ThreadPhaseSnapshot::AfterCall => json!({"kind": "after_call"}),
        ThreadPhaseSnapshot::Repeat { total, completed } => json!({
            "kind": "repeat",
            "total": total,
            "completed": completed,
        }),
        ThreadPhaseSnapshot::Fold {
            fold_id,
            internal_position,
            internal_direction,
            saved_outer_direction,
        } => json!({
            "kind": "fold",
            "fold_id": fold_id.get(),
            "internal_position": coordinate_json(internal_position),
            "internal_direction": direction_name(internal_direction),
            "saved_outer_direction": direction_name(saved_outer_direction),
        }),
        ThreadPhaseSnapshot::FoldResume {
            saved_outer_direction,
        } => json!({
            "kind": "fold_resume",
            "saved_outer_direction": direction_name(saved_outer_direction),
        }),
        ThreadPhaseSnapshot::Terminated => json!({"kind": "terminated"}),
    }
}

fn metrics_json(metrics: &RuntimeMetrics) -> JsonValue {
    json!({
        "global_tick": metrics.global_tick().to_string(),
        "operation_count": metrics.operation_count().to_string(),
        "used_cell_count": metrics.used_cell_count().to_string(),
        "used_cells": metrics.used_cells().iter().map(static_cell_json).collect::<Vec<_>>(),
        "used_memory_address_count": metrics.used_memory_address_count().to_string(),
        "used_memory_addresses": metrics.used_memory_addresses().iter().map(memory_location_json).collect::<Vec<_>>(),
        "peak_data_stack_usage": metrics.peak_data_stack_usage().to_string(),
        "peak_instruction_stack_usage": metrics.peak_instruction_stack_usage().to_string(),
        "peak_call_stack_usage": metrics.peak_call_stack_usage().to_string(),
        "instruction_variety": metrics.instruction_variety().iter().copied().map(instruction_kind_name).collect::<Vec<_>>(),
    })
}

fn static_cell_json(cell: &StaticCellId) -> JsonValue {
    json!({
        "code_grid": code_grid_id_json(cell.code_grid),
        "board": board_id_json(cell.board),
        "folded_block": cell.folded_block.map(Slot::get),
        "position": coordinate_json(cell.position),
    })
}

fn memory_location_json(location: &MemoryLocationId) -> JsonValue {
    json!({
        "space": match location.space {
            MemorySpaceId::Outer => json!({"kind": "outer"}),
            MemorySpaceId::CustomInvocation { global_tick, caller_thread_id, custom_id } => json!({
                "kind": "custom_invocation",
                "global_tick": global_tick.to_string(),
                "caller_thread_id": caller_thread_id.to_string(),
                "custom_id": custom_id.get(),
            }),
        },
        "address": location.address.to_string(),
    })
}

fn event_json(event: &VmEvent) -> JsonValue {
    match event {
        VmEvent::CellReached {
            scope,
            thread_id,
            cell,
        } => json!({
            "kind": "cell_reached",
            "scope": scope_json(*scope),
            "thread_id": thread_id.to_string(),
            "cell": static_cell_json(cell),
        }),
        VmEvent::InputConsumed {
            scope,
            thread_id,
            value,
        } => json!({
            "kind": "input_consumed",
            "scope": scope_json(*scope),
            "thread_id": thread_id.to_string(),
            "value": value,
        }),
        VmEvent::RegisterChanged {
            scope,
            register,
            old,
            new,
        } => json!({
            "kind": "register_changed",
            "scope": scope_json(*scope),
            "register": register,
            "old": old,
            "new": new,
        }),
        VmEvent::MemoryChanged {
            scope,
            location,
            old,
            new,
        } => json!({
            "kind": "memory_changed",
            "scope": scope_json(*scope),
            "location": memory_location_json(location),
            "old": old,
            "new": new,
        }),
        VmEvent::CodeChanged {
            scope,
            cell,
            old,
            new,
        } => json!({
            "kind": "code_changed",
            "scope": scope_json(*scope),
            "cell": static_cell_json(cell),
            "old": old.map(PrimaryInstruction::token),
            "new": new.map(PrimaryInstruction::token),
        }),
        VmEvent::ThreadChanged {
            scope,
            before,
            after,
        } => json!({
            "kind": "thread_changed",
            "scope": scope_json(*scope),
            "before": thread_json(before, false),
            "after": thread_json(after, false),
        }),
    }
}

fn runtime_error_json(error: &RuntimeError) -> JsonValue {
    json!({
        "global_tick": error.global_tick().to_string(),
        "scope": scope_json(error.scope()),
        "code": error.code(),
        "error_number": error_number("vm", error.code()),
        "details": runtime_error_details(error.kind()),
    })
}

fn runtime_error_details(kind: &RuntimeErrorKind) -> JsonValue {
    match kind {
        RuntimeErrorKind::ConcurrentCallerStackReadConflict {
            internal_thread_ids,
        } => json!({
            "internal_thread_ids": ids_json(internal_thread_ids),
        }),
        RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
            internal_thread_ids,
        } => json!({
            "internal_thread_ids": ids_json(internal_thread_ids),
        }),
        RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
            internal_thread_ids,
        } => json!({
            "internal_thread_ids": ids_json(internal_thread_ids),
        }),
        RuntimeErrorKind::ConcurrentCodeWriteConflict { cell, thread_ids } => json!({
            "cell": static_cell_json(cell),
            "thread_ids": ids_json(thread_ids),
        }),
        RuntimeErrorKind::ConcurrentInputConflict { thread_ids }
        | RuntimeErrorKind::ConcurrentOutputConflict { thread_ids } => json!({
            "thread_ids": ids_json(thread_ids),
        }),
        RuntimeErrorKind::ConcurrentMemoryWriteConflict {
            address,
            thread_ids,
        } => json!({
            "address": address.to_string(),
            "thread_ids": ids_json(thread_ids),
        }),
        RuntimeErrorKind::ConcurrentWriteConflict {
            register,
            thread_ids,
        } => json!({
            "register": register,
            "thread_ids": ids_json(thread_ids),
        }),
        RuntimeErrorKind::CustomExecutionLimitExceeded { limit } => json!({
            "limit": limit.to_string(),
        }),
        RuntimeErrorKind::OutOfBounds {
            thread_id,
            board,
            position,
            direction,
        } => json!({
            "thread_id": thread_id.to_string(),
            "board": board_id_json(*board),
            "position": coordinate_json(*position),
            "direction": direction_name(*direction),
        }),
        RuntimeErrorKind::ReturnWithoutCall {
            thread_id,
            board,
            position,
        } => json!({
            "thread_id": thread_id.to_string(),
            "board": board_id_json(*board),
            "position": coordinate_json(*position),
        }),
    }
}

fn ids_json(ids: &[u64]) -> Vec<String> {
    ids.iter().map(u64::to_string).collect()
}

fn scope_json(scope: ExecutionScope) -> JsonValue {
    match scope {
        ExecutionScope::Outer => json!({"kind": "outer"}),
        ExecutionScope::Custom {
            caller_thread_id,
            custom_id,
            internal_tick,
        } => json!({
            "kind": "custom",
            "caller_thread_id": caller_thread_id.to_string(),
            "custom_id": custom_id.get(),
            "internal_tick": internal_tick.to_string(),
        }),
    }
}

fn fault_json(fault: VmFault) -> JsonValue {
    match fault {
        VmFault::MetricCounterOverflow(counter) => json!({
            "kind": "metric_counter_overflow", "error_number" : codegrid_model::error_number("fault", "metric_counter_overflow"),
            "counter": metric_counter_name(counter),
        }),
        VmFault::GlobalTickOverflow => {
            json!({"kind": "global_tick_overflow", "error_number" : codegrid_model::error_number("fault", "global_tick_overflow")})
        }
        VmFault::InternalInvariantViolation => {
            json!({"kind": "internal_invariant_violation", "error_number" : codegrid_model::error_number("fault", "internal_invariant_violation")})
        }
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

fn code_grid_id_json(id: CodeGridId) -> JsonValue {
    match id {
        CodeGridId::Outer => json!({"kind": "outer"}),
        CodeGridId::Custom(slot) => json!({"kind": "custom", "id": slot.get()}),
    }
}

fn board_id_json(id: BoardId) -> JsonValue {
    match id {
        BoardId::Main => json!({"kind": "main"}),
        BoardId::Function(slot) => json!({"kind": "function", "id": slot.get()}),
    }
}

fn coordinate_json(position: codegrid_vm::Coordinate) -> JsonValue {
    json!({"x": position.x, "y": position.y})
}

fn boundary_name(boundary: BoundaryMode) -> &'static str {
    match boundary {
        BoundaryMode::Exit => "exit",
        BoundaryMode::Wrap => "wrap",
    }
}

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

fn direction_token(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "^",
        Direction::Down => "v",
        Direction::Left => "<",
        Direction::Right => ">",
    }
}

fn vm_status_name(status: VmStatus) -> &'static str {
    match status {
        VmStatus::Running => "running",
        VmStatus::Halted => "halted",
        VmStatus::Error => "error",
    }
}

fn instruction_kind_name(kind: InstructionKind) -> &'static str {
    kind.name()
}

fn runtime_error_summary(error: &RuntimeError) -> String {
    format!(
        "runtime error at Global Tick {}: [{}] [{}]",
        error.global_tick(),
        error_number("vm", error.code()).expect("Registered VM error"),
        error.code()
    )
}

fn print_usage() {
    eprintln!("{USAGE}");
}

#[cfg(test)]
mod tests {
    use super::{
        parse_arguments, parse_byte_list, parse_canonical_signed, parse_canonical_u64, Command,
    };
    use std::ffi::OsString;

    #[test]
    fn limited_reads_stop_after_the_first_excess_byte() {
        let mut input = std::io::Cursor::new(vec![b'a'; 1024]);
        assert!(matches!(
            super::read_limited_utf8(&mut input, 8),
            Err(super::EvaluationFileError::TooLarge {
                limit: 8,
                actual: 9
            })
        ));
        assert_eq!(input.position(), 9);
        assert!(matches!(super::read_limited_utf8(&b"12345678"[..], 8), Ok(s) if s == "12345678"));
        assert!(matches!(
            super::read_limited_utf8(&b"\xff"[..], 8),
            Err(super::EvaluationFileError::InvalidUtf8(_))
        ));
        assert!(matches!(
            super::read_limited_utf8("\u{feff}".as_bytes(), 8),
            Err(super::EvaluationFileError::BomNotAllowed)
        ));
    }

    #[test]
    fn accepts_only_canonical_unsigned_arguments() {
        assert_eq!(parse_canonical_u64("0"), Some(0));
        assert_eq!(parse_canonical_u64("18446744073709551615"), Some(u64::MAX));
        for value in ["", "00", "+1", "-1", " 1", "1 ", "18446744073709551616"] {
            assert_eq!(parse_canonical_u64(value), None, "{value:?}");
        }
    }

    #[test]
    fn accepts_only_canonical_arbitrary_precision_addresses() {
        assert_eq!(parse_canonical_signed("0").unwrap().to_string(), "0");
        assert_eq!(parse_canonical_signed("-123").unwrap().to_string(), "-123");
        for value in ["", "-0", "+1", "00", "-01", " 1", "1 "] {
            assert!(parse_canonical_signed(value).is_none(), "{value:?}");
        }
    }

    #[test]
    fn byte_lists_allow_an_explicit_empty_value_and_trim_items() {
        assert!(parse_byte_list("").unwrap().is_empty());
        assert_eq!(parse_byte_list(" 0, 255 ").unwrap(), [0, 255]);
        for value in [" ", "1,,2", "1,", ",1", "01", "256", "-1"] {
            assert!(parse_byte_list(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn run_requires_every_full_execution_setting() {
        let args = ["run", "program.cg"]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(parse_arguments(args).is_err());

        let args = [
            "run",
            "program.cg",
            "--boundary",
            "wrap",
            "--seed",
            "18446744073709551615",
            "--custom-limit",
            "10",
            "--max-ticks",
            "20",
            "--max-work-units",
            "30",
            "--input",
            "",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert!(matches!(parse_arguments(args), Ok(Command::Run(_))));
    }
}
