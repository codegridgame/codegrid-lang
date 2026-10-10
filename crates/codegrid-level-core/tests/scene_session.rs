use codegrid_ir::Cell;
use codegrid_level_core::{scene_protocol::SceneOutcome, scene_session::*, *};
use codegrid_model::{Direction, PrimaryInstruction};
use serde_json::json;
mod support;
use support::*;
#[test]
fn continuous_read_observation_and_vm_slices_match() {
    let v = two_point_robot();
    let p = program(&[",", ",", ".1", ",", ".", ";"]);
    let large = run(session(&v, p.clone(), config(), limits()), 1000);
    let small = run(session(&v, p, config(), limits()), 1);
    assert_eq!(large, small);
    assert_eq!(large.status, EvaluationStatus::Passed);
    assert_eq!(large.summary["frames"], 2);
    assert_eq!(large.summary["rounds"], 1);
    assert_eq!(large.metrics["ticks"], 6);
}
#[test]
fn queued_output_waits_for_scene_budget_without_rerunning_vm() {
    let v = author("robot");
    let mut s = session(&v, program(&[".1", ";"]), config(), limits());
    assert_eq!(s.advance(n(1), n(100_000)), SceneCaseProgress::Pending);
    assert_eq!(s.advance(n(1), n(1)), SceneCaseProgress::Pending);
    let work = s.vm_work();
    assert_eq!(work, 2);
    let SceneCaseProgress::Complete(result) = s.advance(n(1), n(100_000)) else {
        panic!("pending committed output")
    };
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert_eq!(s.vm_work(), work);
    assert_eq!(result.metrics["ticks"], 2);
}
#[test]
fn queue_and_cumulative_input_failures_discard_candidate_round() {
    let v = two_point_robot();
    let mut l = limits();
    l.max_input_queue_bytes = n(2);
    let result = run(session(&v, program(&[".1", ";"]), config(), l), 100);
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.summary["frames"], 0);
    assert_eq!(result.summary["rounds"], 0);
    assert_eq!(result.metrics["ticks"], 2);
    let mut l = limits();
    l.max_total_input_bytes_per_test = n(3);
    let result = run(
        session(&v, program(&[",", ",", ".1", ";"]), config(), l),
        100,
    );
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.summary["rounds"], 0);
}
#[test]
fn halt_and_constraint_terminal_order() {
    let v = two_point_robot();
    let result = run(session(&v, program(&[".1", ";"]), config(), limits()), 100);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    assert!(matches!(result.outcome, Some(SceneOutcome::Failed(_))));
    let mut v = author("robot");
    v["constraints"]["max_ticks"] = json!(1);
    let result = run(session(&v, program(&[".1", ";"]), config(), limits()), 100);
    assert_eq!(result.status, EvaluationStatus::ConstraintExceeded);
    assert_eq!(result.outcome, Some(SceneOutcome::Passed));
    let v = author("robot");
    let wrap_config = config();
    let result = run(
        session(
            &v,
            grid(
                vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(PrimaryInstruction::from_token(".1").unwrap(), None),
                    Cell::entry(Direction::Right),
                    Cell::instruction(PrimaryInstruction::Halt, None),
                ],
                2,
                2,
            ),
            wrap_config,
            limits(),
        ),
        100,
    );
    assert_eq!(result.status, EvaluationStatus::Passed, "{result:?}");
}
#[test]
fn simultaneous_output_halt_checks_constraints_before_incomplete_goal() {
    let mut v = two_point_robot();
    v["constraints"]["max_ticks"] = json!(1);
    let c = config();
    let p = grid(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::from_token(".0").unwrap(), None),
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        2,
        2,
    );
    for slice in [2, 100] {
        let result = run(session(&v, p.clone(), c.clone(), limits()), slice);
        assert_eq!(result.status, EvaluationStatus::ConstraintExceeded);
        assert_eq!(result.outcome, Some(SceneOutcome::Running));
        assert_eq!(result.summary["frames"], 1);
    }
}
#[test]
fn frame_tick_work_and_initial_state_ceilings_are_distinct() {
    let v = two_point_robot();
    let mut l = limits();
    l.max_scene_frames_per_test = n(1);
    let result = run(session(&v, program(&[".0", ".0", ";"]), config(), l), 100);
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.summary["frames"], 1);
    let mut c = config();
    c.safety.per_test_ticks = n(2);
    let result = run(session(&v, program(&[".0", ".0", ";"]), c, limits()), 100);
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(result.metrics["ticks"], 2);
    let mut l = limits();
    l.max_scene_state_units = n(1);
    assert_eq!(
        run(session(&v, program(&[".1"]), config(), l), 100).status,
        EvaluationStatus::ResourceLimitExceeded
    );
    let mut l = limits();
    l.max_total_scene_work = n(1);
    assert_eq!(
        run(session(&v, program(&[".1"]), config(), l), 100).status,
        EvaluationStatus::ResourceLimitExceeded
    );
}
#[test]
fn arm_events_follow_committed_ticks() {
    let v = author("arm");
    let p = program(&[".1", ".4", ".2", ".1", ".4", ".2", ".1", ".4", ".2", ";"]);
    let result = run(session(&v, p, config(), limits()), 1);
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert_eq!(result.summary["matched_output_robots"], 1);
}
#[test]
fn rollback_output_is_never_dispatched_and_cancel_is_terminal() {
    let v = author("robot");
    let p = grid(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::from_token(".1").unwrap(), None),
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::from_token(".1").unwrap(), None),
        ],
        2,
        2,
    );
    let result = run(session(&v, p, config(), limits()), 100);
    assert_eq!(result.status, EvaluationStatus::RuntimeError);
    assert_eq!(result.summary["frames"], 0);
    assert!(!result.runtime_errors.is_empty());
    let mut s = session(&v, program(&[".1"]), config(), limits());
    s.cancel();
    let before = s.result().cloned();
    s.advance(n(100), n(100_000));
    assert_eq!(s.result(), before.as_ref());
    assert_eq!(s.vm_work(), 0);
}

#[test]
fn tight_scene_budget_reports_resource_limit_not_overflow_fault() {
    let v = author("robot");
    // The Robot world initialization bills 518 work units (16x16 map plus the
    // robot and its observation input). A per-call budget at the stale 512
    // reservation floor is a resource shortage, never a NumericOverflow fault.
    let mut l = limits();
    l.max_scene_work_per_call = n(512);
    assert_eq!(
        run(session(&v, program(&[".1", ";"]), config(), l), 100).status,
        EvaluationStatus::ResourceLimitExceeded
    );
    // Exactly the initialization work fits the per-call budget: the candidate
    // world is published with its exact charge and the case stays pending.
    let mut l = limits();
    l.max_scene_work_per_call = n(518);
    let mut s = session(&v, program(&[".1", ";"]), config(), l);
    assert_eq!(s.advance(n(1), n(518)), SceneCaseProgress::Pending);
    assert_eq!(s.scene_work(), 518);
    // The next transition can never fit this budget, so the case ends as a
    // resource limit, never as a NumericOverflow fault.
    assert_eq!(run(s, 518).status, EvaluationStatus::ResourceLimitExceeded);
}

#[test]
fn robot_initialization_reserves_world_state_and_work_before_construction() {
    let v = author("robot");
    let level = load(&v);
    let definition = scene_definition_units(&level).unwrap();
    let mut l = limits();
    // The independent map, actor and two observation bytes need 259 units.
    l.max_scene_state_units = n(definition + 258);
    let mut s = session(&v, program(&[".1", ";"]), config(), l);
    let SceneCaseProgress::Complete(result) = s.advance(n(1), n(100_000)) else {
        panic!("initial world state must be refused")
    };
    assert_eq!(result.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(s.scene_work(), 0);
    assert_eq!(s.vm_work(), 0);

    let mut s = session(&v, program(&[".1", ";"]), config(), limits());
    assert_eq!(s.advance(n(1), n(517)), SceneCaseProgress::Pending);
    assert_eq!(s.scene_work(), 0);
    assert_eq!(s.vm_work(), 0);
    assert_eq!(s.advance(n(1), n(518)), SceneCaseProgress::Pending);
    assert_eq!(s.scene_work(), 518);
}

#[test]
fn completion_at_exact_tick_and_vm_work_ceiling_is_allowed() {
    let v = author("robot");
    let mut c = config();
    c.safety.per_test_ticks = n(2);
    c.safety.cumulative_work = n(2);
    let result = run(session(&v, program(&[".1", ";"]), c, limits()), 1);
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert_eq!(result.metrics["ticks"], 2);
}
#[test]
fn byte_append_keeps_older_unread_snapshot_at_queue_head() {
    let v = two_point_robot();
    // Two READs after the move still consume the initial [0,0], not the new [1,0].
    let result = run(
        session(&v, program(&[".1", ",", ",", ".", ";"]), config(), limits()),
        1,
    );
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    assert_eq!(result.summary["frames"], 2);
    assert_eq!(result.summary["rounds"], 2);
    assert_eq!(result.summary["visited_patrol_points"], 0);
}
#[test]
fn static_output_passes_and_robot_halt_without_patrol_visit_fails() {
    let v = author("exact");
    let result = run(
        session(&v, program(&[",", ".", ";"]), config(), limits()),
        1,
    );
    assert_eq!(result.status, EvaluationStatus::Passed);
    assert!(result.summary.is_empty());
    let v = author("robot");
    let result = run(session(&v, program(&[";"]), config(), limits()), 1);
    assert_eq!(result.status, EvaluationStatus::TestFailed);
    assert_eq!(result.summary["frames"], 0);
    assert_eq!(result.summary["rounds"], 0);
}
