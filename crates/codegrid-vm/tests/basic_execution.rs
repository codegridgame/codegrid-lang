use std::collections::BTreeMap;

use codegrid_ir::{
    Board, BoardId, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
};
use codegrid_model::{BoundaryMode, Direction, PrimaryInstruction, ShiftDirection};
use codegrid_vm::{InstructionKind, RunOutcome, RuntimeErrorKind, Vm, VmConfig, VmStatus};

fn verified(cells: Vec<Cell>, width: usize, height: usize) -> VerifiedProgram {
    VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width,
                height,
                cells,
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .expect("Full test program must pass IR verification")
}

fn vm(cells: Vec<Cell>, width: usize, height: usize, input: &[u8], boundary: BoundaryMode) -> Vm {
    Vm::new(
        verified(cells, width, height),
        input.iter().copied(),
        VmConfig::new(boundary, 0, std::num::NonZeroU64::MIN),
    )
    .expect("a verified Full program must initialize")
}

#[test]
fn starts_with_ten_zero_registers_and_halt_commits_without_moving() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Add, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        3,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.snapshot().registers, [0; 10]);

    assert_eq!(vm.step().status, VmStatus::Running);
    assert_eq!(vm.snapshot().threads[0].position.x, 1);
    assert_eq!(vm.step().status, VmStatus::Running);
    assert_eq!(vm.snapshot().registers[0], 1);

    let halt = vm.step();
    assert_eq!(halt.status, VmStatus::Halted);
    assert_eq!(halt.committed_ticks, 3);
    assert_eq!(vm.snapshot().threads[0].position.x, 2);
    assert_eq!(vm.run(2), RunOutcome::Halted);
    assert_eq!(vm.snapshot().committed_ticks, 3);
}

#[test]
fn arithmetic_wraps_and_operation_variety_matches_the_full_inventory() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Sub, None),
            Cell::instruction(PrimaryInstruction::Add, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        4,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.run(8), RunOutcome::Halted);

    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 0);
    assert_eq!(snapshot.metrics.operation_count(), 3);
    assert_eq!(snapshot.metrics.used_cell_count(), 4);
    assert_eq!(
        snapshot.metrics.instruction_variety(),
        &[
            InstructionKind::Add,
            InstructionKind::Sub,
            InstructionKind::Halt
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn zero_test_changes_direction_only_when_the_register_is_zero() {
    let mut zero_case = vm(
        vec![
            Cell::entry(Direction::Right),
            {
                let mut cell =
                    Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None);
                cell.prefix = Some(codegrid_model::ConditionPrefix::Zero);
                cell
            },
            Cell::empty(),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        2,
        2,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(zero_case.run(8), RunOutcome::Halted);
    assert_eq!(zero_case.snapshot().threads[0].direction, Direction::Down);
    assert_eq!(zero_case.snapshot().threads[0].position.y, 1);

    let mut nonzero_case = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Add, None),
            {
                let mut cell =
                    Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None);
                cell.prefix = Some(codegrid_model::ConditionPrefix::Zero);
                cell
            },
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        4,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(nonzero_case.run(8), RunOutcome::Halted);
    assert_eq!(nonzero_case.snapshot().registers[0], 1);
    assert_eq!(
        nonzero_case.snapshot().threads[0].direction,
        Direction::Right
    );
}

#[test]
fn successful_read_consumes_one_byte_and_keeps_the_existing_direction() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Read, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        3,
        1,
        &[173],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.run(8), RunOutcome::Halted);

    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 173);
    assert!(snapshot.input.is_empty());
    assert_eq!(snapshot.threads[0].direction, Direction::Right);
    assert_eq!(snapshot.threads[0].position.x, 2);
}

#[test]
fn output_appends_the_register_value_and_reports_the_step_delta() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Add, None),
            Cell::instruction(PrimaryInstruction::Output, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        4,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.step().status, VmStatus::Running);
    assert_eq!(vm.step().status, VmStatus::Running);
    let output = vm.step();
    assert_eq!(output.newly_emitted_output, [1]);
    assert_eq!(vm.snapshot().output, [1]);
}

#[test]
fn exhausted_read_sets_flag_without_turning_or_an_error() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Read, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        4,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.run(8), RunOutcome::Halted);

    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 0);
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 2, y: 0 }
    );
    assert_eq!(snapshot.threads[0].direction, Direction::Right);
    assert!(snapshot.errors.is_empty());
    assert_eq!(snapshot.threads[0].status_flag, 1);
}

#[test]
fn exhausted_read_preserves_a_previously_read_value_and_keeps_direction() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Read, None),
            Cell::instruction(PrimaryInstruction::Read, None),
            Cell::instruction(PrimaryInstruction::Output, None),
            Cell::instruction(PrimaryInstruction::Halt, None),
            Cell::instruction(PrimaryInstruction::Direction(Direction::Right), None),
        ],
        6,
        1,
        &[173],
        BoundaryMode::Wrap,
    );

    assert_eq!(vm.run(8), RunOutcome::Halted);

    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 173);
    assert!(snapshot.input.is_empty());
    assert_eq!(snapshot.output, [173]);
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 4, y: 0 }
    );
    assert_eq!(snapshot.threads[0].direction, Direction::Right);
    assert_eq!(snapshot.committed_ticks, 5);
}

#[test]
fn shifts_are_eight_bit_left_and_logical_right() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Read, None),
            Cell::instruction(PrimaryInstruction::Shift(ShiftDirection::Left), None),
            Cell::instruction(PrimaryInstruction::Shift(ShiftDirection::Right), None),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        5,
        1,
        &[0x81],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.run(8), RunOutcome::Halted);
    assert_eq!(vm.snapshot().registers[0], 1);
}

#[test]
fn right_shift_zero_fills_the_high_bit_and_discards_the_low_bit() {
    for (input, expected) in [(0x80_u8, 0x40_u8), (0x01_u8, 0x00_u8)] {
        let mut machine = vm(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Shift(ShiftDirection::Right), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
            1,
            &[input],
            BoundaryMode::Exit,
        );

        assert_eq!(machine.run(8), RunOutcome::Halted);
        assert_eq!(machine.snapshot().registers[0], expected);
    }
}

#[test]
fn failed_exit_tick_rolls_back_state_but_keeps_attempt_metrics() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Add, None),
        ],
        2,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.step().status, VmStatus::Running);
    let failed = vm.step();

    assert_eq!(failed.status, VmStatus::Error);
    assert_eq!(failed.attempted_tick, 2);
    assert_eq!(failed.committed_ticks, 1);
    assert_eq!(failed.errors[0].code(), "OutOfBounds");
    assert_eq!(
        failed.errors[0].kind(),
        &RuntimeErrorKind::OutOfBounds {
            thread_id: 0,
            board: BoardId::Main,
            position: codegrid_vm::Coordinate { x: 1, y: 0 },
            direction: Direction::Right,
        }
    );
    assert!(failed.events.is_empty());
    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 0);
    assert_eq!(snapshot.threads[0].position.x, 1);
    assert_eq!(snapshot.committed_ticks, 1);
    assert_eq!(snapshot.metrics.operation_count(), 1);
    assert_eq!(snapshot.metrics.used_cell_count(), 2);
}

#[test]
fn failed_exit_tick_rolls_back_direction_change_and_reports_attempt_context() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None),
        ],
        2,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.step().status, VmStatus::Running);

    let failed = vm.step();

    assert_eq!(failed.status, VmStatus::Error);
    assert_eq!(failed.attempted_tick, 2);
    assert_eq!(failed.committed_ticks, 1);
    assert_eq!(
        failed.errors[0].kind(),
        &RuntimeErrorKind::OutOfBounds {
            thread_id: 0,
            board: BoardId::Main,
            position: codegrid_vm::Coordinate { x: 1, y: 0 },
            direction: Direction::Down,
        }
    );
    let snapshot = vm.snapshot();
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 1, y: 0 }
    );
    assert_eq!(snapshot.threads[0].direction, Direction::Right);
    assert_eq!(snapshot.committed_ticks, 1);
}

#[test]
fn failed_exit_read_rolls_back_register_and_input_consumption() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Read, None),
        ],
        2,
        1,
        &[173],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.step().status, VmStatus::Running);

    let failed = vm.step();

    assert_eq!(failed.status, VmStatus::Error);
    assert_eq!(failed.attempted_tick, 2);
    assert_eq!(failed.committed_ticks, 1);
    assert_eq!(
        failed.errors[0].kind(),
        &RuntimeErrorKind::OutOfBounds {
            thread_id: 0,
            board: BoardId::Main,
            position: codegrid_vm::Coordinate { x: 1, y: 0 },
            direction: Direction::Right,
        }
    );
    let snapshot = vm.snapshot();
    assert_eq!(snapshot.registers[0], 0);
    assert_eq!(snapshot.input, [173]);
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 1, y: 0 }
    );
    assert_eq!(snapshot.threads[0].direction, Direction::Right);
    assert_eq!(snapshot.metrics.operation_count(), 1);
    assert_eq!(snapshot.metrics.used_cell_count(), 2);
}

#[test]
fn failed_exit_output_rolls_back_only_the_failed_tick_output() {
    let mut vm = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Output, None),
            Cell::instruction(PrimaryInstruction::Add, None),
            Cell::instruction(PrimaryInstruction::Output, None),
        ],
        4,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(vm.run(3), RunOutcome::TickLimitReached);
    assert_eq!(vm.snapshot().output, [0]);

    let failed = vm.step();

    assert_eq!(failed.status, VmStatus::Error);
    assert_eq!(failed.attempted_tick, 4);
    assert_eq!(failed.committed_ticks, 3);
    assert!(failed.newly_emitted_output.is_empty());
    assert_eq!(
        failed.errors[0].kind(),
        &RuntimeErrorKind::OutOfBounds {
            thread_id: 0,
            board: BoardId::Main,
            position: codegrid_vm::Coordinate { x: 3, y: 0 },
            direction: Direction::Right,
        }
    );
    let snapshot = vm.snapshot();
    assert_eq!(snapshot.output, [0]);
    assert_eq!(snapshot.registers[0], 1);
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 3, y: 0 }
    );
    assert_eq!(snapshot.metrics.operation_count(), 3);
}

#[test]
fn step_is_a_no_op_after_halt_or_error() {
    let mut halted = vm(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Halt, None),
        ],
        2,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(halted.run(4), RunOutcome::Halted);
    let halted_before = halted.snapshot();
    let halted_step = halted.step();
    assert_eq!(halted_step.status, VmStatus::Halted);
    assert!(halted_step.events.is_empty());
    assert!(halted_step.newly_emitted_output.is_empty());
    assert_eq!(halted.snapshot(), halted_before);

    let mut errored = vm(
        vec![Cell::entry(Direction::Right)],
        1,
        1,
        &[],
        BoundaryMode::Exit,
    );
    assert_eq!(errored.step().status, VmStatus::Error);
    let error_before = errored.snapshot();
    let error_step = errored.step();
    assert_eq!(error_step.status, VmStatus::Error);
    assert_eq!(error_step.errors, error_before.errors);
    assert!(error_step.events.is_empty());
    assert!(error_step.newly_emitted_output.is_empty());
    assert_eq!(errored.snapshot(), error_before);
}

#[test]
fn zero_tick_limit_is_invalid_and_a_later_run_continues_after_tick_limit() {
    let mut vm = vm(
        vec![Cell::entry(Direction::Right)],
        1,
        1,
        &[],
        BoundaryMode::Wrap,
    );
    assert_eq!(vm.run(0), RunOutcome::InvalidTickLimit);
    assert_eq!(vm.snapshot().committed_ticks, 0);

    assert_eq!(vm.run(1), RunOutcome::TickLimitReached);
    assert_eq!(vm.snapshot().committed_ticks, 1);
    assert_eq!(vm.run(2), RunOutcome::TickLimitReached);

    let snapshot = vm.snapshot();
    assert_eq!(snapshot.status, VmStatus::Running);
    assert_eq!(snapshot.committed_ticks, 3);
    assert_eq!(
        snapshot.threads[0].position,
        codegrid_vm::Coordinate { x: 0, y: 0 }
    );
}
