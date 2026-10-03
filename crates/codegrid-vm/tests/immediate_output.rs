use codegrid_ir::{
    Board, Cell, CustomDefinition, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
};
use codegrid_model::{BoundaryMode, Direction, PrimaryInstruction, Slot};
use codegrid_vm::{RunOutcome, Vm, VmConfig, VmStatus};
use std::{collections::BTreeMap, num::NonZeroU64};

fn instruction(token: &str) -> Cell {
    Cell::instruction(PrimaryInstruction::from_token(token).unwrap(), None)
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
        VmConfig::new(BoundaryMode::Exit, 0, NonZeroU64::new(100).unwrap()),
    )
    .unwrap()
}

#[test]
fn all_digits_preserve_selected_register_pointer_and_share_output_metrics() {
    let mut cells = vec![
        Cell::entry(Direction::Right),
        instruction("}"),
        instruction(",>"),
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
