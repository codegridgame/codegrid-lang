use codegrid_level_core::*;
use serde_json::json;
mod support;
use support::*;

#[test]
fn direction_variants_share_one_initial_program_inventory() {
    let mut v = author("exact");
    v["program_rules"]["instruction_limits"] = json!({"DIRECTION":1});
    for direction in [">", "<", "^", "v"] {
        assert!(validate_program_rules(load(&v).rules(), &program(&[direction, ";"])).is_ok());
    }
    let error = validate_program_rules(load(&v).rules(), &program(&[">", "v", ";"])).unwrap_err();
    assert_eq!(error.reason, "InstructionCountExceeded");
    assert_eq!(error.path, "$.program_rules.instruction_limits.DIRECTION");
}

#[test]
fn pointer_permissions_and_two_item_inventory_are_independent() {
    let mut v = author("exact");
    v["program_rules"]["allowed_instructions"] = json!(["POINTER_LEFT"]);
    v["program_rules"]["instruction_limits"] = json!({"REGISTER_POINTER":2});
    assert!(validate_program_rules(load(&v).rules(), &program(&["{", "{", ";"])).is_ok());
    assert_eq!(
        validate_program_rules(load(&v).rules(), &program(&["{", "{", "{", ";"]))
            .unwrap_err()
            .reason,
        "InstructionCountExceeded"
    );
    assert_eq!(
        validate_program_rules(load(&v).rules(), &program(&["}", ";"]))
            .unwrap_err()
            .reason,
        "InstructionNotAllowed"
    );
}

#[test]
fn unused_functions_and_folded_definitions_share_inventory_with_main() {
    use codegrid_ir::{Board, Cell, FoldedBlock, VerifiedProgram};
    use codegrid_model::{Direction, PrimaryInstruction as P, Slot};
    use std::collections::BTreeMap;
    let mut v = author("exact");
    v["program_rules"]["instruction_limits"] = json!({"DIRECTION":1});
    let mut p = program(&[">", ";"]).program().clone();
    p.outer.functions.insert(
        Slot::new(0).unwrap(),
        Board {
            width: 1,
            height: 1,
            cells: vec![Cell::instruction(P::Direction(Direction::Left), None)],
            folded_blocks: BTreeMap::new(),
        },
    );
    assert_eq!(
        validate_program_rules(load(&v).rules(), &VerifiedProgram::new(p.clone()).unwrap())
            .unwrap_err()
            .reason,
        "InstructionCountExceeded"
    );
    p.outer.functions.clear();
    p.outer.main.folded_blocks.insert(
        Slot::new(0).unwrap(),
        FoldedBlock {
            prefixes: BTreeMap::new(),
            cells: vec![Some(P::Direction(Direction::Left)), None, None],
        },
    );
    assert_eq!(
        validate_program_rules(load(&v).rules(), &VerifiedProgram::new(p).unwrap())
            .unwrap_err()
            .reason,
        "InstructionCountExceeded"
    );
}

#[test]
fn runtime_writes_can_increase_direction_count() {
    use codegrid_ir::Cell;
    use codegrid_model::{AttachmentInstruction as A, Direction, PrimaryInstruction as P};
    let mut v = author("exact");
    v["program_rules"]["instruction_limits"] = json!({"DIRECTION":1});
    v["evaluation"]["tests"] = json!([{"visible":true,"input":[62],"expected_output":[]}]);
    let p = grid(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Read, None),
            Cell::instruction(P::Decode, None),
            Cell::instruction(P::Clear, Some(A::WriteCode)),
            Cell::instruction(P::Direction(Direction::Right), None),
            Cell::instruction(P::Halt, None),
        ],
        6,
        1,
    );
    assert_eq!(
        run(session(&v, p, config(), limits()), 1).status,
        EvaluationStatus::Passed
    );
}

#[test]
fn limit_loader_rejects_unknown_types_and_invalid_counts() {
    let mut v = author("exact");
    for limits in [
        json!({"NOT_A_TYPE":1}),
        json!({"DIRECTION":-1}),
        json!({"DIRECTION":1.5}),
        json!({"DIRECTION":null}),
        json!([]),
    ] {
        v["program_rules"]["instruction_limits"] = limits;
        assert!(load_scene_level_json(
            v.to_string().as_bytes(),
            LoadLimits {
                max_level_bytes: 100000,
                max_tests: 32,
                max_total_test_bytes: 100000
            }
        )
        .is_err());
    }
}
