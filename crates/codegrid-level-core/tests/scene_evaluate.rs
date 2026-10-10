mod support;
use codegrid_level_core::{scene_evaluate::*, *};
use serde_json::json;
use support::*;
fn run_eval(mut s: SceneEvaluationSession, slice: u64) -> SceneEvaluationResult {
    for _ in 0..2000 {
        if let SceneEvaluationProgress::Complete(result) = s.advance(n(slice), n(100_000)) {
            return result;
        }
    }
    panic!("bounded evaluation did not terminate")
}
#[test]
fn visible_metrics_and_comparison_ignore_hidden_cases_and_match_slices() {
    let mut v = author("exact");
    v["evaluation"]["tests"] = json!([
        {"visible":true,"input":[9],"expected_output":[9]},
        {"visible":true,"input":[2],"expected_output":[2]},
        {"visible":false,"input":[211],"expected_output":[211]}]);
    v["scoring"]["metrics"] = json!({"ticks":{"target":6}});
    let p = program(&[",", ".", ";"]);
    let large = run_eval(
        start_scene_evaluation(
            load(&v),
            p.clone(),
            EvaluationMode::Official,
            config(),
            limits(),
        ),
        1000,
    );
    let small = run_eval(
        start_scene_evaluation(load(&v), p, EvaluationMode::Official, config(), limits()),
        1,
    );
    assert_eq!(large, small);
    assert_eq!(large.status, EvaluationStatus::Passed);
    assert_eq!(large.final_metrics.as_ref().unwrap()["ticks"], 6);
    assert_eq!(large.partial_metrics["cost"], 4);
    assert_eq!(large.rating, Some(3));
    assert_eq!(large.visible_cases.len(), 2);
    assert!(large.hidden_failure.is_none());
    for case in &large.visible_cases {
        assert!(case.source_index < 2);
        let Some(SceneComparison::Static {
            input,
            expected_output,
            actual_output,
        }) = &case.comparison
        else {
            panic!("static comparison")
        };
        assert_eq!(input, expected_output);
        assert_eq!(input, actual_output);
        assert!(!input.contains(&211));
    }
}
#[test]
fn debug_runs_visible_cases_after_failure_and_official_redacts_hidden_failure() {
    let mut v = author("exact");
    v["evaluation"]["tests"] = json!([
        {"visible":true,"input":[],"expected_output":[0]},
        {"visible":true,"input":[],"expected_output":[1]},
        {"visible":false,"input":[211],"expected_output":[211]}]);
    let debug = evaluate_scene(
        load(&v),
        program(&[".1", ";"]),
        EvaluationMode::Debug,
        config(),
        limits(),
    );
    assert_eq!(debug.status, EvaluationStatus::TestFailed);
    assert_eq!(debug.visible_cases.len(), 2);
    assert!(debug
        .visible_cases
        .iter()
        .any(|c| c.outcome == VisibleSceneOutcome::Passed));
    assert!(debug
        .visible_cases
        .iter()
        .any(|c| c.outcome == VisibleSceneOutcome::WrongOutput));
    assert!(debug.final_metrics.is_none());
    assert!(debug.rating.is_none());
    assert!(debug.hidden_failure.is_none());
    let mut c = config();
    c.shuffle_seed = (0..100)
        .find(|s| shuffled_indices(vec![0, 1, 2], *s)[0] == 2)
        .unwrap();
    let official = evaluate_scene(
        load(&v),
        program(&[".1", ";"]),
        EvaluationMode::Official,
        c,
        limits(),
    );
    assert_eq!(official.status, EvaluationStatus::TestFailed);
    assert_eq!(
        official.hidden_failure,
        Some(HiddenSceneFailure::SceneFailure)
    );
    assert!(official.visible_cases.is_empty());
    assert_eq!(
        official.partial_metrics.get("ticks").copied().unwrap_or(0),
        0
    );
}
#[test]
fn register_input_and_scene_state_reset_for_every_case() {
    let mut v = author("exact");
    v["evaluation"]["tests"] = json!([
        {"visible":true,"input":[9],"expected_output":[9]},
        {"visible":true,"input":[],"expected_output":[0]}]);
    let result = evaluate_scene(
        load(&v),
        program(&[",", ".", ";"]),
        EvaluationMode::Official,
        config(),
        limits(),
    );
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert_eq!(result.partial_metrics["ticks"], 6);
    let mut v = two_point_robot();
    let original = v["evaluation"]["tests"][0].clone();
    v["evaluation"]["tests"] = json!([original.clone(), original.clone(), original]);
    let result = evaluate_scene(
        load(&v),
        program(&[".1", ".1", ";"]),
        EvaluationMode::Official,
        config(),
        limits(),
    );
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert_eq!(result.partial_metrics["ticks"], 9);
    for case in &result.visible_cases {
        assert_eq!(case.summary["frames"], 2);
    }
}
#[test]
fn cancellation_keeps_visible_partial_metrics_and_not_completed_case() {
    let v = two_point_robot();
    let mut s = start_scene_evaluation(
        load(&v),
        program(&[".0", ".0", ";"]),
        EvaluationMode::Debug,
        config(),
        limits(),
    );
    assert_eq!(
        s.advance(n(1), n(100_000)),
        SceneEvaluationProgress::Pending
    );
    s.cancel();
    let result = s.result().unwrap().clone();
    assert_eq!(result.status, EvaluationStatus::Cancelled);
    assert_eq!(result.partial_metrics["ticks"], 1);
    assert_eq!(result.visible_cases.len(), 1);
    assert_eq!(
        result.visible_cases[0].outcome,
        VisibleSceneOutcome::NotCompleted
    );
    assert!(result.visible_cases[0].failure.is_none());
    assert!(result.rating.is_none());
    assert_eq!(
        s.advance(n(100), n(100_000)),
        SceneEvaluationProgress::Complete(result)
    );
}
#[test]
fn evaluation_work_and_feedback_limits_do_not_reset_between_cases() {
    let mut v = author("exact");
    let case = v["evaluation"]["tests"][0].clone();
    v["evaluation"]["tests"] = json!([case.clone(), case]);
    let mut c = config();
    c.safety.cumulative_work = n(3);
    let result = evaluate_scene(
        load(&v),
        program(&[",", ".", ";"]),
        EvaluationMode::Official,
        c,
        limits(),
    );
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.visible_cases.len(), 1);
    assert_eq!(result.partial_metrics["ticks"], 3);
    let mut l = limits();
    l.max_scene_feedback_bytes = n(1);
    let result = evaluate_scene(
        load(&v),
        program(&[",", ".", ";"]),
        EvaluationMode::Official,
        config(),
        l,
    );
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert!(result.visible_cases.is_empty());
}
#[test]
fn hidden_resource_failure_exposes_no_player_failure_or_hidden_metrics() {
    let mut v = two_point_robot();
    let case = v["evaluation"]["tests"][0].clone();
    v["evaluation"]["tests"] = json!([case.clone(), case]);
    v["evaluation"]["tests"][1]["visible"] = json!(false);
    let mut c = config();
    c.shuffle_seed = (0..100)
        .find(|s| shuffled_indices(vec![0, 1], *s)[0] == 1)
        .unwrap();
    c.safety.per_test_ticks = n(2);
    let result = evaluate_scene(
        load(&v),
        program(&[".0", ".0", ";"]),
        EvaluationMode::Official,
        c,
        limits(),
    );
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert!(result.hidden_failure.is_none());
    assert!(result.visible_cases.is_empty());
    assert_eq!(result.partial_metrics.get("ticks").copied().unwrap_or(0), 0);
}
#[test]
fn mechanical_comparison_excludes_hidden_input_robot_records() {
    let v = author("arm");
    let p = program(&[".1", ".4", ".2", ".1", ".4", ".2", ".1", ".4", ".2", ";"]);
    let result = evaluate_scene(load(&v), p, EvaluationMode::Official, config(), limits());
    assert_eq!(result.status, EvaluationStatus::Passed);
    let Some(SceneComparison::MechanicalArm {
        expected_output,
        actual_output,
    }) = &result.visible_cases[0].comparison
    else {
        panic!("arm comparison")
    };
    assert_eq!(expected_output, &[33]);
    assert_eq!(actual_output, &[33]);
}

#[test]
fn failure_tick_is_local_to_case_and_runtime_pairs_keep_registry_identity() {
    let mut v = author("exact");
    v["evaluation"]["tests"] = json!([
        {"visible":true,"input":[9],"expected_output":[9]},
        {"visible":true,"input":[2],"expected_output":[3]}]);
    let result = evaluate_scene(
        load(&v),
        program(&[",", ".", ";"]),
        EvaluationMode::Debug,
        config(),
        limits(),
    );
    let wrong = result
        .visible_cases
        .iter()
        .find(|c| c.outcome == VisibleSceneOutcome::WrongOutput)
        .unwrap();
    let Some(VisibleSceneFailure::Scene { tick, .. }) = &wrong.failure else {
        panic!("visible wrong output")
    };
    assert_eq!(*tick, Some(3));
    assert_eq!(result.partial_metrics["ticks"], 6);
    let failure = VisibleSceneFailure::RuntimeError {
        codes: vec!["ConcurrentOutputConflict".into()],
    };
    let value = codegrid_level_core::scene_feedback::visible_failure_value(&failure, false);
    assert_eq!(value["code"], "level.runtime_error");
    assert_eq!(value["error_number"], "9028");
    assert_eq!(
        value["details"]["runtime_errors"][0]["code"],
        "ConcurrentOutputConflict"
    );
    assert_eq!(
        value["details"]["runtime_errors"][0]["error_number"],
        codegrid_model::error_number("vm", "ConcurrentOutputConflict").unwrap()
    );
}
