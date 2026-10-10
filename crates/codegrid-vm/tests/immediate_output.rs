use codegrid_ir::{
    Board, Cell, CustomDefinition, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
};
use codegrid_model::{Direction, PrimaryInstruction, Slot};
use codegrid_vm::{RunOutcome, Vm, VmConfig, VmStatus};
use std::{collections::BTreeMap, num::NonZeroU64};

fn instruction(token: &str) -> Cell {
    Cell::instruction(PrimaryInstruction::from_token(token).unwrap(), None)
}

fn repeated_digit(digit: u8, count: u8) -> Cell {
    Cell::instruction(
        PrimaryInstruction::OutputImmediate(codegrid_model::ImmediateDigit::new(digit).unwrap()),
        Some(codegrid_model::AttachmentInstruction::Repeat(count)),
    )
}

#[test]
fn immediate_repeat_emits_once_per_tick_and_matches_bounded_execution() {
    for digit in 0..10 {
        for count in 2..=5 {
            let main = board(
                vec![
                    Cell::entry(Direction::Right),
                    repeated_digit(digit, count),
                    instruction(";"),
                ],
                3,
            );
            let mut stepped = machine(main.clone(), None, &[]);
            let mut bounded = machine(main, None, &[]);
            stepped.step();
            for completed in 1..=count {
                stepped.step();
                let state = stepped.snapshot();
                assert_eq!(state.output, vec![digit; usize::from(completed)]);
                assert_eq!(state.registers, [0; 10]);
                assert_eq!(state.threads[0].register_pointer, 0);
                assert_eq!(state.metrics.operation_count(), u64::from(completed));
                assert_eq!(stepped.committed_ticks(), u64::from(completed) + 1);
            }
            stepped.step();
            assert_eq!(bounded.run(20), RunOutcome::Halted);
            assert_eq!(stepped.snapshot(), bounded.snapshot());
        }
    }
}

#[test]
fn repeated_immediate_output_conflict_rolls_back_only_the_conflicting_tick() {
    let main = board(
        vec![
            Cell::entry(Direction::Right),
            repeated_digit(3, 3),
            instruction(";"),
            Cell::entry(Direction::Right),
            Cell::empty(),
            instruction("."),
        ],
        3,
    );
    let mut vm = machine(main, None, &[]);
    vm.step();
    vm.step();
    assert_eq!(vm.snapshot().output, [3]);
    assert_eq!(vm.step().status, VmStatus::Error);
    assert_eq!(vm.snapshot().output, [3]);
    assert_eq!(vm.committed_ticks(), 2);
}

#[test]
fn repeated_custom_immediate_output_pushes_each_value_to_the_caller() {
    let outer = board(
        vec![
            Cell::entry(Direction::Right),
            instruction("#0"),
            instruction(")"),
            instruction("."),
            instruction(")"),
            instruction("."),
            instruction(")"),
            instruction("."),
            instruction(";"),
        ],
        9,
    );
    let custom = board(
        vec![
            Cell::entry(Direction::Right),
            repeated_digit(9, 3),
            instruction("#]"),
        ],
        3,
    );
    let mut vm = machine(outer, Some(custom), &[]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    // POP adds each stacked byte to the selected register.
    assert_eq!(vm.snapshot().output, [9, 18, 27]);
}
fn board(cells: Vec<Cell>, width: usize) -> Board {
    Board {
        width,
        height: cells.len() / width,
        cells,
        folded_blocks: BTreeMap::new(),
    }
}
fn machine(main: Board, custom: Option<Board>, input: &[u8]) -> Vm {
    let customs = custom
        .map(|main| {
            BTreeMap::from([(
                Slot::new(0).unwrap(),
                CustomDefinition {
                    program: ScopedProgram {
                        main,
                        functions: BTreeMap::new(),
                    },
                },
            )])
        })
        .unwrap_or_default();
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main,
            functions: BTreeMap::new(),
        },
        customs,
    })
    .unwrap();
    Vm::new(
        program,
        input.iter().copied(),
        VmConfig::new(0, NonZeroU64::new(100).unwrap()),
    )
    .unwrap()
}

#[test]
fn all_digits_preserve_selected_register_pointer_and_share_output_metrics() {
    let mut cells = vec![
        Cell::entry(Direction::Right),
        instruction("}"),
        instruction(","),
    ];
    for digit in 0..10 {
        cells.push(instruction(&format!(".{digit}")));
    }
    cells.extend([instruction("."), instruction(";")]);
    let mut vm = machine(board(cells, 15), None, &[77]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.output, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 77]);
    assert_eq!(state.registers[1], 77);
    assert_eq!(state.registers[0], 0);
    assert_eq!(state.threads[0].register_pointer, 1);
    assert_eq!(state.metrics.operation_count(), 14);
    assert_eq!(state.metrics.instruction_variety().len(), 4);
    assert!(state
        .metrics
        .instruction_variety()
        .contains(&codegrid_vm::InstructionKind::Output));
}

#[test]
fn immediate_and_register_outputs_conflict_and_roll_back() {
    let main = board(
        vec![
            Cell::entry(Direction::Right),
            instruction(".3"),
            Cell::entry(Direction::Right),
            instruction("."),
        ],
        2,
    );
    let mut vm = machine(main, None, &[]);
    vm.step();
    let result = vm.step();
    assert_eq!(result.status, VmStatus::Error);
    assert!(vm.snapshot().output.is_empty());
    assert_eq!(vm.committed_ticks(), 1);
}

#[test]
fn custom_immediate_output_pushes_to_caller_stack() {
    let outer = board(
        vec![
            Cell::entry(Direction::Right),
            instruction("+"),
            instruction("#0"),
            instruction(")"),
            instruction("."),
            instruction(";"),
        ],
        6,
    );
    let custom = board(
        vec![
            Cell::entry(Direction::Right),
            instruction(".9"),
            instruction("#]"),
        ],
        3,
    );
    let mut vm = machine(outer, Some(custom), &[]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [10]);
    assert_eq!(vm.snapshot().registers[0], 10);
}

#[test]
fn appended_input_preserves_unread_tail_and_execution_state() {
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                instruction("."),
                instruction(","),
                instruction("."),
                instruction(";"),
            ],
            6,
        ),
        None,
        &[9],
    );
    vm.step();
    let before = vm.snapshot();
    vm.append_input(&[8, 7]).unwrap();
    vm.append_input(&[]).unwrap();
    let after = vm.snapshot();
    assert_eq!(after.input, [9, 8, 7]);
    assert_eq!(after.registers, before.registers);
    assert_eq!(after.threads, before.threads);
    assert_eq!(after.metrics, before.metrics);
    assert_eq!(after.runtime_program, before.runtime_program);
    assert_eq!(vm.run(10), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [9, 8]);
    assert_eq!(vm.snapshot().input, [7]);
    let terminal = vm.snapshot();
    assert_eq!(
        vm.append_input(&[6]),
        Err(codegrid_vm::InputAppendError::Terminal)
    );
    assert_eq!(vm.snapshot(), terminal);
}

#[test]
fn append_after_interrupted_tick_uses_the_rolled_back_boundary() {
    let outer = board(
        vec![
            Cell::entry(Direction::Right),
            instruction("#0"),
            instruction(","),
            instruction("."),
            instruction(";"),
        ],
        5,
    );
    let custom = board(
        vec![
            Cell::entry(Direction::Right),
            instruction(".9"),
            instruction("#]"),
        ],
        3,
    );
    let mut vm = machine(outer, Some(custom), &[8]);
    vm.step();
    let before = vm.snapshot();
    let (result, work) = vm.step_with_work_accounting(NonZeroU64::new(1).unwrap());
    assert!(result.is_err());
    assert_eq!(work, 1);
    assert_eq!(vm.snapshot(), before);
    vm.append_input(&[7]).unwrap();
    assert_eq!(vm.snapshot().input, [8, 7]);
    assert_eq!(vm.snapshot().metrics, before.metrics);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [8]);
    assert_eq!(vm.snapshot().input, [7]);
}
