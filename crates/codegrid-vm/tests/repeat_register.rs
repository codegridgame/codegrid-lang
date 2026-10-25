use std::collections::BTreeMap;

use codegrid_ir::{
    Board, Cell, FoldedBlock, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
};
use codegrid_model::{
    AttachmentInstruction, ConditionPrefix, Direction, ImmediateDigit, PrimaryInstruction, Slot,
};
use codegrid_vm::{InstructionKind, RunOutcome, ThreadPhaseSnapshot, Vm, VmConfig};

fn primary(token: &str) -> PrimaryInstruction {
    PrimaryInstruction::from_token(token).expect("token is a Full Primary")
}

fn instruction(token: &str) -> Cell {
    Cell::instruction(primary(token), None)
}

fn repeat_register(token: &str) -> Cell {
    Cell::instruction(primary(token), Some(AttachmentInstruction::RepeatRegister))
}

fn board(cells: Vec<Cell>, folded_blocks: BTreeMap<Slot, FoldedBlock>) -> Board {
    Board {
        width: cells.len(),
        height: 1,
        cells,
        folded_blocks,
    }
}

fn machine(main: Board, input: &[u8]) -> Vm {
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main,
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .expect("test program is valid Full IR");
    Vm::new(program, input.iter().copied(), VmConfig::new(0)).unwrap()
}

#[test]
fn repeat_register_prefix_is_checked_before_zero_count_and_primary() {
    let mut repeated = repeat_register("+");
    repeated.prefix = Some(ConditionPrefix::Zero);
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                repeated,
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::new(),
        ),
        &[],
    );
    vm.step();

    let candidate = vm.prepare_step();
    let operations = &candidate.report().footprint.operations;
    assert_eq!(operations.get(&InstructionKind::Condition), Some(&1));
    assert!(!operations.contains_key(&InstructionKind::Add));
    candidate.commit();

    let state = vm.snapshot();
    assert_eq!(state.registers[0], 0);
    assert_eq!(state.threads[0].position.x, 2);
    assert_eq!(vm.run(10), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [0]);
}

#[test]
fn failed_prefix_skips_primary_operands_and_does_not_read_repeat_operand() {
    let mut repeated = repeat_register("$(");
    repeated.prefix = Some(ConditionPrefix::One);
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                repeated,
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::new(),
        ),
        &[2],
    );
    vm.step();
    vm.step();

    let candidate = vm.prepare_step();
    let operations = &candidate.report().footprint.operations;
    assert_eq!(operations.get(&InstructionKind::Condition), Some(&1));
    assert!(!operations.contains_key(&InstructionKind::MemoryLoad));
    candidate.commit();

    let state = vm.snapshot();
    assert_eq!(state.registers[0], 2);
    assert_eq!(state.threads[0].position.x, 3);
    assert_eq!(state.threads[0].phase, ThreadPhaseSnapshot::Normal);
    assert_eq!(vm.run(10), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [2]);
}

#[test]
fn failed_folded_prefix_skips_primary_and_repeat_operands() {
    let slot = Slot::new(0).unwrap();
    let folded_with_prefix_and_repeat = FoldedBlock {
        prefixes: BTreeMap::from([(0, ConditionPrefix::One)]),
        attachments: BTreeMap::from([(0, AttachmentInstruction::RepeatRegister)]),
        cells: vec![Some(primary("$(")), None, None, None, None],
    };
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                instruction("$0"),
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::from([(slot, folded_with_prefix_and_repeat)]),
        ),
        &[2],
    );
    vm.step();
    vm.step();

    let candidate = vm.prepare_step();
    let operations = &candidate.report().footprint.operations;
    assert_eq!(operations.get(&InstructionKind::Condition), Some(&1));
    assert!(!operations.contains_key(&InstructionKind::MemoryLoad));
    candidate.commit();

    assert_eq!(vm.run(10), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 2);
    assert_eq!(state.output, [2]);
}

#[test]
fn repeat_register_captures_the_first_selected_byte_and_counts_each_execution() {
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                repeat_register("+"),
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::new(),
        ),
        &[3],
    );
    vm.step();
    vm.step();
    vm.step();
    assert_eq!(vm.snapshot().registers[0], 4);
    assert_eq!(
        vm.snapshot().threads[0].phase,
        ThreadPhaseSnapshot::Repeat {
            total: 3,
            completed: 1
        }
    );
    vm.step();
    assert_eq!(vm.snapshot().registers[0], 5);
    assert_eq!(
        vm.snapshot().threads[0].phase,
        ThreadPhaseSnapshot::Repeat {
            total: 3,
            completed: 2
        }
    );
    assert_eq!(vm.run(10), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 6);
    assert_eq!(state.output, [6]);
    assert_eq!(state.committed_steps, 7);
}

#[test]
fn repeat_register_supports_one_and_maximum_byte_counts() {
    for (count, expected) in [(1, 2), (u8::MAX, 254)] {
        let mut vm = machine(
            board(
                vec![
                    Cell::entry(Direction::Right),
                    instruction(","),
                    repeat_register("+"),
                    instruction("."),
                    instruction(";"),
                ],
                BTreeMap::new(),
            ),
            &[count],
        );
        assert_eq!(vm.run(300), RunOutcome::Halted);
        let state = vm.snapshot();
        assert_eq!(state.registers[0], expected);
        assert_eq!(state.output, [expected]);
        assert_eq!(state.committed_steps, u64::from(count) + 4);
    }
}

#[test]
fn repeat_register_rechecks_prefix_before_each_repeated_primary() {
    let mut repeated = repeat_register("+");
    repeated.prefix = Some(ConditionPrefix::Two);
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                repeated,
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::new(),
        ),
        &[2],
    );
    assert_eq!(vm.run(10), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 3);
    assert_eq!(state.output, [3]);
    assert_eq!(state.committed_steps, 6);
}

#[test]
fn repeat_register_applies_to_immediate_output() {
    let immediate = PrimaryInstruction::OutputImmediate(ImmediateDigit::new(5).unwrap());
    let mut vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                Cell::instruction(immediate, Some(AttachmentInstruction::RepeatRegister)),
                instruction(";"),
            ],
            BTreeMap::new(),
        ),
        &[2],
    );
    assert_eq!(vm.run(10), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [5, 5]);
    assert_eq!(vm.snapshot().registers[0], 2);
}

#[test]
fn repeat_register_applies_inside_folded_blocks_and_to_whole_invocations() {
    let slot = Slot::new(0).unwrap();
    let folded_with_repeat = FoldedBlock {
        prefixes: BTreeMap::new(),
        attachments: BTreeMap::from([(0, AttachmentInstruction::RepeatRegister)]),
        cells: vec![Some(primary("+")), None, None, None, None],
    };
    let mut inner_repeat_vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                instruction("$0"),
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::from([(slot, folded_with_repeat)]),
        ),
        &[3],
    );
    assert_eq!(inner_repeat_vm.run(20), RunOutcome::Halted);
    assert_eq!(inner_repeat_vm.snapshot().registers[0], 6);
    assert_eq!(inner_repeat_vm.snapshot().output, [6]);

    let folded_once = FoldedBlock {
        prefixes: BTreeMap::new(),
        attachments: BTreeMap::new(),
        cells: vec![Some(primary("+")), None, None, None, None],
    };
    let mut whole_invocation_vm = machine(
        board(
            vec![
                Cell::entry(Direction::Right),
                instruction(","),
                repeat_register("$0"),
                instruction("."),
                instruction(";"),
            ],
            BTreeMap::from([(slot, folded_once)]),
        ),
        &[2],
    );
    assert_eq!(whole_invocation_vm.run(20), RunOutcome::Halted);
    assert_eq!(whole_invocation_vm.snapshot().registers[0], 4);
    assert_eq!(whole_invocation_vm.snapshot().output, [4]);
}
