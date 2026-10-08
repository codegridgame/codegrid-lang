use codegrid_ir::{Board, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION};
use codegrid_model::{AttachmentInstruction, ConditionPrefix, Direction, PrimaryInstruction, Slot};
use codegrid_vm::{BoundaryMode, RunOutcome, Vm, VmConfig, VmEvent, VmStatus};
use std::{collections::BTreeMap, num::NonZeroU64};

fn cell(token: &str) -> Cell {
    match token {
        "~>" => Cell::entry(Direction::Right),
        "_" => Cell::empty(),
        _ => Cell::instruction(PrimaryInstruction::from_token(token).unwrap(), None),
    }
}
fn board(tokens: &[&str]) -> Board {
    Board {
        width: tokens.len(),
        height: 1,
        cells: tokens.iter().map(|token| cell(token)).collect(),
        folded_blocks: BTreeMap::new(),
    }
}
fn machine(main: Board, functions: Vec<Board>, input: &[u8]) -> Vm {
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main,
            functions: functions
                .into_iter()
                .enumerate()
                .map(|(id, board)| (Slot::new(id as u8).unwrap(), board))
                .collect(),
        },
        customs: BTreeMap::new(),
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
fn neg_all_bytes_and_double_negation_use_selected_register_and_step_run_parity() {
    for value in 0..=255u8 {
        let main = board(&["~>", "}", ",", "$!", ".", "$!", ".", ";"]);
        let mut stepped = machine(main.clone(), vec![], &[value]);
        let mut bounded = machine(main, vec![], &[value]);
        while stepped.status() == VmStatus::Running {
            stepped.step();
        }
        assert_eq!(bounded.run(20), RunOutcome::Halted);
        assert_eq!(stepped.snapshot(), bounded.snapshot());
        let state = bounded.snapshot();
        assert_eq!(state.output, [value.wrapping_neg(), value]);
        assert_eq!(state.registers[0], 0);
        assert_eq!(state.registers[1], value);
        assert_eq!(state.threads[0].register_pointer, 1);
    }
}

#[test]
fn neg_repeat_rechecks_conditions_and_counts_each_execution() {
    for (prefix, count, value, expected) in [
        (None, 2, 37, 37),
        (None, 3, 37, 219),
        (Some(ConditionPrefix::One), 3, 1, 255),
    ] {
        let mut main = board(&["~>", ",", "$!", ".", ";"]);
        main.cells[2].prefix = prefix;
        main.cells[2].attachment = Some(AttachmentInstruction::Repeat(count));
        let mut vm = machine(main, vec![], &[value]);
        assert_eq!(vm.run(20), RunOutcome::Halted);
        assert_eq!(vm.snapshot().output, [expected]);
    }
}

#[test]
fn neg_code_round_trip_and_stack_subtraction() {
    let mut vm = machine(
        board(&["~>", ",", "(", ",", "$!", ")", ".", ",", "&", "%", ".", ";"]),
        vec![],
        &[10, 3, 69],
    );
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [7, 69]);
    let mut wrapping = machine(
        board(&["~>", ",", "(", ",", "$!", ")", ".", ";"]),
        vec![],
        &[3, 10],
    );
    assert_eq!(wrapping.run(20), RunOutcome::Halted);
    assert_eq!(wrapping.snapshot().output, [249]);
}

#[test]
fn nested_functions_copy_all_registers_restore_pointer_and_share_thread_stack() {
    let mut tokens = vec!["~>"];
    for _ in 0..9 {
        tokens.extend([",", "}"]);
    }
    tokens.extend([",", "[0", ".", ")", ".", ";"]);
    let mut vm = machine(
        board(&tokens),
        vec![
            board(&["~>", "+", "[1", ".", "(", "}", "!", "]"]),
            board(&["~>", "$!", ".", "]"]),
        ],
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    );
    for _ in 0..21 {
        vm.step();
    }
    let entered = vm.snapshot();
    assert_eq!(
        entered.threads[0].private_registers,
        Some([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
    );
    assert_eq!(entered.threads[0].register_pointer, 9);
    assert_eq!(vm.run(40), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.output, [245, 11, 10, 21]);
    assert_eq!(state.registers, [1, 2, 3, 4, 5, 6, 7, 8, 9, 21]);
    assert_eq!(state.threads[0].register_pointer, 9);
    assert_eq!(state.threads[0].private_registers, None);
    assert!(state.threads[0].data_stack.is_empty());
}

#[test]
fn private_banks_do_not_conflict_with_each_other_or_main_and_return_preserves_sibling_writes() {
    let mut main = board(&[
        "~>", "[0", "_", "_", "_", "_", "_", ";", "~>", "[0", "_", "_", "_", "_", "_", ";", "~>",
        "_", "+", "+", "_", "_", "_", ";",
    ]);
    main.width = 8;
    main.height = 3;
    let mut vm = machine(main, vec![board(&["~>", "+", "+", "]"])], &[]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 2);
    assert!(state
        .threads
        .iter()
        .all(|thread| thread.private_registers.is_none()));
}

#[test]
fn private_state_and_shared_output_conflict_roll_back_atomically() {
    let mut main = board(&[
        "~>", "[0", "_", "_", "_", "_", ";", "~>", "_", "_", "_", ".", "_", ";",
    ]);
    main.width = 7;
    main.height = 2;
    let mut function = board(&["~>", "+", "$!", "]"]);
    function.cells[2] = Cell::instruction(PrimaryInstruction::Output, None);
    let mut vm = machine(main, vec![function], &[]);
    for _ in 0..4 {
        assert_eq!(vm.step().status, VmStatus::Running);
    }
    let before = vm.snapshot();
    let failed = vm.step();
    assert_eq!(failed.status, VmStatus::Error);
    assert!(failed.events.is_empty());
    assert_eq!(vm.snapshot().threads, before.threads);
    assert_eq!(vm.snapshot().output, before.output);
}

#[test]
fn self_tail_call_keeps_current_private_values_and_original_caller() {
    let mut vm = machine(
        board(&["~>", ",", "[0", ".", ";"]),
        vec![board(&["~>", "-", "[0", "]"])],
        &[5],
    );
    assert_eq!(vm.run(12), RunOutcome::TickLimitReached);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 5);
    assert_eq!(state.threads[0].private_registers.unwrap()[0], 2);
    assert_eq!(state.threads[0].call_stack.len(), 1);
    assert_eq!(state.threads[0].call_stack[0].saved_registers, None);
    assert_eq!(state.threads[0].call_stack[0].saved_register_pointer, 0);
}

#[test]
fn work_yield_restores_function_state_and_private_changes_have_thread_events() {
    let mut vm = machine(
        board(&["~>", ",", "[0", ".", ";"]),
        vec![board(&["~>", "$!", "]"])],
        &[5],
    );
    for _ in 0..4 {
        vm.step();
    }
    let before = vm.snapshot();
    let result = vm.run_with_work_limit(10, NonZeroU64::new(1).unwrap());
    assert!(result.is_err());
    assert_eq!(vm.snapshot().threads[0].private_registers.unwrap()[0], 251);
    let current = vm.snapshot();
    assert_eq!(vm.step().status, VmStatus::Running);
    assert_eq!(vm.snapshot().registers, before.registers);
    assert_eq!(current.threads[0].call_stack[0].saved_register_pointer, 0);
    assert_eq!(vm.run(10), RunOutcome::Halted);
    let mut events = machine(
        board(&["~>", ",", "[0", ".", ";"]),
        vec![board(&["~>", "$!", "]"])],
        &[5],
    );
    let result = events.run_with_work_limit_detailed(20, NonZeroU64::new(100).unwrap());
    assert!(result.events.iter().any(|event| matches!(event, VmEvent::ThreadChanged { before, after, .. } if before.private_registers != after.private_registers && after.private_registers.is_some())));
}
