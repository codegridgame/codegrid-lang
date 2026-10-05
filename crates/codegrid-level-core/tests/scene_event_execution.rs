mod support;
use codegrid_level_core::{scene_evaluate::*, scene_feedback::*, *};
use serde_json::{json, Value};
use support::*;
fn start(
    v: &Value,
    tokens: &[&str],
    l: codegrid_level_core::scene_session::SceneLimits,
) -> SceneEvaluationSession {
    start_scene_evaluation(load(v), program(tokens), EvaluationMode::Debug, config(), l)
}
fn finish(s: &mut SceneEvaluationSession, slice: u64) -> SceneEvaluationResult {
    for _ in 0..2000 {
        if let SceneEvaluationProgress::Complete(result) = s.advance(n(slice), n(100_000)) {
            return result;
        }
    }
    panic!("bounded event evaluation did not terminate")
}
fn events(s: &mut SceneEvaluationSession) -> Vec<Value> {
    s.scene_feedback(0, n(1000))
        .unwrap()
        .events
        .iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect()
}
fn kinds(events: &[Value]) -> Vec<&str> {
    events.iter().map(|e| e["kind"].as_str().unwrap()).collect()
}
#[test]
fn robot_events_and_results_are_identical_across_vm_slices() {
    let v = two_point_robot();
    let tokens = [",>", ",>", ".1", ",>", ".", ";"];
    let mut large = start(&v, &tokens, limits());
    let mut small = start(&v, &tokens, limits());
    assert_eq!(finish(&mut large, 1000), finish(&mut small, 1));
    assert_eq!(large.scene_work(), small.scene_work());
    let trace = events(&mut large);
    assert_eq!(trace, events(&mut small));
    assert_eq!(
        kinds(&trace),
        [
            "CaseStarted",
            "InputAppended",
            "ActionApplied",
            "RoundCompleted",
            "InputAppended",
            "ActionApplied",
            "CaseEnded"
        ]
    );
    assert_eq!(trace[2]["data"]["effect"]["from_position"], 0);
    assert_eq!(trace[2]["data"]["effect"]["to_position"], 1);
    assert_eq!(trace[3]["data"]["observation"], json!([1, 0]));
    assert_eq!(trace[5]["data"]["effect"]["to_position"], 2);
    for (index, event) in trace.iter().enumerate() {
        assert_eq!(event["sequence"], (index + 1).to_string());
    }
}
#[test]
fn terminal_a_ignores_invalid_b_without_round_events() {
    let mut v = author("robot");
    v["scene_config"]["robot_count"] = json!(2);
    set_robot_path(&mut v, 4);
    set_robot_start(&mut v, "B", 3, "left");
    let mut s = start(&v, &[".1", ".9", ";"], limits());
    assert_eq!(finish(&mut s, 1).status, EvaluationStatus::Passed);
    let trace = events(&mut s);
    assert_eq!(
        kinds(&trace),
        ["CaseStarted", "InputAppended", "ActionApplied", "CaseEnded"]
    );
    assert_eq!(trace[2]["actor"], "A");
    assert_eq!(trace[2]["frame_index"], "1");
    assert!(trace[3]["data"]["failure"].is_null());
}
#[test]
fn invalid_b_retains_a_effect_but_no_round_or_observation() {
    let mut v = two_point_robot();
    v["scene_config"]["robot_count"] = json!(2);
    set_robot_path(&mut v, 4);
    set_robot_start(&mut v, "B", 3, "left");
    let mut s = start(&v, &[".1", ".5", ";"], limits());
    let result = finish(&mut s, 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    assert_eq!(result.visible_cases[0].summary["rounds"], 0);
    let trace = events(&mut s);
    assert_eq!(
        kinds(&trace),
        ["CaseStarted", "InputAppended", "ActionApplied", "CaseEnded"]
    );
    assert_eq!(trace[2]["data"]["effect"]["to_position"], 1);
    assert_eq!(trace[3]["data"]["failure"]["details"]["actor"], "B");
    assert_eq!(trace[3]["data"]["failure"]["details"]["value"], 5);
}
#[test]
fn input_and_event_capacity_failures_publish_only_last_boundary_and_end() {
    let v = two_point_robot();
    let mut l = limits();
    l.max_input_queue_bytes = n(2);
    let mut s = start(&v, &[".1", ";"], l);
    let result = finish(&mut s, 1000);
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.visible_cases[0].summary["frames"], 0);
    let trace = events(&mut s);
    assert_eq!(kinds(&trace), ["CaseStarted", "InputAppended", "CaseEnded"]);
    assert_eq!(trace[2]["data"]["outcome"], "NotCompleted");
    let mut l = limits();
    l.max_scene_events_per_evaluation = n(3);
    let mut s = start(&v, &[".0", ";"], l);
    assert_eq!(
        finish(&mut s, 1000).status,
        EvaluationStatus::ResourceLimitExceeded
    );
    let trace = s.scene_feedback(0, n(3)).unwrap().events;
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[2].draft.payload.kind(), "CaseEnded");
}
#[test]
fn door_changes_publish_only_at_round_completion() {
    let mut v = two_point_robot();
    set_robot_path(&mut v, 4);
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"index":1,"type":"trigger","id":7}),
            json!({"index":3,"type":"door","id":7}),
        ]);
    let mut s = start(&v, &[".1", ";"], limits());
    finish(&mut s, 1000);
    let trace = events(&mut s);
    let round = trace
        .iter()
        .find(|e| e["kind"] == "RoundCompleted")
        .unwrap();
    assert_eq!(
        round["data"]["transitions"],
        json!([{"kind":"DoorOpened","door_id":7}])
    );
}
#[test]
fn inspection_is_revealed_only_on_ready_and_illegal_packing_has_location() {
    let mut v = author("arm");
    v["evaluation"]["tests"][0]["input"][0]["true_inspection"] = json!(1);
    let mut s = start(&v, &[".1", ".4", ".2", ".1", ".4", ".2", ";"], limits());
    let result = finish(&mut s, 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    let trace = events(&mut s);
    assert!(!serde_json::to_string(&trace)
        .unwrap()
        .contains("true_inspection"));
    let first_grab = trace.iter().find(|e| e["kind"] == "ActionApplied").unwrap();
    assert_eq!(first_grab["data"]["effect"]["after_state"], 13);
    let ready = trace
        .iter()
        .find(|e| {
            e["kind"] == "RoundCompleted"
                && !e["data"]["transitions"].as_array().unwrap().is_empty()
        })
        .unwrap();
    assert_eq!(
        ready["data"]["transitions"],
        json!([{"kind":"TableReady","worktable_index":0,"state":5}])
    );
    assert_eq!(ready["data"]["observation"], json!([]));
    let end = trace.last().unwrap();
    assert_eq!(end["data"]["failure"]["reason"], "IllegalOperation");
    assert_eq!(
        end["data"]["failure"]["details"]["interaction"],
        "Worktable"
    );
    assert_eq!(end["data"]["failure"]["details"]["worktable_index"], 1);
}
#[test]
fn dual_buffer_drop_cannot_be_grabbed_until_following_round() {
    let mut v = author("arm");
    v["scene_config"]["arm_count"] = json!(2);
    let mut s = start(
        &v,
        &[".1", ".0", ".3", ".4", ".2", ".1", ".0", ".1", ";"],
        limits(),
    );
    finish(&mut s, 1);
    let trace = events(&mut s);
    let rounds = trace
        .iter()
        .filter(|e| e["kind"] == "RoundCompleted")
        .collect::<Vec<_>>();
    assert_eq!(rounds[2]["data"]["observation"], json!([255, 255]));
    assert_eq!(
        rounds[2]["data"]["transitions"],
        json!([{"kind":"BufferReady","buffer":"Left","state":13}])
    );
    assert_eq!(rounds[3]["data"]["observation"], json!([255, 13]));
}
#[test]
fn unvisited_patrol_fails_on_halt_without_action_events() {
    let v = author("robot");
    let mut s = start(&v, &[";"], limits());
    let result = finish(&mut s, 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    let trace = events(&mut s);
    assert_eq!(kinds(&trace), ["CaseStarted", "InputAppended", "CaseEnded"]);
    assert_eq!(trace[0]["data"]["observation"], json!([0, 0]));
    assert_eq!(trace[2]["data"]["failure"]["reason"], "IncompleteGoal");
}
#[test]
fn official_execution_never_constructs_or_exposes_debug_events() {
    let v = author("robot");
    let mut l = limits();
    l.max_scene_events_per_evaluation = n(1);
    let mut s = start_scene_evaluation(
        load(&v),
        program(&[".1", ";"]),
        EvaluationMode::Official,
        config(),
        l,
    );
    assert_eq!(finish(&mut s, 1).status, EvaluationStatus::Passed);
    assert!(matches!(
        s.scene_feedback(0, n(1)),
        Err(SceneFeedbackError::InvalidConfiguration)
    ));
}
#[test]
fn resource_work_end_is_reserved_and_acknowledgement_frees_event_capacity() {
    let v = author("robot");
    let mut l = limits();
    l.max_total_scene_work = n(12_000);
    let mut s = start(&v, &[".1", ";"], l);
    assert_eq!(
        finish(&mut s, 1000).status,
        EvaluationStatus::ResourceLimitExceeded
    );
    assert_eq!(
        kinds(&events(&mut s)),
        ["CaseStarted", "InputAppended", "CaseEnded"]
    );
    assert!(s.scene_work() <= 12_000);
    let mut l = limits();
    l.max_scene_events_per_evaluation = n(6);
    let mut s = start(&v, &[".0", ".1", ";"], l);
    assert_eq!(
        s.advance(n(2), n(100_000)),
        SceneEvaluationProgress::Pending
    );
    let first = s.scene_feedback(0, n(6)).unwrap();
    assert_eq!(first.next_sequence, 5);
    assert!(s.scene_feedback(5, n(6)).unwrap().events.is_empty());
    assert_eq!(finish(&mut s, 1000).status, EvaluationStatus::Passed);
    let last = s.scene_feedback(5, n(6)).unwrap();
    assert_eq!(last.next_sequence, 7);
    assert_eq!(last.events.len(), 2);
}
