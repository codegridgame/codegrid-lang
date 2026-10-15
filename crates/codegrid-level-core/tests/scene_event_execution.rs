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
    let tokens = [",", ",", ".1", ".2", ",", ",", ".", ".2", ";"];
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
            "InputUpdated",
            "ActionApplied",
            "RoundCompleted",
            "InputUpdated",
            "ActionApplied",
            "RoundCompleted",
            "InputUpdated",
            "ActionApplied",
            "RoundCompleted",
            "InputUpdated",
            "ActionApplied",
            "CaseEnded"
        ]
    );
    assert_eq!(trace[2]["data"]["effect"]["from_position"], 0);
    assert_eq!(trace[2]["data"]["effect"]["to_position"], 1);
    assert_eq!(trace[3]["data"]["observation"], json!([0, 1]));
    assert_eq!(trace[5]["data"]["action"], 2);
    assert_eq!(trace[8]["data"]["effect"]["to_position"], 2);
    assert_eq!(trace[11]["data"]["action"], 2);
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
    let mut s = start(&v, &[".1", ".0", ".2", ".9", ";"], limits());
    assert_eq!(finish(&mut s, 1).status, EvaluationStatus::Passed);
    let trace = events(&mut s);
    assert_eq!(
        kinds(&trace),
        [
            "CaseStarted",
            "InputUpdated",
            "ActionApplied",
            "ActionApplied",
            "RoundCompleted",
            "InputUpdated",
            "ActionApplied",
            "CaseEnded"
        ]
    );
    assert_eq!(trace[2]["actor"], "A");
    assert_eq!(trace[6]["actor"], "A");
    assert_eq!(trace[6]["frame_index"], "2");
    assert!(trace[7]["data"]["failure"].is_null());
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
        ["CaseStarted", "InputUpdated", "ActionApplied", "CaseEnded"]
    );
    assert_eq!(trace[2]["data"]["effect"]["to_position"], 1);
    assert_eq!(trace[3]["data"]["failure"]["details"]["actor"], "B");
    assert_eq!(trace[3]["data"]["failure"]["details"]["value"], 5);
}
#[test]
fn input_and_event_capacity_failures_publish_only_last_boundary_and_end() {
    let v = two_point_robot();
    let mut l = limits();
    l.max_total_input_bytes_per_test = n(3);
    let mut s = start(&v, &[".1", ";"], l);
    let result = finish(&mut s, 1000);
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.visible_cases[0].summary["frames"], 0);
    let trace = events(&mut s);
    assert_eq!(kinds(&trace), ["CaseStarted", "InputUpdated", "CaseEnded"]);
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
fn flip_is_immediate_and_failed_input_drop_has_station_location() {
    let v = author("arm");
    let mut s = start(&v, &[".1", ".4", ".2", ".1", ".3", ".2", ";"], limits());
    let result = finish(&mut s, 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    let trace = events(&mut s);
    let snapshots = trace
        .iter()
        .filter(|e| e["kind"] == "InputUpdated")
        .map(|e| e["data"]["bytes"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        snapshots,
        vec![
            json!([255, 0]),
            json!([1, 0]),
            json!([1, 1]),
            json!([255, 1]),
            json!([5, 1]),
            json!([5, 0])
        ]
    );
    assert!(trace.iter().any(|e| e["kind"] == "RoundCompleted"
        && e["data"]["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["station_id"] == 1 && t["contents"] == json!([5]))));
    let end = trace.last().unwrap();
    assert_eq!(end["data"]["failure"]["reason"], "IllegalOperation");
    assert_eq!(end["data"]["failure"]["details"]["station_id"], 0);
}
#[test]
fn dual_shared_noop_conflict_publishes_no_partial_actions() {
    let mut v = author("arm");
    v["scene_config"]["arm_count"] = json!(2);
    v["scene_config"]["stations"] = json!((0..10)
        .map(|id| json!({"station_id":id,"station_type":if id==3 {"OUTPUT"} else {"STACK"}}))
        .collect::<Vec<_>>());
    v["evaluation"]["tests"][0]["initial_station_contents"] =
        json!([[], [], [], [], [], [], [], [], [], []]);
    v["evaluation"]["tests"][0]["input_queue"] = json!([]);
    // A turns left toward shared station 5, B turns right toward the same station.
    let mut s = start(&v, &[".3", ".4", ".2", ".1", ";"], limits());
    assert_eq!(finish(&mut s, 1).status, EvaluationStatus::TestFailed);
    let trace = events(&mut s);
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "ActionApplied")
            .count(),
        2
    );
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "RoundCompleted")
            .count(),
        1
    );
    assert_eq!(
        trace.last().unwrap()["data"]["failure"]["reason"],
        "IllegalOperation"
    );
}

#[test]
fn unrepaired_patrol_fails_on_halt_without_action_events() {
    let v = author("robot");
    let mut s = start(&v, &[";"], limits());
    let result = finish(&mut s, 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    let trace = events(&mut s);
    assert_eq!(kinds(&trace), ["CaseStarted", "InputUpdated", "CaseEnded"]);
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
        program(&[".1", ".2", ";"]),
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
        ["CaseStarted", "InputUpdated", "CaseEnded"]
    );
    assert!(s.scene_work() <= 12_000);
    let mut l = limits();
    l.max_scene_events_per_evaluation = n(6);
    let mut s = start(&v, &[".0", ".1", ".2", ";"], l);
    assert_eq!(
        s.advance(n(2), n(100_000)),
        SceneEvaluationProgress::Pending
    );
    let first = s.scene_feedback(0, n(6)).unwrap();
    assert_eq!(first.next_sequence, 5);
    assert!(s.scene_feedback(5, n(6)).unwrap().events.is_empty());
    assert_eq!(finish(&mut s, 1000).status, EvaluationStatus::Passed);
    let last = s.scene_feedback(5, n(6)).unwrap();
    assert_eq!(last.next_sequence, 10);
    assert_eq!(last.events.len(), 5);
}
