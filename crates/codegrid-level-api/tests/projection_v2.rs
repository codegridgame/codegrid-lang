use codegrid_level_api::{project_scene_result, SafetyProfileV2};
use codegrid_level_core::{scene_evaluate::*, *};
use serde_json::{json, Value};
use std::num::NonZeroU64;

fn n(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}
fn author(name: &str) -> Value {
    let text = match name {
        "exact" => include_str!("../../../examples/scene-level-v2/exactio.json"),
        "baudot" => include_str!("../../../examples/scene-level-v2/baudot.json"),
        "quality" => include_str!("../../../examples/scene-level-v2/quality-control.json"),
        "elevator" => include_str!("../../../examples/scene-level-v2/elevator.json"),
        "robot" => include_str!("../../../examples/scene-level-v2/robot.json"),
        "arm" => include_str!("../../../examples/scene-level-v2/mechanical-arm.json"),
        _ => panic!("fixture"),
    };
    let mut value: Value = serde_json::from_str(text).unwrap();
    value["program_rules"]["allowed_instructions"] = json!(instruction_identifiers());
    value["program_rules"]["allowed_attachments"] = json!(attachment_identifiers());
    value
}
fn run(value: Value, mode: EvaluationMode) -> SceneEvaluationResult {
    let profile = SafetyProfileV2::from_json(include_str!(
        "../../../examples/scene-host-v2/profile-local-v2.json"
    ))
    .unwrap();
    let level = load_scene_level_json(
        value.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 1_000_000,
            max_tests: 100,
            max_total_test_bytes: 1_000_000,
        },
    )
    .unwrap();
    let program =
        codegrid_compiler::compile(include_str!("../../../fixtures/levels/echo.cg")).unwrap();
    let config = EvaluationConfig {
        boundary_mode: codegrid_model::BoundaryMode::Exit,
        shuffle_seed: 42,
        custom_execution_limit: n(100),
        safety: ExecutionSafetyProfile {
            id: profile.profile_id.clone(),
            version: 2,
            max_output_bytes: n(profile.max_output_bytes),
            max_state_units: n(profile.max_state_units),
            max_feedback_bytes: n(profile.max_response_bytes),
            per_test_ticks: n(profile.max_ticks_per_test),
            per_call_work: n(profile.max_work_per_call),
            cumulative_work: n(profile.max_total_work),
        },
    };
    let mut session = start_scene_evaluation(
        level,
        program,
        mode,
        config,
        profile.scene_limits.to_core().unwrap(),
    );
    for _ in 0..1000 {
        if let SceneEvaluationProgress::Complete(result) = session.advance(n(1000), n(100_000)) {
            return result;
        }
    }
    panic!("bounded fixture did not terminate")
}
fn project(result: &SceneEvaluationResult) -> Value {
    serde_json::from_str(
        &project_scene_result(result, &json!({"level_sha256":"test"}), 4_000_000).unwrap(),
    )
    .unwrap()
}
#[test]
fn compiled_scene_results_use_exact_tagged_shapes() {
    for name in ["exact", "baudot", "quality", "elevator", "robot", "arm"] {
        let result = run(author(name), EvaluationMode::Debug);
        assert_eq!(result.visible_cases.len(), 1, "{name}: {result:?}");
        let wire = project(&result);
        assert_eq!(wire["api_version"], 2);
        assert_eq!(wire["schema"], "codegrid.level.response");
        let body = &wire["result"];
        assert!(body.get("visible_tests").is_none());
        assert_eq!(body["level_format_version"], 1);
        assert_eq!(body["scene_protocol_version"], 1);
        assert_eq!(body["configuration"]["profile_version"], 2);
        let case = &body["visible_cases"][0];
        assert_eq!(case.as_object().unwrap().len(), 6);
        assert_eq!(case["source_index"], "0");
        let comparison = &case["comparison"];
        match name {
            "elevator" | "robot" => assert!(comparison.is_null()),
            "arm" => {
                assert_eq!(comparison.as_object().unwrap().len(), 2);
                assert!(comparison.get("input").is_none());
                assert!(!wire.to_string().contains("true_inspection"));
            }
            _ => assert_eq!(comparison.as_object().unwrap().len(), 3),
        }
        for counter in case["summary"].as_object().unwrap().values() {
            assert!(counter.is_string());
        }
        if !case["failure"].is_null() {
            assert_eq!(case["failure"].as_object().unwrap().len(), 4);
            assert!(case["failure"]["error_number"].is_string());
        }
    }
}
#[test]
fn hidden_mismatch_has_only_coarse_failure_and_no_comparison() {
    let mut value = author("exact");
    value["evaluation"]["tests"] = json!([
        {"visible":true,"input":[9],"expected_output":[9]},
        {"visible":false,"input":[211],"expected_output":[212]}
    ]);
    let result = run(value, EvaluationMode::Official);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    let wire = project(&result);
    assert_eq!(
        wire["result"]["hidden_failure"],
        json!({"category":"HiddenTestFailed","reason":"SceneFailure"})
    );
    assert!(wire["result"]["visible_cases"]
        .as_array()
        .unwrap()
        .iter()
        .all(|case| case["source_index"] == "0"));
    let text = wire.to_string();
    assert!(!text.contains("211"));
    assert!(!text.contains("212"));
}
#[test]
fn wide_counters_and_seeds_remain_decimal_strings() {
    let mut result = run(author("exact"), EvaluationMode::Debug);
    result.config.shuffle_seed = u64::MAX;
    result.vm_seed = u64::MAX;
    result.partial_metrics.insert("ticks".into(), u64::MAX);
    let wire = project(&result);
    assert_eq!(
        wire["result"]["partial_metrics"]["ticks"],
        u64::MAX.to_string()
    );
    assert_eq!(
        wire["result"]["configuration"]["shuffle_seed"],
        u64::MAX.to_string()
    );
    assert_eq!(
        wire["result"]["configuration"]["vm_seed"],
        u64::MAX.to_string()
    );
}
#[test]
fn insufficient_response_capacity_returns_error_without_partial_success() {
    let result = run(author("exact"), EvaluationMode::Debug);
    assert_eq!(
        project_scene_result(&result, &json!({}), 512)
            .unwrap_err()
            .code,
        "level_api.response_too_large"
    );
    assert_eq!(project(&result)["status"], "result");
}
