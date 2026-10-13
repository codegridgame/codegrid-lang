use codegrid_level_core::*;
use serde_json::json;
mod support;
use support::*;

#[test]
fn fixed_initial_instruction_can_be_rewritten_at_runtime() {
    use codegrid_ir::Cell;
    use codegrid_model::{AttachmentInstruction as A, Direction, PrimaryInstruction as P};
    let mut value = author("exact");
    value["evaluation"]["tests"] = json!([{"visible":true,"input":[96],"expected_output":[96]}]);
    value["program_rules"]["fixed_cells"] =
        json!([{"x":3,"y":0,"instruction":{"main":">","prefix":null,"suffix":"="}}]);
    let p = grid(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Read, None),
            Cell::instruction(P::Decode, None),
            Cell::instruction(P::Direction(Direction::Right), Some(A::WriteCode)),
            Cell::instruction(P::Output, None),
            Cell::instruction(P::Halt, None),
        ],
        6,
        1,
    );
    let result = run(session(&value, p, config(), limits()), 1);
    assert_eq!(result.status, EvaluationStatus::Passed, "{result:?}");
}

#[test]
fn main_fixed_cells_require_the_complete_instruction_and_reject_missing_cells() {
    let mut value = author("exact");
    value["evaluation"]["tests"] = json!([{"visible":true,"input":[],"expected_output":[]}]);
    value["program_rules"]["fixed_cells"] =
        json!([{"x":1,"y":0,"instruction":{"main":">","prefix":null,"suffix":null}}]);
    let good = run(session(&value, program(&[">", ";"]), config(), limits()), 1);
    assert_eq!(good.status, EvaluationStatus::Passed);
    let wrong = run(session(&value, program(&["v", ";"]), config(), limits()), 1);
    assert!(
        matches!(wrong.status, EvaluationStatus::ProgramRejected(ref r) if r.reason == "FixedCellMismatch")
    );
    value["program_rules"]["fixed_cells"][0]["x"] = json!(9);
    let missing = run(session(&value, program(&[">", ";"]), config(), limits()), 1);
    assert!(
        matches!(missing.status, EvaluationStatus::ProgramRejected(ref r) if r.reason == "FixedCellMismatch")
    );
}

#[test]
fn fixed_cell_validation_rejects_duplicates_out_of_bounds_and_obsolete_read_tokens() {
    let mut value = author("exact");
    let cell = json!({"x":1,"y":0,"instruction":{"main":",","prefix":"?!","suffix":"x2"}});
    value["program_rules"]["fixed_cells"] = json!([cell.clone()]);
    assert_eq!(load(&value).rules().fixed_cells.len(), 1);
    value["program_rules"]["fixed_cells"] = json!([cell.clone(), cell]);
    let error = load_scene_level_json(
        value.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 100000,
            max_tests: 32,
            max_total_test_bytes: 100000,
        },
    )
    .unwrap_err();
    assert_eq!(error.reason, "DuplicateFixedCell");
    value["program_rules"]["fixed_cells"] =
        json!([{"x":1,"y":0,"instruction":{"main":",>","prefix":null,"suffix":null}}]);
    let error = load_scene_level_json(
        value.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 100000,
            max_tests: 32,
            max_total_test_bytes: 100000,
        },
    )
    .unwrap_err();
    assert_eq!(error.reason, "InvalidFixedInstruction");
    value["program_rules"]["fixed_cells"][0]["instruction"]["main"] = json!(",");
    value["program_rules"]["fixed_cells"][0]["y"] = json!(2);
    assert!(load_scene_level_json(
        value.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 100000,
            max_tests: 32,
            max_total_test_bytes: 100000
        }
    )
    .is_err());
}
