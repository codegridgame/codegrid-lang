//! API-2 wire projection of the core's privacy-safe scene result.
use crate::{
    session::{bounded_json, common_result_json, response_too_large, CommonResult},
    ApiError,
};
use codegrid_level_core::{
    scene_evaluate::*,
    scene_feedback::{visible_failure_value, visible_outcome_name},
    scenes::SceneKind,
    EvaluationStatus,
};
use serde_json::{json, Value};

/// Produce a complete bounded API-2 result; never truncate a visible case.
/// This conversion does not register an evaluator or execute a VM.
pub fn project_scene_result(
    result: &SceneEvaluationResult,
    provenance: &Value,
    limit: u64,
) -> Result<String, ApiError> {
    if representation_bound(result, provenance, limit).is_none_or(|n| n > limit) {
        return Err(response_too_large());
    }
    let r = result;
    let mut value = common_result_json(
        CommonResult {
            level_id: &r.level_id,
            level_version: r.level_version,
            evaluator_contract: &r.evaluator_contract,
            mode: r.mode,
            config: &r.config,
            vm_seed: r.vm_seed,
            status: &r.status,
            partial_metrics: &r.partial_metrics,
            final_metrics: r.final_metrics.as_ref(),
            rating: r.rating,
            constraints: &r.constraints,
            scoring: &r.scoring,
        },
        provenance,
    );
    value["level_format_version"] = json!(r.level_format_version);
    value["scene_type"] = json!(format!("{:?}", r.scene_type));
    value["scene_protocol_version"] = json!(r.scene_protocol_version);
    value["configuration"]["scene_limits"] = json!({
        "max_input_queue_bytes": r.scene_limits.max_input_queue_bytes.get().to_string(),
        "max_total_input_bytes_per_test": r.scene_limits.max_total_input_bytes_per_test.get().to_string(),
        "max_scene_state_units": r.scene_limits.max_scene_state_units.get().to_string(),
        "max_scene_frames_per_test": r.scene_limits.max_scene_frames_per_test.get().to_string(),
        "max_scene_work_per_call": r.scene_limits.max_scene_work_per_call.get().to_string(),
        "max_total_scene_work": r.scene_limits.max_total_scene_work.get().to_string(),
        "max_scene_events_per_evaluation": r.scene_limits.max_scene_events_per_evaluation.get().to_string(),
        "max_scene_feedback_bytes": r.scene_limits.max_scene_feedback_bytes.get().to_string()
    });
    value["hidden_failure"] = r
        .hidden_failure
        .map(|failure| json!({"category":"HiddenTestFailed", "reason":format!("{failure:?}")}))
        .unwrap_or(Value::Null);
    value["visible_cases"] = json!(r.visible_cases.iter().map(|case| {
        let static_scene = matches!(case.scene_type, SceneKind::ExactIO | SceneKind::Baudot | SceneKind::QualityControl);
        let comparison = match &case.comparison {
            Some(SceneComparison::Static { input, expected_output, actual_output }) => json!({"input":input, "expected_output":expected_output, "actual_output":actual_output}),
            Some(SceneComparison::MechanicalArm { expected_output, actual_output }) => json!({"expected_output":expected_output,"actual_output":actual_output}),
            None => Value::Null,
        };
        let summary = case.summary.iter().map(|(key, value)| ((*key).to_owned(), json!(value.to_string()))).collect::<serde_json::Map<_,_>>();
        json!({"source_index":case.source_index.to_string(), "scene_type":format!("{:?}",case.scene_type), "outcome":visible_outcome_name(case.outcome), "failure":case.failure.as_ref().map(|failure| visible_failure_value(failure, static_scene)), "comparison":comparison, "summary":summary})
    }).collect::<Vec<_>>());
    let response = json!({"schema":"codegrid.level.response","api_version":2,"status":"result","result":value});
    bounded_json(&response, limit).ok_or_else(response_too_large)
}

fn representation_bound(r: &SceneEvaluationResult, provenance: &Value, limit: u64) -> Option<u64> {
    let mut total = 16_384u64.checked_add(bounded_json(provenance, limit)?.len() as u64)?;
    fn text(total: &mut u64, value: &str, overhead: u64) -> Option<()> {
        *total = total
            .checked_add((value.len() as u64).checked_mul(6)?)?
            .checked_add(overhead)?;
        Some(())
    }
    for value in [&r.level_id, &r.evaluator_contract, &r.config.safety.id] {
        text(&mut total, value, 128)?;
    }
    if let EvaluationStatus::ProgramRejected(rejection) = &r.status {
        text(&mut total, rejection.reason, 128)?;
        text(&mut total, &rejection.path, 128)?;
    }
    for metric in std::iter::once(&r.partial_metrics).chain(r.final_metrics.iter()) {
        for key in metric.keys() {
            text(&mut total, key, 128)?;
        }
    }
    for constraint in &r.constraints {
        text(&mut total, &constraint.name, 256)?;
    }
    for score in &r.scoring {
        text(&mut total, &score.name, 256)?;
    }
    for case in &r.visible_cases {
        total = total.checked_add(2048)?;
        for key in case.summary.keys() {
            text(&mut total, key, 128)?;
        }
        if let Some(VisibleSceneFailure::RuntimeError { codes }) = &case.failure {
            for code in codes {
                text(&mut total, code, 128)?;
            }
        }
        let bytes = match &case.comparison {
            Some(SceneComparison::Static {
                input,
                expected_output,
                actual_output,
            }) => (input.len() as u64)
                .checked_add(expected_output.len() as u64)?
                .checked_add(actual_output.len() as u64)?,
            Some(SceneComparison::MechanicalArm {
                expected_output,
                actual_output,
            }) => (expected_output.len() as u64).checked_add(actual_output.len() as u64)?,
            None => 0,
        };
        total = total.checked_add(bytes.checked_mul(4)?)?;
    }
    Some(total)
}
