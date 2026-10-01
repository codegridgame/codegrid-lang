use crate::{profile::parse_decimal, ApiError, SafetyProfile, LEVEL_API_VERSION};
use codegrid_ir::VerifiedProgram;
use codegrid_level_core::*;
use codegrid_model::BoundaryMode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

#[derive(Deserialize)]
struct Request {
    api_version: u32,
    #[serde(flatten)]
    operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Capabilities,
    LoadLevel {
        level_json: String,
    },
    CompileProgram {
        source: String,
    },
    StartEvaluation {
        level: String,
        program: String,
        mode: String,
        boundary_mode: String,
        shuffle_seed: Option<String>,
        custom_execution_limit: String,
    },
    AdvanceEvaluation {
        evaluation: String,
        work_budget: String,
    },
    EvaluationResult {
        evaluation: String,
    },
    Release {
        kind: String,
        handle: String,
    },
    Shutdown,
}
struct Retained<T> {
    value: T,
    bytes: u64,
    provenance: Value,
}
/// One isolated host session. Handles cannot be used in another session.
pub struct LevelApi {
    profile: SafetyProfile,
    levels: BTreeMap<String, Retained<ValidatedLevel>>,
    programs: BTreeMap<String, Retained<VerifiedProgram>>,
    evaluations: BTreeMap<String, Retained<EvaluationSession>>,
    nonce: String,
    next: u64,
    retained_bytes: u64,
    stopped: bool,
    seed_source: Option<Box<dyn FnMut() -> u64>>,
}
static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
impl LevelApi {
    pub fn new(profile: SafetyProfile) -> Result<Self, ApiError> {
        profile.validate()?;
        let id = NEXT_SESSION
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |id| id.checked_add(1),
            )
            .map_err(|_| {
                ApiError::new("level_api.handle_exhausted", "Session identity exhausted")
            })?;
        Ok(Self {
            profile,
            levels: BTreeMap::new(),
            programs: BTreeMap::new(),
            evaluations: BTreeMap::new(),
            nonce: id.to_string(),
            next: 1,
            retained_bytes: 0,
            stopped: false,
            seed_source: None,
        })
    }
    pub fn with_seed_source(mut self, source: impl FnMut() -> u64 + 'static) -> Self {
        self.seed_source = Some(Box::new(source));
        self
    }
    pub fn profile(&self) -> &SafetyProfile {
        &self.profile
    }
    fn handle(&mut self, bytes: u64) -> Result<String, ApiError> {
        if (self.levels.len() + self.programs.len() + self.evaluations.len()) as u64
            >= self.profile.max_handles
            || self
                .retained_bytes
                .checked_add(bytes)
                .is_none_or(|n| n > self.profile.max_state_bytes)
        {
            return Err(ApiError::new(
                "level_api.resource_limit",
                "Retained state or handle ceiling reached",
            ));
        }
        let next = self.next;
        self.next = self.next.checked_add(1).ok_or_else(|| {
            ApiError::new("level_api.handle_exhausted", "Handle identity exhausted")
        })?;
        // Pair the instance namespace and local counter as one exact integer.
        let owner = parse_decimal(&self.nonce).unwrap();
        let value = owner
            .checked_shl(32)
            .filter(|_| owner <= u32::MAX as u64)
            .and_then(|n| n.checked_add(next))
            .filter(|_| next <= u32::MAX as u64)
            .ok_or_else(|| {
                ApiError::new("level_api.handle_exhausted", "Handle identity exhausted")
            })?;
        self.retained_bytes += bytes;
        Ok(value.to_string())
    }
    pub fn request_json(&mut self, request: &str) -> String {
        let response = if request.len() as u64
            > self
                .profile
                .max_level_bytes
                .max(self.profile.max_source_bytes)
                .saturating_mul(8)
                .saturating_add(4096)
        {
            Err(ApiError::new(
                "level_api.resource_limit",
                "Request exceeds byte ceiling",
            ))
        } else {
            parse_request(request).and_then(|r| self.request(r))
        };
        let value = match response {
            Ok(v) => v,
            Err(e) => envelope(
                "error",
                json!({"error":{"code":e.code,"error_number":codegrid_model::error_number("level",e.code),"message":e.message}}),
            ),
        };
        bounded_json(&value,self.profile.max_response_bytes).unwrap_or_else(|| envelope("error",json!({"error":{"code":"level_api.response_too_large","error_number":codegrid_model::error_number("level","level_api.response_too_large"),"message":"Complete response exceeds trusted byte ceiling"}})).to_string())
    }
    fn request(&mut self, request: Request) -> Result<Value, ApiError> {
        if request.api_version != LEVEL_API_VERSION {
            return Err(ApiError::new(
                "level_api.unsupported_version",
                "Unsupported level API version",
            ));
        }
        if self.stopped {
            return Err(ApiError::new("level_api.shutdown", "Session is shut down"));
        }
        match request.operation {
            Operation::Capabilities => Ok(envelope(
                "ok",
                json!({"capabilities":{"format_versions":[1],"api_versions":[1],"evaluation_types":["ExactIO"],"scene_types":[],"evaluator_contract":"exactio-v1/2026-09-30"}}),
            )),
            Operation::LoadLevel { level_json } => {
                if level_json.len() as u64 > self.profile.max_level_bytes {
                    return Err(ApiError::new(
                        "level_api.resource_limit",
                        "Level JSON exceeds trusted byte ceiling",
                    ));
                }
                let limits = LoadLimits {
                    max_level_bytes: self.profile.max_level_bytes as usize,
                    max_tests: self.profile.max_tests as usize,
                    max_total_test_bytes: self.profile.max_total_test_bytes as usize,
                };
                match load_level(level_json.as_bytes(), limits) {
                    Ok(level) => {
                        let bytes = level_json.len() as u64;
                        let handle = self.handle(bytes)?;
                        self.levels.insert(
                            handle.clone(),
                            Retained {
                                value: level,
                                bytes,
                                provenance: json!(sha256(level_json.as_bytes())),
                            },
                        );
                        Ok(envelope("ok", json!({"handle":handle})))
                    }
                    Err(e)
                        if matches!(
                            e.reason,
                            "InputSizeExceeded" | "TestCountExceeded" | "TestDataSizeExceeded"
                        ) =>
                    {
                        Err(ApiError::new(
                            "level_api.resource_limit",
                            format!("Trusted level loading ceiling reached: {}", e.reason),
                        ))
                    }
                    Err(e) => {
                        if (e.path.len() as u64)
                            .checked_mul(6)
                            .and_then(|n| n.checked_add(512))
                            .is_none_or(|n| n > self.profile.max_response_bytes)
                        {
                            return Err(response_too_large());
                        }
                        Ok(envelope(
                            "level_rejected",
                            json!({"error":{"code":level_code(e.category),"error_number":codegrid_model::error_number("level",level_code(e.category)),"category":e.category,"reason":e.reason,"path":e.path}}),
                        ))
                    }
                }
            }
            Operation::CompileProgram { source } => {
                if source.len() as u64 > self.profile.max_source_bytes {
                    return Err(ApiError::new(
                        "level_api.resource_limit",
                        "Source exceeds byte ceiling",
                    ));
                }
                match codegrid_compiler::compile(&source) {
                    Ok(program) => {
                        let (cells, boards) = program_size(&program);
                        if cells > self.profile.max_program_cells
                            || boards > self.profile.max_program_boards
                        {
                            return Err(ApiError::new(
                                "level_api.resource_limit",
                                "Program structure exceeds trusted ceiling",
                            ));
                        }
                        let bytes = source.len() as u64;
                        let handle = self.handle(bytes)?;
                        self.programs.insert(
                            handle.clone(),
                            Retained {
                                value: program,
                                bytes,
                                provenance: json!(sha256(source.as_bytes())),
                            },
                        );
                        Ok(envelope("ok", json!({"handle":handle})))
                    }
                    Err(diagnostics) => {
                        let bound = diagnostics.iter().try_fold(256u64, |n, d| {
                            n.checked_add(256)?.checked_add(
                                (d.code.len() as u64)
                                    .checked_add(d.message.len() as u64)?
                                    .checked_mul(6)?,
                            )
                        });
                        if bound.is_none_or(|n| n > self.profile.max_response_bytes) {
                            return Err(response_too_large());
                        }
                        Ok(envelope(
                            "source_rejected",
                            json!({"diagnostics":diagnostics.into_iter().map(|d|json!({"code":d.code,"error_number":codegrid_model::error_number("source",d.code).or_else(|| codegrid_model::error_number("ir",d.code)),"message":d.message,"severity":format!("{:?}",d.severity),"span":{"start":d.span.start.to_string(),"end":d.span.end.to_string()}})).collect::<Vec<_>>()}),
                        ))
                    }
                }
            }
            Operation::StartEvaluation {
                level,
                program,
                mode,
                boundary_mode,
                shuffle_seed,
                custom_execution_limit,
            } => {
                let level_record = self.levels.get(&level).ok_or_else(invalid_handle)?;
                let program_record = self.programs.get(&program).ok_or_else(invalid_handle)?;
                let provenance = json!({"level_sha256":level_record.provenance,"source_sha256":program_record.provenance,"profile_sha256":sha256(&serde_json::to_vec(&self.profile).expect("Finite validated profile"))});
                let input_bytes = level_record
                    .bytes
                    .checked_add(program_record.bytes)
                    .ok_or_else(|| {
                        ApiError::new(
                            "level_api.resource_limit",
                            "Retained representation accounting overflow",
                        )
                    })?;
                let level = level_record.value.clone();
                let program = program_record.value.clone();
                let base = result_base_bound(level.level_id(), &self.profile.profile_id)
                    .ok_or_else(|| {
                        ApiError::new(
                            "level_api.resource_limit",
                            "Result representation accounting overflow",
                        )
                    })?;
                let available = self
                    .profile
                    .max_state_bytes
                    .checked_sub(self.retained_bytes)
                    .and_then(|n| n.checked_sub(input_bytes))
                    .ok_or_else(|| {
                        ApiError::new("level_api.resource_limit", "Retained state ceiling reached")
                    })?;
                let feedback_budget = available.min(self.profile.max_response_bytes.max(base));
                if feedback_budget < base {
                    return Err(ApiError::new(
                        "level_api.resource_limit",
                        "Result reservation exceeds retained state ceiling",
                    ));
                }
                let bytes = input_bytes.checked_add(feedback_budget).ok_or_else(|| {
                    ApiError::new(
                        "level_api.resource_limit",
                        "Retained state accounting overflow",
                    )
                })?;
                let mode = match mode.as_str() {
                    "Debug" => EvaluationMode::Debug,
                    "Official" => EvaluationMode::Official,
                    _ => return Err(invalid_config()),
                };
                let boundary_mode = match boundary_mode.as_str() {
                    "Exit" => BoundaryMode::Exit,
                    "Wrap" => BoundaryMode::Wrap,
                    _ => return Err(invalid_config()),
                };
                let custom_execution_limit = positive(&custom_execution_limit)?;
                let shuffle_seed = match shuffle_seed {
                    Some(s) => parse_decimal(&s).ok_or_else(invalid_config)?,
                    None => self.seed_source.as_mut().ok_or_else(|| {
                        ApiError::new(
                            "level_api.seed_required",
                            "Omitted seed requires an explicit host seed source",
                        )
                    })?(),
                };
                let config = EvaluationConfig {
                    boundary_mode,
                    shuffle_seed,
                    custom_execution_limit,
                    safety: ExecutionSafetyProfile {
                        id: self.profile.profile_id.clone(),
                        version: self.profile.profile_version,
                        max_output_bytes: nz(self.profile.max_output_bytes),
                        max_state_units: nz(self.profile.max_state_units),
                        max_feedback_bytes: nz(feedback_budget),
                        per_test_ticks: nz(self.profile.max_ticks_per_test),
                        cumulative_work: nz(self.profile.max_total_work),
                        per_call_work: nz(self.profile.max_work_per_call),
                    },
                };
                let handle = self.handle(bytes)?;
                self.evaluations.insert(
                    handle.clone(),
                    Retained {
                        value: start_evaluation(level, program, mode, config),
                        bytes,
                        provenance,
                    },
                );
                Ok(envelope("ok", json!({"handle":handle})))
            }
            Operation::AdvanceEvaluation {
                evaluation,
                work_budget,
            } => {
                let budget = positive(&work_budget)?;
                if budget.get() > self.profile.max_work_per_call {
                    return Err(ApiError::new(
                        "level_api.resource_limit",
                        "Work budget exceeds trusted per-call ceiling",
                    ));
                }
                let retained = self
                    .evaluations
                    .get_mut(&evaluation)
                    .ok_or_else(invalid_handle)?;
                let provenance = retained.provenance.clone();
                let session = &mut retained.value;
                Ok(match session.advance(budget) {
                    EvaluationProgress::Pending => {
                        envelope("pending", json!({"evaluation":evaluation}))
                    }
                    EvaluationProgress::Complete(result) => {
                        project_result(&result, &provenance, self.profile.max_response_bytes)?
                    }
                })
            }
            Operation::EvaluationResult { evaluation } => {
                let retained = self
                    .evaluations
                    .get(&evaluation)
                    .ok_or_else(invalid_handle)?;
                Ok(match retained.value.result() {
                    Some(r) => {
                        project_result(r, &retained.provenance, self.profile.max_response_bytes)?
                    }
                    None => envelope("pending", json!({"evaluation":evaluation})),
                })
            }
            Operation::Release { kind, handle } => {
                let bytes = match kind.as_str() {
                    "level" => self.levels.remove(&handle).map(|x| x.bytes),
                    "program" => self.programs.remove(&handle).map(|x| x.bytes),
                    "evaluation" => self.evaluations.remove(&handle).map(|mut x| {
                        x.value.cancel();
                        x.bytes
                    }),
                    _ => None,
                }
                .ok_or_else(invalid_handle)?;
                self.retained_bytes -= bytes;
                Ok(envelope("ok", json!({})))
            }
            Operation::Shutdown => {
                self.levels.clear();
                self.programs.clear();
                self.evaluations.clear();
                self.retained_bytes = 0;
                self.stopped = true;
                Ok(envelope("ok", json!({})))
            }
        }
    }
}
fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).expect("Validated positive profile")
}
fn parse_request(text: &str) -> Result<Request, ApiError> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = serde_json::Map<String, Value>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a request object with unique fields")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry::<String, Value>()? {
                if values.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("Duplicate request field"));
                }
            }
            Ok(values)
        }
    }
    use serde::Deserializer;
    let mut d = serde_json::Deserializer::from_str(text);
    let values = d
        .deserialize_map(Unique)
        .map_err(|e| ApiError::new("level_api.invalid_request", e.to_string()))?;
    d.end()
        .map_err(|e| ApiError::new("level_api.invalid_request", e.to_string()))?;
    let version = values
        .get("api_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::new(
                "level_api.invalid_request",
                "API version must be an integer",
            )
        })?;
    if version != u64::from(LEVEL_API_VERSION) {
        return Err(ApiError::new(
            "level_api.unsupported_version",
            "Unsupported level API version",
        ));
    }
    let fields: &[&str] = match values.get("operation").and_then(Value::as_str) {
        Some("capabilities" | "shutdown") => &[],
        Some("load_level") => &["level_json"],
        Some("compile_program") => &["source"],
        Some("start_evaluation") => &[
            "level",
            "program",
            "mode",
            "boundary_mode",
            "shuffle_seed",
            "custom_execution_limit",
        ],
        Some("advance_evaluation") => &["evaluation", "work_budget"],
        Some("evaluation_result") => &["evaluation"],
        Some("release") => &["kind", "handle"],
        _ => {
            return Err(ApiError::new(
                "level_api.invalid_request",
                "Unsupported operation",
            ))
        }
    };
    if values
        .keys()
        .any(|k| k != "api_version" && k != "operation" && !fields.contains(&k.as_str()))
    {
        return Err(ApiError::new(
            "level_api.invalid_request",
            "Unknown request field",
        ));
    }
    serde_json::from_value(Value::Object(values))
        .map_err(|e| ApiError::new("level_api.invalid_request", e.to_string()))
}
fn positive(s: &str) -> Result<NonZeroU64, ApiError> {
    parse_decimal(s)
        .and_then(NonZeroU64::new)
        .ok_or_else(invalid_config)
}
fn invalid_handle() -> ApiError {
    ApiError::new(
        "level_api.invalid_handle",
        "Handle is stale, has another kind, or belongs to another session",
    )
}
fn invalid_config() -> ApiError {
    ApiError::new(
        "level_api.invalid_configuration",
        "Expected explicit supported configuration and canonical exact integer",
    )
}
fn level_code(category: &str) -> &str {
    match category {
        "UnsupportedFormatVersion" => "level.unsupported_format_version",
        "UnsupportedSceneType" => "level.unsupported_scene_type",
        _ => "level.invalid",
    }
}
fn envelope(status: &str, fields: Value) -> Value {
    let mut v = json!({"schema":"codegrid.level.response","api_version":1,"status":status});
    v.as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    v
}
fn program_size(program: &VerifiedProgram) -> (u64, u64) {
    let mut cells = 0;
    let mut boards = 0;
    for scope in std::iter::once(&program.program().outer)
        .chain(program.program().customs.values().map(|c| &c.program))
    {
        for board in std::iter::once(&scope.main).chain(scope.functions.values()) {
            boards += 1;
            cells += board.cells.len() as u64;
            for folded in board.folded_blocks.values() {
                boards += 1;
                cells += folded.cells.len() as u64;
            }
        }
    }
    (cells, boards)
}
fn metrics_json(metrics: &BTreeMap<String, u64>) -> Value {
    serde_json::to_value(
        metrics
            .iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}
fn outcome_json(outcome: &TestOutcome) -> Value {
    match outcome {
        TestOutcome::Passed => json!({"status":"Passed"}),
        TestOutcome::WrongOutput => {
            json!({"status":"WrongOutput","code":"level.wrong_output","error_number":codegrid_model::error_number("level","level.wrong_output")})
        }
        TestOutcome::IncompleteOutput => {
            json!({"status":"IncompleteOutput","code":"level.incomplete_output","error_number":codegrid_model::error_number("level","level.incomplete_output")})
        }
        TestOutcome::RuntimeError(codes) => {
            json!({"status":"RuntimeError","code":"level.runtime_error","error_number":codegrid_model::error_number("level","level.runtime_error"),"codes":codes,"error_numbers":codes.iter().map(|c|codegrid_model::error_number("vm",c)).collect::<Vec<_>>()})
        }
    }
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn result_json(r: &EvaluationResult, provenance: &Value) -> Value {
    let result_code = match &r.status {
        EvaluationStatus::Passed => None,
        EvaluationStatus::ProgramRejected(_) => Some("level.program_rejected"),
        EvaluationStatus::TestFailed => Some("level.test_failed"),
        EvaluationStatus::RuntimeError => Some("level.runtime_error"),
        EvaluationStatus::ConstraintExceeded => Some("level.constraint_exceeded"),
        EvaluationStatus::ResourceLimitExceeded => Some("level.resource_limit"),
        EvaluationStatus::Cancelled => Some("level.cancelled"),
        EvaluationStatus::Fault(reason) => Some(reason.code()),
    };

    let (status, rejection) = match &r.status {
        EvaluationStatus::Passed => ("Passed", Value::Null),
        EvaluationStatus::ProgramRejected(p) => (
            "ProgramRejected",
            json!({"code":"level.program_rejected","error_number":codegrid_model::error_number("level","level.program_rejected"),"reason":p.reason,"path":p.path}),
        ),
        EvaluationStatus::TestFailed => ("TestFailed", Value::Null),
        EvaluationStatus::RuntimeError => ("RuntimeError", Value::Null),
        EvaluationStatus::ConstraintExceeded => ("ConstraintExceeded", Value::Null),
        EvaluationStatus::ResourceLimitExceeded => (
            "ResourceLimitExceeded",
            json!({"code":"level.resource_limit","error_number":codegrid_model::error_number("level","level.resource_limit")}),
        ),
        EvaluationStatus::Cancelled => (
            "Cancelled",
            json!({"code":"level.cancelled","error_number":codegrid_model::error_number("level","level.cancelled")}),
        ),
        EvaluationStatus::Fault(reason) => (
            "Fault",
            json!({"code":reason.code(),"error_number":codegrid_model::error_number("level",reason.code()),"reason":format!("{reason:?}")}),
        ),
    };
    json!({"status":status,"error_number":result_code.and_then(|c|codegrid_model::error_number("level",c)),"replay":provenance,"failure":rejection,"level_id":r.level_id,"level_version":r.level_version,"evaluator_contract":r.evaluator_contract,"evaluator_build":env!("CODEGRID_LEVEL_BUILD_ID"),"mode":format!("{:?}",r.mode),"configuration":{"boundary_mode":format!("{:?}",r.config.boundary_mode),"shuffle_seed":r.config.shuffle_seed.to_string(),"vm_seed":r.vm_seed.to_string(),"custom_execution_limit":r.config.custom_execution_limit.get().to_string(),"profile_id":r.config.safety.id,"profile_version":r.config.safety.version,"safety":{"max_output_bytes":r.config.safety.max_output_bytes.get().to_string(),"max_state_units":r.config.safety.max_state_units.get().to_string(),"max_feedback_bytes":r.config.safety.max_feedback_bytes.get().to_string(),"max_ticks_per_test":r.config.safety.per_test_ticks.get().to_string(),"max_work_per_call":r.config.safety.per_call_work.get().to_string(),"max_total_work":r.config.safety.cumulative_work.get().to_string()}},"hidden_failure":r.hidden_failure.as_ref().map(|o|json!({"category":"HiddenTestFailed","reason":outcome_json(o)})),"visible_tests":r.visible_tests.iter().map(|t|json!({"source_index":t.source_index.to_string(),"input":t.input,"expected_output":t.expected_output,"actual_output":t.actual_output,"outcome":outcome_json(&t.outcome)})).collect::<Vec<_>>(),"constraints":r.constraints.iter().map(|c|json!({"name":c.name,"limit":c.limit.to_string(),"value":c.value.to_string(),"passed":c.passed})).collect::<Vec<_>>(),"scoring":r.scoring.iter().map(|s|json!({"name":s.name,"target":s.target.map(|t|t.to_string()),"value":s.value.to_string(),"direction":"minimize","rating":s.rating})).collect::<Vec<_>>(),"partial_metrics":metrics_json(&r.partial_metrics),"final_metrics":r.final_metrics.as_ref().map(metrics_json),"rating":r.rating})
}

fn response_too_large() -> ApiError {
    ApiError::new(
        "level_api.response_too_large",
        "Complete response exceeds trusted byte ceiling",
    )
}
fn project_result(
    result: &EvaluationResult,
    provenance: &Value,
    limit: u64,
) -> Result<Value, ApiError> {
    if result_representation_bound(result).is_none_or(|n| n > limit) {
        return Err(response_too_large());
    }
    Ok(envelope(
        "result",
        json!({"result":result_json(result,provenance)}),
    ))
}
/// Never allocate a serialized response buffer beyond the trusted ceiling.
fn bounded_json(value: &Value, limit: u64) -> Option<String> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: u64,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if (self.bytes.len() as u64)
                .checked_add(bytes.len() as u64)
                .is_none_or(|n| n > self.limit)
            {
                return Err(std::io::Error::other("Response ceiling reached"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).ok()?;
    String::from_utf8(writer.bytes).ok()
}
