use crate::{profile::parse_decimal, ApiError};
use codegrid_ir::VerifiedProgram;
use codegrid_level_core::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

#[derive(Deserialize)]
pub(crate) struct Request {
    #[serde(rename = "api_version")]
    pub(crate) _api_version: u32,
    #[serde(flatten)]
    pub(crate) operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Operation {
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
    SceneFeedback {
        evaluation_handle: String,
        after_sequence: String,
        max_events: String,
    },
}
pub(crate) static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub(crate) fn parse_versioned_request(
    text: &str,
    expected_version: u32,
) -> Result<Request, ApiError> {
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
    if version != u64::from(expected_version) {
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
            "shuffle_seed",
            "custom_execution_limit",
        ],
        Some("advance_evaluation") => &["evaluation", "work_budget"],
        Some("evaluation_result") => &["evaluation"],
        Some("release") => &["kind", "handle"],
        Some("scene_feedback") => &["evaluation_handle", "after_sequence", "max_events"],
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
pub(crate) fn positive(s: &str) -> Result<NonZeroU64, ApiError> {
    parse_decimal(s)
        .and_then(NonZeroU64::new)
        .ok_or_else(invalid_config)
}
pub(crate) fn invalid_handle() -> ApiError {
    ApiError::new(
        "level_api.invalid_handle",
        "Handle is stale, has another kind, or belongs to another session",
    )
}
pub(crate) fn invalid_config() -> ApiError {
    ApiError::new(
        "level_api.invalid_configuration",
        "Expected explicit supported configuration and canonical exact integer",
    )
}
pub(crate) fn level_code(category: &str) -> &str {
    match category {
        "UnsupportedFormatVersion" => "level.unsupported_format_version",
        "UnsupportedSceneType" => "level.unsupported_scene_type",
        _ => "level.invalid",
    }
}
pub(crate) fn program_size(program: &VerifiedProgram) -> (u64, u64) {
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
pub(crate) fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) struct CommonResult<'a> {
    pub level_id: &'a str,
    pub level_version: u32,
    pub evaluator_contract: &'a str,
    pub gas_schedule_version: u32,
    pub mode: EvaluationMode,
    pub config: &'a EvaluationConfig,
    pub vm_seed: u64,
    pub status: &'a EvaluationStatus,
    pub partial_metrics: &'a Metrics,
    pub final_metrics: Option<&'a Metrics>,
    pub rating: Option<u8>,
    pub constraints: &'a [ConstraintResult],
    pub scoring: &'a [ScoringResult],
}
pub(crate) fn common_result_json(r: CommonResult<'_>, provenance: &Value) -> Value {
    let result_code = match r.status {
        EvaluationStatus::Passed => None,
        EvaluationStatus::ProgramRejected(_) => Some("level.program_rejected"),
        EvaluationStatus::TestFailed => Some("level.test_failed"),
        EvaluationStatus::RuntimeError => Some("level.runtime_error"),
        EvaluationStatus::ConstraintExceeded => Some("level.constraint_exceeded"),
        EvaluationStatus::ResourceLimitExceeded => Some("level.resource_limit"),
        EvaluationStatus::Cancelled => Some("level.cancelled"),
        EvaluationStatus::Fault(reason) => Some(reason.code()),
    };

    let (status, rejection) = match r.status {
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
    json!({"status":status,"error_number":result_code.and_then(|c|codegrid_model::error_number("level",c)),"replay":provenance,"failure":rejection,"level_id":r.level_id,"level_version":r.level_version,"evaluator_contract":r.evaluator_contract,"gas_schedule_version":r.gas_schedule_version,"evaluator_build":env!("CODEGRID_LEVEL_BUILD_ID"),"mode":format!("{:?}",r.mode),"configuration":{"shuffle_seed":r.config.shuffle_seed.to_string(),"vm_seed":r.vm_seed.to_string(),"custom_execution_limit":r.config.custom_execution_limit.get().to_string(),"profile_id":r.config.safety.id,"profile_version":r.config.safety.version,"safety":{"max_output_bytes":r.config.safety.max_output_bytes.get().to_string(),"max_state_units":r.config.safety.max_state_units.get().to_string(),"max_feedback_bytes":r.config.safety.max_feedback_bytes.get().to_string(),"max_ticks_per_test":r.config.safety.per_test_ticks.get().to_string(),"max_gas_per_test":r.config.safety.per_test_gas.get().to_string(),"max_work_per_call":r.config.safety.per_call_work.get().to_string(),"max_total_work":r.config.safety.cumulative_work.get().to_string()}},"constraints":r.constraints.iter().map(|c|json!({"name":c.name,"limit":c.limit.to_string(),"value":c.value.to_string(),"passed":c.passed})).collect::<Vec<_>>(),"scoring":r.scoring.iter().map(|s|json!({"name":s.name,"target":s.target.map(|t|t.to_string()),"value":s.value.to_string(),"direction":"minimize","rating":s.rating})).collect::<Vec<_>>(),"partial_metrics":metrics_json(r.partial_metrics),"final_metrics":r.final_metrics.map(metrics_json),"rating":r.rating})
}
pub(crate) fn response_too_large() -> ApiError {
    ApiError::new(
        "level_api.response_too_large",
        "Complete response exceeds trusted byte ceiling",
    )
}
/// Never allocate a serialized response buffer beyond the trusted ceiling.
pub(crate) fn bounded_json(value: &Value, limit: u64) -> Option<String> {
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
