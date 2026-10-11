use codegrid_ir::{Board, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION};
use codegrid_model::{AttachmentInstruction, Direction, PrimaryInstruction};
use codegrid_vm::{RunOutcome, RuntimeErrorKind, Vm, VmConfig, VmStatus};
use std::{collections::BTreeMap, num::NonZeroU64};

fn machine(tokens: &[&str], limit: u64) -> Vm {
    let cells = tokens
        .iter()
        .map(|token| {
            if *token == "entry" {
                Cell::entry(Direction::Right)
            } else if *token == "empty" {
                Cell::empty()
            } else {
                Cell::instruction(PrimaryInstruction::from_token(token).unwrap(), None)
            }
        })
        .collect();
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: tokens.len(),
                height: 1,
                cells,
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .unwrap();
    Vm::new(
        program,
        [],
        VmConfig::new(0, NonZeroU64::MIN).with_gas_hard_limit(NonZeroU64::new(limit).unwrap()),
    )
    .unwrap()
}

#[test]
fn representative_programs_freeze_execution_prices_and_breakdown() {
    for (tokens, execution, stack) in [
        (vec!["entry", "+", ".", ";"], 8, 0),
        (vec!["entry", ">", "}", ";"], 3, 0),
        (vec!["entry", "(", ")", ";"], 6, 1),
        (vec!["entry", ",", ";"], 5, 0),
    ] {
        let mut vm = machine(&tokens, 1000);
        assert_eq!(vm.run(20), RunOutcome::Halted);
        let metrics = vm.snapshot().metrics;
        assert_eq!(metrics.execution_gas(), execution, "{tokens:?}");
        assert_eq!(metrics.stack_gas(), stack, "{tokens:?}");
        assert_eq!(metrics.gas_used(), execution + stack);
        assert_eq!(metrics.gas_schedule_version(), 1);
    }
}

#[test]
fn exact_limit_commits_but_excess_rolls_back_with_attempted_gas() {
    let mut exact = machine(&["+", ".", ";"], 8);
    assert_eq!(exact.run(10), RunOutcome::Halted);
    let mut exceeded = machine(&["+", ".", ";"], 7);
    exceeded.step();
    let before = exceeded.snapshot();
    let result = exceeded.step();
    assert_eq!(result.status, VmStatus::Error);
    assert_eq!(
        result.errors[0].kind(),
        &RuntimeErrorKind::GasLimitExceeded {
            limit: 7,
            attempted_gas: 8
        }
    );
    assert_eq!(exceeded.snapshot().registers, before.registers);
    assert_eq!(exceeded.snapshot().threads, before.threads);
    assert!(exceeded.snapshot().output.is_empty());
    assert_eq!(exceeded.committed_ticks(), 1);
    assert_eq!(result.metrics.gas_used(), 8);
    assert_eq!(exceeded.step().metrics.gas_used(), 8);
}

#[test]
fn step_run_and_resumed_execution_have_identical_gas() {
    let tokens = ["entry", "+", "(", ")", ".", ";"];
    let mut stepped = machine(&tokens, 1000);
    let mut run = machine(&tokens, 1000);
    let mut resumed = machine(&tokens, 1000);
    while stepped.status() == VmStatus::Running {
        stepped.step();
    }
    assert_eq!(run.run(20), RunOutcome::Halted);
    while resumed.status() == VmStatus::Running {
        resumed.run(1);
    }
    assert_eq!(stepped.snapshot(), run.snapshot());
    assert_eq!(stepped.snapshot(), resumed.snapshot());
}

#[test]
fn shared_memory_is_cold_once_across_ticks_and_empty_store_is_not_an_access() {
    let mut vm = machine(&["entry", "$(", "$(", "$)", ";"], 1000);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let metrics = vm.snapshot().metrics;
    assert_eq!(metrics.execution_gas(), 18);
    assert_eq!(metrics.memory_gas(), 10);
    assert_eq!(metrics.gas_used(), 30);
    let mut fresh = machine(&["$(", ";"], 1000);
    fresh.run(10);
    assert_eq!(fresh.snapshot().metrics.gas_used(), 16);
    let mut empty_store = machine(&["$)", ";"], 1000);
    empty_store.run(10);
    assert_eq!(empty_store.snapshot().metrics.gas_used(), 8);
    assert_eq!(empty_store.snapshot().metrics.memory_gas(), 0);
}

#[test]
fn repeat_is_not_a_separate_charge() {
    let mut vm = machine(&["+", ";"], 1000);
    // Build another verified program because repeat belongs to a cell attachment.
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: 2,
                height: 1,
                cells: vec![
                    Cell::instruction(
                        PrimaryInstruction::Add,
                        Some(AttachmentInstruction::Repeat(3)),
                    ),
                    Cell::instruction(PrimaryInstruction::Halt, None),
                ],
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .unwrap();
    let mut repeated = Vm::new(program, [], vm.config()).unwrap();
    repeated.run(10);
    assert_eq!(repeated.snapshot().metrics.gas_used(), 9);
    vm.run(10);
    assert_eq!(vm.snapshot().metrics.gas_used(), 3);
}

#[test]
fn skipped_body_charges_only_the_condition() {
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: 2,
                height: 1,
                cells: vec![
                    Cell::instruction(PrimaryInstruction::Add, None)
                        .with_prefix(codegrid_model::ConditionPrefix::Flag),
                    Cell::instruction(PrimaryInstruction::Halt, None),
                ],
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .unwrap();
    let mut vm = Vm::new(program, [], VmConfig::new(0, NonZeroU64::MIN)).unwrap();
    vm.run(10);
    assert_eq!(vm.snapshot().metrics.gas_used(), 2);
    assert_eq!(vm.snapshot().registers[0], 0);
}
