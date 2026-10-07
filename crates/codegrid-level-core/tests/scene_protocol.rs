use codegrid_level_core::scene_protocol::{SceneFailure, SceneMachine, SceneOutcome};
use codegrid_level_core::{load_scene_level_json, LoadLimits};
use serde_json::{json, Value};
mod support;
use support::{set_robot_path, set_robot_patrol, set_robot_start};
fn example(name: &str) -> Value {
    serde_json::from_str(match name {
        "robot" => include_str!("../../../examples/scene-level-v2/robot.json"),
        "arm" => include_str!("../../../examples/scene-level-v2/mechanical-arm.json"),
        "exact" => include_str!("../../../examples/scene-level-v2/exactio.json"),
        _ => panic!("fixture name"),
    })
    .unwrap()
}
fn machine(v: &Value) -> SceneMachine {
    let level = load_scene_level_json(
        v.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 1_000_000,
            max_tests: 20,
            max_total_test_bytes: 1_000_000,
        },
    )
    .unwrap();
    SceneMachine::new(&level, 0).unwrap()
}
#[test]
fn complete_frame_dispatches_in_order_and_terminal_a_ignores_invalid_b() {
    let mut v = example("robot");
    v["scene_config"]["robot_count"] = json!(2);
    set_robot_path(&mut v, 4);
    set_robot_start(&mut v, "B", 3, "left");
    let mut m = machine(&v);
    assert_eq!(m.take_initial_input(), [0, 0, 3, 0]);
    assert!(m.take_initial_input().is_empty());
    let first = m.consume(1).unwrap();
    assert!(!first.completed_frame);
    assert_eq!(m.frames(), 0);
    assert_eq!(m.pending_actions(), [1]);
    let second = m.consume(255).unwrap();
    assert!(second.completed_frame);
    assert_eq!(second.outcome, SceneOutcome::Passed);
    assert_eq!((m.frames(), m.rounds()), (1, 0));
    assert!(second.observation.is_empty());
    assert_eq!(m.consume(255).unwrap().outcome, SceneOutcome::Passed);
}
#[test]
fn static_domains_comparison_and_silent_empty_halt() {
    let mut generic = example("exact");
    generic["evaluation"]["tests"][0]["input"] = json!([3, 32, 255]);
    generic["evaluation"]["tests"][0]["expected_output"] = json!([32, 255, 2]);
    let mut m = machine(&generic);
    assert_eq!(m.take_initial_input(), [3, 32, 255]);
    assert_eq!(m.consume(32).unwrap().outcome, SceneOutcome::Running);
    assert_eq!(m.consume(255).unwrap().outcome, SceneOutcome::Running);
    assert_eq!(m.consume(2).unwrap().outcome, SceneOutcome::Passed);
    let mut m = machine(&example("exact"));
    assert_eq!(m.consume(9).unwrap().outcome, SceneOutcome::Passed);
    let mut v = example("exact");
    v["evaluation"]["tests"][0]["expected_output"] = json!([]);
    let mut m = machine(&v);
    assert_eq!(m.outcome(), &SceneOutcome::Running);
    m.halt();
    assert_eq!(m.outcome(), &SceneOutcome::Passed);
    let mut m = machine(&v);
    assert!(matches!(
        m.consume(1).unwrap().outcome,
        SceneOutcome::Failed(SceneFailure::WrongOutput { expected: None, .. })
    ));
}
#[test]
fn robot_initial_input_and_partial_frame_halt() {
    let mut v = example("robot");
    let mut m = machine(&v);
    assert_eq!(m.take_initial_input(), [0, 0]);
    m.halt();
    assert_eq!(
        m.outcome(),
        &SceneOutcome::Failed(SceneFailure::IncompleteGoal)
    );
    v["scene_config"]["robot_count"] = json!(2);
    set_robot_start(&mut v, "A", 0, "right");
    set_robot_path(&mut v, 4);
    set_robot_patrol(&mut v, 0, 2);
    set_robot_start(&mut v, "B", 3, "left");
    let mut m = machine(&v);
    m.consume(1).unwrap();
    m.halt();
    assert_eq!(
        m.outcome(),
        &SceneOutcome::Failed(SceneFailure::IncompleteGoal)
    );
    assert_eq!(m.pending_actions(), [1]);
    assert_eq!(m.frames(), 0);
}
#[test]
fn arm_event_input_and_direct_packing_sequence() {
    let mut m = machine(&example("arm"));
    assert!(m.initial_input().is_empty());
    assert_eq!(m.consume(1).unwrap().observation, [13]);
    m.consume(4).unwrap();
    m.consume(2).unwrap();
    assert_eq!(m.consume(1).unwrap().observation, [1]);
    m.consume(4).unwrap();
    m.consume(2).unwrap();
    assert_eq!(m.consume(1).unwrap().observation, [33]);
    m.consume(4).unwrap();
    let terminal = m.consume(2).unwrap();
    assert_eq!(terminal.outcome, SceneOutcome::Passed);
    assert!(terminal.observation.is_empty());
    assert_eq!(m.actual_output(), Some([33].as_slice()));
}
