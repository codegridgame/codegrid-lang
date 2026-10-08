use codegrid_ir::{Board, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION};
use codegrid_level_core::*;
use codegrid_model::{BoundaryMode, Direction, PrimaryInstruction as P};
use std::{collections::BTreeMap, num::NonZeroU64};
fn config() -> EvaluationConfig {
    EvaluationConfig {
        boundary_mode: BoundaryMode::Exit,
        shuffle_seed: 42,
        custom_execution_limit: NonZeroU64::new(100).unwrap(),
        safety: ExecutionSafetyProfile {
            id: "test".into(),
            version: 1,
            per_test_ticks: NonZeroU64::new(100).unwrap(),
            cumulative_work: NonZeroU64::new(1000).unwrap(),
            per_call_work: NonZeroU64::new(100).unwrap(),
            max_output_bytes: NonZeroU64::new(100).unwrap(),
            max_state_units: NonZeroU64::new(10000).unwrap(),
            max_feedback_bytes: NonZeroU64::new(1000000).unwrap(),
        },
    }
}
fn program(instructions: &[P]) -> VerifiedProgram {
    let cells = std::iter::once(Cell::entry(Direction::Right))
        .chain(instructions.iter().map(|p| Cell::instruction(*p, None)))
        .collect::<Vec<_>>();
    VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: cells.len(),
                height: 1,
                cells,
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .unwrap()
}
fn level(tests: serde_json::Value, constraints: serde_json::Value) -> ValidatedLevel {
    let value = serde_json::json!({"format_version":1,"level_id":"test","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":["READ","OUTPUT","HALT","MOVE_RIGHT"],"allowed_attachments":[],"main_board":{"width":100,"height":1},"function_board":{"width":100,"height":1},"max_functions":10,"max_custom":10,"max_threads":1,"memory_enabled":true},"constraints":constraints,"scoring":{"metrics":{"ticks":{"target":6}}},"evaluation":{"tests":tests}});
    load_level_json(value.to_string().as_bytes(), 100000).unwrap()
}
#[test]
fn fresh_state_visible_only_metrics_and_slices() {
    let tests = serde_json::json!([{"visible":true,"input":[9],"expected_output":[9]},{"visible":true,"input":[2],"expected_output":[2]},{"visible":false,"input":[4],"expected_output":[4]}]);
    let l = level(tests, serde_json::json!({}));
    let p = program(&[P::Read, P::Output, P::Halt]);
    let expected = evaluate(l.clone(), p.clone(), EvaluationMode::Official, config());
    assert_eq!(expected.status, EvaluationStatus::Passed, "{expected:?}");
    assert_eq!(expected.final_metrics.as_ref().unwrap()["ticks"], 6);
    assert_eq!(expected.final_metrics.as_ref().unwrap()["cost"], 4);
    assert_eq!(expected.rating, Some(3));
    assert_eq!(expected.visible_tests.len(), 2);
    let mut s = start_evaluation(l, p, EvaluationMode::Official, config());
    let actual = loop {
        if let EvaluationProgress::Complete(r) = s.advance(NonZeroU64::MIN) {
            break r;
        }
    };
    assert_eq!(actual, expected);
}
#[test]
fn debug_continues_all_and_official_hidden_failure_is_redacted() {
    let tests = serde_json::json!([{"visible":true,"input":[],"expected_output":[0]},{"visible":false,"input":[211],"expected_output":[211]}]);
    let l = level(tests, serde_json::json!({}));
    let p = program(&[P::Output, P::Halt]);
    let r = evaluate(l.clone(), p.clone(), EvaluationMode::Official, config());
    assert_eq!(r.status, EvaluationStatus::TestFailed);
    assert_eq!(r.hidden_failure, Some(TestOutcome::WrongOutput));
    assert!(r.final_metrics.is_none());
    let r = evaluate(l, p, EvaluationMode::Debug, config());
    assert_eq!(r.status, EvaluationStatus::Passed);
    assert!(r.hidden_failure.is_none());
}
#[test]
fn constraint_breach_overrides_correct_output_and_debug_runs_rest() {
    let tests = serde_json::json!([{"visible":true,"input":[],"expected_output":[0]},{"visible":true,"input":[],"expected_output":[0]}]);
    let r = evaluate(
        level(tests, serde_json::json!({"max_ticks":1})),
        program(&[P::Output, P::Halt]),
        EvaluationMode::Debug,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::ConstraintExceeded);
    assert_eq!(r.visible_tests.len(), 2);
    assert_eq!(r.partial_metrics["ticks"], 4);
    assert!(r.rating.is_none());
}
#[test]
fn empty_output_requires_halt_and_safety_is_distinct() {
    let l = level(
        serde_json::json!([{"visible":true,"input":[],"expected_output":[]}]),
        serde_json::json!({}),
    );
    assert_eq!(
        evaluate(
            l.clone(),
            program(&[P::Halt]),
            EvaluationMode::Official,
            config()
        )
        .status,
        EvaluationStatus::Passed
    );
    let mut c = config();
    c.boundary_mode = BoundaryMode::Wrap;
    c.safety.per_test_ticks = NonZeroU64::new(3).unwrap();
    assert_eq!(
        evaluate(
            l,
            program(&[P::Direction(Direction::Right)]),
            EvaluationMode::Official,
            c
        )
        .status,
        EvaluationStatus::ResourceLimitExceeded
    );
}
#[test]
fn ratings_use_widened_thresholds() {
    let m = BTreeMap::from([("ticks".into(), u64::MAX)]);
    assert_eq!(
        metrics::rating(&m, &BTreeMap::from([("ticks".into(), Some(u64::MAX - 1))])),
        Some(2)
    );
    assert_eq!(metrics::rating(&m, &BTreeMap::new()), None);
}

#[test]
fn shuffle_vectors() {
    assert_eq!(
        shuffled_indices((0..10).collect(), 0),
        vec![6, 3, 4, 8, 9, 2, 7, 1, 0, 5]
    );
    assert_eq!(
        shuffled_indices((0..10).collect(), 42),
        vec![8, 4, 1, 9, 0, 7, 2, 6, 5, 3]
    );
    assert_eq!(
        shuffled_indices((0..10).collect(), u64::MAX),
        vec![9, 1, 8, 7, 4, 5, 3, 2, 0, 6]
    );
}

#[test]
fn insufficient_budget_rolls_back_and_larger_retry_advances() {
    let mut p = program(&[P::Output, P::Halt]).program().clone();
    p.outer.main.height = 2;
    p.outer.main.cells.extend([
        Cell::entry(Direction::Right),
        Cell::instruction(P::Direction(Direction::Right), None),
        Cell::instruction(P::Halt, None),
    ]);
    let p = VerifiedProgram::new(p).unwrap();
    let mut vm = codegrid_vm::Vm::new(
        p,
        [],
        codegrid_vm::VmConfig::new(BoundaryMode::Exit, 0, NonZeroU64::new(100).unwrap()),
    )
    .unwrap();
    let (result, used) = vm.step_with_work_accounting(NonZeroU64::MIN);
    assert!(result.is_err());
    assert_eq!(used, 1);
    assert_eq!(vm.committed_ticks(), 0);
    let (result, used) = vm.step_with_work_accounting(NonZeroU64::new(2).unwrap());
    assert!(result.is_ok());
    assert_eq!(used, 2);
    assert_eq!(vm.committed_ticks(), 1);
}
#[test]
fn custom_generated_forbidden_primary_is_checked_after_commit() {
    use codegrid_ir::CustomDefinition;
    use codegrid_model::{AttachmentInstruction as A, ShiftDirection, Slot};
    let slot = Slot::new(0).unwrap();
    let forbidden = P::Shift(ShiftDirection::Left).instruction_code().unwrap();
    let mut p = program(&[P::Read, P::Push, P::Direction(Direction::Right), P::Halt])
        .program()
        .clone();
    p.outer.main.cells[3].primary = Some(P::Custom(slot));
    p.customs.insert(
        slot,
        CustomDefinition {
            program: ScopedProgram {
                main: Board {
                    width: 5,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(P::Read, None),
                        Cell::instruction(P::Decode, None),
                        Cell::instruction(P::Direction(Direction::Right), Some(A::WriteCode)),
                        Cell::instruction(P::CustomReturn, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
        },
    );
    let p = VerifiedProgram::new(p).unwrap();
    let value = serde_json::json!({"format_version":1,"level_id":"generated","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":["READ","PUSH","DECODE","CUSTOM","CUSTOM_RETURN","HALT","MOVE_RIGHT"],"allowed_attachments":["WRITE_CODE"],"main_board":{"width":100,"height":1},"function_board":{"width":100,"height":1},"max_functions":10,"max_custom":10,"max_threads":1,"memory_enabled":true},"constraints":{},"scoring":{"metrics":{}},"evaluation":{"tests":[{"visible":true,"input":[forbidden],"expected_output":[]}]}});
    let l = load_level_json(value.to_string().as_bytes(), 100000).unwrap();
    let r = evaluate(l.clone(), p.clone(), EvaluationMode::Official, config());
    let mut sliced = start_evaluation(l, p, EvaluationMode::Official, config());
    for _ in 0..4 {
        assert_eq!(sliced.advance(NonZeroU64::MIN), EvaluationProgress::Pending);
    }
    let retried = loop {
        if let EvaluationProgress::Complete(result) = sliced.advance(NonZeroU64::new(100).unwrap())
        {
            break result;
        }
    };
    assert_eq!(retried, r);
    assert!(
        matches!(r.status, EvaluationStatus::ProgramRejected(_)),
        "{r:?}"
    );
    assert!(r.final_metrics.is_none());
}
#[test]
fn custom_generated_neg_requires_its_independent_permission() {
    use codegrid_ir::CustomDefinition;
    use codegrid_model::{AttachmentInstruction as A, Slot};
    let slot = Slot::new(0).unwrap();
    let forbidden = P::Neg.instruction_code().unwrap();
    let mut p = program(&[P::Read, P::Push, P::Direction(Direction::Right), P::Halt])
        .program()
        .clone();
    p.outer.main.cells[3].primary = Some(P::Custom(slot));
    p.customs.insert(
        slot,
        CustomDefinition {
            program: ScopedProgram {
                main: Board {
                    width: 5,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(P::Read, None),
                        Cell::instruction(P::Decode, None),
                        Cell::instruction(P::Direction(Direction::Right), Some(A::WriteCode)),
                        Cell::instruction(P::CustomReturn, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
        },
    );
    let p = VerifiedProgram::new(p).unwrap();
    let value = serde_json::json!({"format_version":1,"level_id":"generated","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":["READ","PUSH","DECODE","CUSTOM","CUSTOM_RETURN","HALT","MOVE_RIGHT"],"allowed_attachments":["WRITE_CODE"],"main_board":{"width":100,"height":1},"function_board":{"width":100,"height":1},"max_functions":10,"max_custom":10,"max_threads":1,"memory_enabled":true},"constraints":{},"scoring":{"metrics":{}},"evaluation":{"tests":[{"visible":true,"input":[forbidden],"expected_output":[]}]}});
    let l = load_level_json(value.to_string().as_bytes(), 100000).unwrap();
    let r = evaluate(l.clone(), p.clone(), EvaluationMode::Official, config());
    let mut sliced = start_evaluation(l, p, EvaluationMode::Official, config());
    for _ in 0..4 {
        assert_eq!(sliced.advance(NonZeroU64::MIN), EvaluationProgress::Pending);
    }
    let retried = loop {
        if let EvaluationProgress::Complete(result) = sliced.advance(NonZeroU64::new(100).unwrap())
        {
            break result;
        }
    };
    assert_eq!(retried, r);
    assert!(
        matches!(r.status, EvaluationStatus::ProgramRejected(_)),
        "{r:?}"
    );
    assert!(r.final_metrics.is_none());
}
#[test]
fn runtime_error_rolls_back_same_tick_output_and_keeps_stable_identity() {
    let l = level(
        serde_json::json!([{"visible":true,"input":[],"expected_output":[0]}]),
        serde_json::json!({}),
    );
    let r = evaluate(l, program(&[P::Output]), EvaluationMode::Official, config());
    assert_eq!(r.status, EvaluationStatus::RuntimeError);
    assert!(
        matches!(&r.visible_tests[0].outcome,TestOutcome::RuntimeError(codes) if !codes.is_empty())
    );
    assert!(r.final_metrics.is_none());
}
#[test]
fn exact_work_ceiling_allows_completed_test_and_cancellation_has_no_rating() {
    let l = level(
        serde_json::json!([{"visible":true,"input":[],"expected_output":[0]}]),
        serde_json::json!({}),
    );
    let p = program(&[P::Output, P::Halt]);
    let mut c = config();
    c.safety.cumulative_work = NonZeroU64::new(2).unwrap();
    let r = evaluate(l.clone(), p.clone(), EvaluationMode::Official, c);
    assert_eq!(r.status, EvaluationStatus::Passed);
    let mut session = start_evaluation(l, p, EvaluationMode::Official, config());
    session.cancel();
    let r = session.result().unwrap();
    assert_eq!(r.status, EvaluationStatus::Cancelled);
    assert!(r.rating.is_none());
}
#[test]
fn concurrent_equal_output_conflicts_cannot_pass() {
    let base = level(
        serde_json::json!([{"visible":true,"input":[],"expected_output":[0]}]),
        serde_json::json!({}),
    );
    let mut value = serde_json::json!({"format_version":1,"level_id":"conflict","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":["OUTPUT","HALT"],"allowed_attachments":[],"main_board":{"width":100,"height":2},"function_board":{"width":100,"height":1},"max_functions":10,"max_custom":10,"max_threads":2,"memory_enabled":true},"constraints":{},"scoring":{"metrics":{}},"evaluation":{"tests":[{"visible":true,"input":[],"expected_output":[0]}]}});
    value["evaluation"]["tests"][0]["expected_output"] =
        serde_json::json!(base.tests()[0].expected_output);
    let l = load_level_json(value.to_string().as_bytes(), 100000).unwrap();
    let mut p = program(&[P::Output, P::Halt]).program().clone();
    p.outer.main.height = 2;
    p.outer.main.cells.extend(p.outer.main.cells.clone());
    let r = evaluate(
        l,
        VerifiedProgram::new(p).unwrap(),
        EvaluationMode::Official,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::RuntimeError);
    assert!(
        matches!(&r.visible_tests[0].outcome,TestOutcome::RuntimeError(codes) if codes.iter().any(|c|c=="ConcurrentOutputConflict"))
    );
    assert!(r.visible_tests[0].actual_output.is_empty());
}

fn permissive_level(expected: &[u8], constraints: serde_json::Value) -> ValidatedLevel {
    let value = serde_json::json!({"format_version":1,"level_id":"metrics","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":instruction_identifiers(),"allowed_attachments":attachment_identifiers(),"main_board":{"width":100,"height":100},"function_board":{"width":100,"height":100},"max_functions":10,"max_custom":10,"max_threads":10,"memory_enabled":true},"constraints":constraints,"scoring":{"metrics":{}},"evaluation":{"tests":[{"visible":true,"input":[],"expected_output":expected}]}});
    load_level_json(value.to_string().as_bytes(), 100000).unwrap()
}
#[test]
fn function_folded_and_repeat_static_and_dynamic_metrics() {
    use codegrid_ir::FoldedBlock;
    use codegrid_model::{AttachmentInstruction as A, Slot};
    let slot = Slot::new(0).unwrap();
    let mut p = program(&[P::Direction(Direction::Right), P::Output, P::Halt])
        .program()
        .clone();
    p.outer.main.cells[1].primary = Some(P::Call(slot));
    p.outer.functions.insert(
        slot,
        Board {
            width: 3,
            height: 1,
            cells: vec![
                Cell::entry(Direction::Right),
                Cell::instruction(P::Add, None),
                Cell::instruction(P::Return, None),
            ],
            folded_blocks: BTreeMap::new(),
        },
    );
    let r = evaluate(
        permissive_level(&[0], serde_json::json!({})),
        VerifiedProgram::new(p).unwrap(),
        EvaluationMode::Official,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::Passed);
    let m = r.final_metrics.unwrap();
    assert_eq!(m["functions_used"], 1);
    assert_eq!(m["boards_used"], 2);
    assert_eq!(m["max_call_stack_depth"], 1);
    assert_eq!(m["non_empty_cells"], 7);
    let mut p = program(&[
        P::Direction(Direction::Right),
        P::Direction(Direction::Right),
        P::Output,
        P::Halt,
    ])
    .program()
    .clone();
    p.outer.main.cells[1].primary = Some(P::FoldedBlock(slot));
    p.outer.main.cells[2].primary = Some(P::FoldedBlock(slot));
    p.outer.main.folded_blocks.insert(
        slot,
        FoldedBlock {
            prefixes: Default::default(),
            cells: vec![
                Some(P::Add),
                Some(P::Direction(Direction::Right)),
                Some(P::Direction(Direction::Up)),
                None,
                None,
            ],
        },
    );
    let r = evaluate(
        permissive_level(&[2], serde_json::json!({})),
        VerifiedProgram::new(p).unwrap(),
        EvaluationMode::Official,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::Passed);
    let m = r.final_metrics.unwrap();
    assert_eq!(m["boards_used"], 2);
    assert_eq!(m["non_empty_cells"], 8);
    let mut p = program(&[P::Add, P::Output, P::Halt]).program().clone();
    p.outer.main.cells[1].attachment = Some(A::Repeat(3));
    let r = evaluate(
        permissive_level(&[3], serde_json::json!({})),
        VerifiedProgram::new(p).unwrap(),
        EvaluationMode::Official,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::Passed);
    let m = r.final_metrics.unwrap();
    assert_eq!(m["cost"], 4);
    assert_eq!(m["cost"], m["operation_count"]);
    assert_eq!(m["instruction_kinds"], 4);
}
#[test]
fn every_constraint_mapping_and_static_debug_behavior() {
    let pairs = [
        ("max_ticks", "ticks"),
        ("max_cost", "cost"),
        ("max_operation_count", "operation_count"),
        ("max_memory_addresses", "memory_addresses_used"),
        ("max_data_stack_depth", "max_data_stack_depth"),
        ("max_instruction_stack_depth", "max_instruction_stack_depth"),
        ("max_call_stack_depth", "max_call_stack_depth"),
        ("max_non_empty_cells", "non_empty_cells"),
        ("max_instruction_kinds", "instruction_kinds"),
        ("max_functions_used", "functions_used"),
        ("max_boards_used", "boards_used"),
    ];
    for (constraint, metric) in pairs {
        assert_eq!(metrics::constraint_metric(constraint), metric);
        assert!(metrics::constraints_exceeded(
            &BTreeMap::from([(metric.into(), 1)]),
            &BTreeMap::from([(constraint.into(), 0)])
        ));
        assert!(!metrics::constraints_exceeded(
            &BTreeMap::from([(metric.into(), 1)]),
            &BTreeMap::from([(constraint.into(), 1)])
        ));
    }
    let r = evaluate(
        permissive_level(&[0], serde_json::json!({"max_non_empty_cells":0})),
        program(&[P::Output, P::Halt]),
        EvaluationMode::Debug,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::ConstraintExceeded);
    assert_eq!(r.visible_tests.len(), 1);
    assert!(r.final_metrics.is_none());
    assert!(metrics::aggregate(
        &BTreeMap::from([("ticks".into(), u64::MAX)]),
        &BTreeMap::from([("ticks".into(), 1)])
    )
    .is_none());
}
#[test]
fn memory_and_stack_measurements_follow_vm_raw_metrics() {
    let p = program(&[
        P::Add,
        P::MemoryStore,
        P::MemoryLoad,
        P::Push,
        P::PopAdd,
        P::Output,
        P::Halt,
    ]);
    let r = evaluate(
        permissive_level(&[2], serde_json::json!({})),
        p,
        EvaluationMode::Official,
        config(),
    );
    assert_eq!(r.status, EvaluationStatus::Passed, "{r:?}");
    let m = r.final_metrics.unwrap();
    assert_eq!(m["memory_addresses_used"], 1);
    assert_eq!(m["max_data_stack_depth"], 2);
    assert_eq!(m["max_instruction_stack_depth"], 0);
}
#[test]
fn retained_state_and_output_ceilings_apply_at_initialization_and_completion() {
    let mut c = config();
    c.safety.max_state_units = NonZeroU64::MIN;
    let r = evaluate(
        permissive_level(&[0], serde_json::json!({})),
        program(&[P::Output, P::Halt]),
        EvaluationMode::Official,
        c,
    );
    assert_eq!(r.status, EvaluationStatus::ResourceLimitExceeded);
    assert!(r.visible_tests.is_empty());
    let mut c = config();
    c.safety.max_output_bytes = NonZeroU64::MIN;
    let r = evaluate(
        permissive_level(&[0, 0], serde_json::json!({})),
        program(&[P::Output, P::Output, P::Halt]),
        EvaluationMode::Official,
        c.clone(),
    );
    assert_eq!(r.status, EvaluationStatus::ResourceLimitExceeded);
    assert!(r.final_metrics.is_none());
    let r = evaluate(
        permissive_level(&[0, 1], serde_json::json!({})),
        program(&[P::Output, P::Output, P::Halt]),
        EvaluationMode::Official,
        c,
    );
    assert_eq!(r.status, EvaluationStatus::TestFailed);
    let mut c = config();
    c.safety.per_test_ticks = NonZeroU64::new(2).unwrap();
    let r = evaluate(
        permissive_level(&[0], serde_json::json!({})),
        program(&[P::Output, P::Halt]),
        EvaluationMode::Official,
        c,
    );
    assert_eq!(r.status, EvaluationStatus::Passed);
}

#[test]
fn feedback_reservation_limits_accumulation_before_retaining_next_test() {
    let l = level(
        serde_json::json!([{"visible":true,"input":[],"expected_output":[0]},{"visible":true,"input":[],"expected_output":[0]}]),
        serde_json::json!({}),
    );
    let mut c = config();
    c.safety.max_feedback_bytes =
        NonZeroU64::new(result_base_bound("test", "test").unwrap() + 268).unwrap();
    let r = evaluate(l, program(&[P::Output, P::Halt]), EvaluationMode::Debug, c);
    assert_eq!(r.status, EvaluationStatus::ResourceLimitExceeded);
    assert_eq!(r.visible_tests.len(), 1);
    assert!(r.final_metrics.is_none());
    assert!(result_representation_bound(&r).unwrap() <= r.config.safety.max_feedback_bytes.get());
}
