use codegrid_ir::{Board, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION};
use codegrid_model::{
    AttachmentInstruction, ConditionPrefix, Direction, PrimaryInstruction as P, Slot,
};
use codegrid_vm::{BoundaryMode, RunOutcome, Vm, VmConfig, VmStatus};
use std::{collections::BTreeMap, num::NonZeroU64};

fn board(tokens: &[&str]) -> Board {
    Board {
        width: tokens.len(),
        height: 1,
        folded_blocks: BTreeMap::new(),
        cells: tokens
            .iter()
            .map(|token| {
                if *token == "~>" {
                    return Cell::entry(Direction::Right);
                }
                let (prefix, rest) = if let Some(rest) = token.strip_prefix("?!") {
                    (Some(ConditionPrefix::Flag), rest)
                } else {
                    (None, *token)
                };
                let mut cell = Cell::instruction(P::from_token(rest).unwrap(), None);
                cell.prefix = prefix;
                cell
            })
            .collect(),
    }
}
fn machine(main: Board, functions: Vec<Board>, customs: Vec<Board>, input: &[u8]) -> Vm {
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main,
            functions: functions
                .into_iter()
                .enumerate()
                .map(|(i, b)| (Slot::new(i as u8).unwrap(), b))
                .collect(),
        },
        customs: customs
            .into_iter()
            .enumerate()
            .map(|(i, main)| {
                (
                    Slot::new(i as u8).unwrap(),
                    codegrid_ir::CustomDefinition {
                        program: ScopedProgram {
                            main,
                            functions: BTreeMap::new(),
                        },
                    },
                )
            })
            .collect(),
    })
    .unwrap();
    Vm::new(
        program,
        input.iter().copied(),
        VmConfig::new(BoundaryMode::Exit, 0, NonZeroU64::new(100).unwrap()),
    )
    .unwrap()
}

#[test]
fn byte_producers_overwrite_flag_for_every_byte_and_run_matches_step() {
    for token in ["+", "-", "$<", "$>"] {
        for value in 0..=255u8 {
            let main = board(&["~>", ",", token, ";"]);
            let mut vm = machine(main.clone(), vec![], vec![], &[value]);
            let mut stepped = machine(main, vec![], vec![], &[value]);
            while stepped.status() == VmStatus::Running {
                stepped.step();
            }
            assert_eq!(vm.run(20), RunOutcome::Halted);
            assert_eq!(vm.snapshot(), stepped.snapshot());
            let expected = match token {
                "+" => u8::from(value == 255),
                "-" => u8::from(value == 0),
                "$<" => value >> 7,
                _ => value & 1,
            };
            assert_eq!(
                vm.snapshot().threads[0].status_flag,
                expected,
                "{token} {value}"
            );
        }
    }
}

#[test]
fn read_last_byte_succeeds_exhaustion_preserves_value_direction_and_test_does_not_clear() {
    let mut vm = machine(
        board(&["~>", ",", "?!.9", ",", "?!.1", "?!.2", ";"]),
        vec![],
        vec![],
        &[173],
    );
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.output, [1, 2]);
    assert_eq!(state.registers[0], 173);
    assert_eq!(state.threads[0].direction, Direction::Right);
    assert_eq!(state.threads[0].status_flag, 1);
}

#[test]
fn popadd_empty_carry_and_no_carry_and_nand_preservation() {
    for (input, expected) in [(0, 0), (127, 0), (128, 1), (255, 1)] {
        let mut vm = machine(board(&["~>", ",", "(", ")", ";"]), vec![], vec![], &[input]);
        vm.run(20);
        assert_eq!(vm.snapshot().threads[0].status_flag, expected);
        assert_eq!(vm.snapshot().registers[0], input.wrapping_mul(2));
    }
    for token in ["$&", "$!", "?=", "!", ".", "("] {
        let mut vm = machine(board(&["~>", ")", token, ";"]), vec![], vec![], &[]);
        vm.run(20);
        assert_eq!(vm.snapshot().threads[0].status_flag, 1, "{token}");
    }
}

#[test]
fn pointer_wraps_and_repeat_rechecks_flag() {
    let mut main = board(&["~>", "{", "?!}", "?!.1", ";"]);
    let mut vm = machine(main.clone(), vec![], vec![], &[]);
    vm.run(20);
    assert_eq!(vm.snapshot().output, [1]);
    assert_eq!(vm.snapshot().threads[0].register_pointer, 0);
    main = board(&["~>", ")", "?!+", "?!.1", ";"]);
    main.cells[2].attachment = Some(AttachmentInstruction::Repeat(5));
    let mut vm = machine(main, vec![], vec![], &[]);
    vm.run(20);
    assert_eq!(vm.snapshot().registers[0], 1);
    assert!(vm.snapshot().output.is_empty());
    assert_eq!(vm.snapshot().threads[0].status_flag, 0);
}

#[test]
fn functions_copy_and_restore_flags_custom_copies_and_discards() {
    let mut vm = machine(
        board(&["~>", ")", "[0", "?!.1", "#0", "?!.2", ";"]),
        vec![board(&["~>", "?!.3", "+", "]"])],
        vec![board(&["~>", "?!.4", ",", "?!.", "#]"])],
        &[],
    );
    vm.run(30);
    let state = vm.snapshot();
    assert_eq!(state.output, [3, 1, 2]);
    assert_eq!(state.threads[0].status_flag, 1);
    // Custom reads the 4 it just pushed to the caller stack, then clears F.
    assert!(state.threads[0].data_stack.is_empty());
}

#[test]
fn rejected_tick_rolls_back_flag_and_register_and_new_instances_start_zero() {
    let mut vm = machine(board(&["~>", "-"]), vec![], vec![], &[]);
    assert_eq!(vm.snapshot().threads[0].status_flag, 0);
    vm.step();
    vm.step();
    assert_eq!(vm.snapshot().threads[0].status_flag, 0);
    assert_eq!(vm.snapshot().registers[0], 0);
}

#[test]
fn empty_concurrent_reads_set_independent_flags_and_work_yield_restores_both() {
    let mut main = board(&["~>", ",", ";", "~>", ",", ";"]);
    main.width = 3;
    main.height = 2;
    let mut vm = machine(main, vec![], vec![], &[]);
    vm.step();
    let before = vm.snapshot();
    assert!(vm
        .step_with_work_limit(NonZeroU64::new(1).unwrap())
        .is_err());
    assert_eq!(vm.snapshot(), before);
    vm.step();
    assert_eq!(
        vm.snapshot()
            .threads
            .iter()
            .map(|t| t.status_flag)
            .collect::<Vec<_>>(),
        [1, 1]
    );
}

#[test]
fn register_conflict_rolls_back_flags_even_when_values_and_flags_are_equal() {
    let mut main = board(&["~>", "-", ";", "~>", "-", ";"]);
    main.width = 3;
    main.height = 2;
    let mut vm = machine(main, vec![], vec![], &[]);
    vm.step();
    vm.step();
    let snapshot = vm.snapshot();
    assert_eq!(snapshot.status, VmStatus::Error);
    assert_eq!(snapshot.registers[0], 0);
    assert!(snapshot.threads.iter().all(|t| t.status_flag == 0));
}

#[test]
fn nested_calls_restore_each_suspended_flag_and_tail_reuse_retains_original_caller() {
    let mut vm = machine(
        board(&["~>", ")", "[0", "?!.1", ";"]),
        vec![
            board(&["~>", "+", "[1", "?!.9", "]"]),
            board(&["~>", ")", "?!.3", "]"]),
        ],
        vec![],
        &[],
    );
    vm.run(40);
    assert_eq!(vm.snapshot().output, [3, 1]);
    let mut vm = machine(
        board(&["~>", ")", "[0", ";"]),
        vec![board(&["~>", "-", "[0", "]"])],
        vec![],
        &[],
    );
    vm.run(20);
    let state = vm.snapshot();
    assert_eq!(state.threads[0].call_stack.len(), 1);
    assert_eq!(state.threads[0].call_stack[0].saved_status_flag, 1);
    assert_eq!(state.threads[0].status_flag, 0);
}

#[test]
fn mutable_read_code_preserves_flag_prefix_and_uses_current_flag_on_revisit() {
    let mut main = board(&["~>", ",", ")", "&", "?!!", "<"]);
    main.cells[4].attachment = Some(AttachmentInstruction::WriteCode);
    let mut vm = machine(main, vec![], vec![], &[44, 9]);
    for _ in 0..5 {
        vm.step();
    }
    assert_eq!(
        vm.snapshot().runtime_program.main.cells[4].primary,
        Some(P::Read)
    );
    vm.step();
    vm.step();
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 9);
    assert_eq!(state.threads[0].status_flag, 0);
    assert_eq!(
        state.runtime_program.main.cells[4].prefix,
        Some(ConditionPrefix::Flag)
    );
}
