use std::{collections::BTreeMap, num::NonZeroU64};

use codegrid_ir::{
    Board, Cell, CustomDefinition, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
};
use codegrid_model::{Direction, PrimaryInstruction};
use codegrid_vm::{
    MemoryAccessObservation, MemoryAccessSpace, ThreadSnapshot, Vm, VmConfig, VmSnapshot,
    VmSnapshotView, VmStatus,
};
use num_bigint::BigInt;

fn machine(cells: Vec<Cell>, width: usize) -> Vm {
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width,
                height: 1,
                cells,
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .expect("transaction test program must pass IR verification");
    Vm::new(program, [], VmConfig::new(0))
        .expect("verified transaction test program must initialize")
}

fn output_machine() -> Vm {
    machine(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(
                PrimaryInstruction::OutputImmediate(
                    codegrid_model::ImmediateDigit::new(7).expect("7 is a valid digit"),
                ),
                None,
            ),
        ],
        2,
    )
}

fn snapshot_from_view(view: &VmSnapshotView<'_>) -> VmSnapshot {
    VmSnapshot {
        status: view.status(),
        committed_steps: view.committed_steps(),
        registers: *view.registers(),
        memory: view.memory().clone(),
        input: view.input().copied().collect(),
        output: view.output().to_vec(),
        runtime_program: view.runtime_program().clone(),
        threads: view
            .threads()
            .map(|thread| ThreadSnapshot {
                id: thread.id(),
                board: thread.board(),
                position: thread.position(),
                direction: thread.direction(),
                register_pointer: thread.register_pointer(),
                status_flag: thread.status_flag(),
                private_registers: thread.private_registers().copied(),
                page: thread.page().clone(),
                data_stack: thread.data_stack().to_vec(),
                instruction_stack: thread.instruction_stack().to_vec(),
                call_stack: thread.call_frames().collect(),
                phase: thread.phase(),
                random_state: thread.random_state(),
            })
            .collect(),
        metrics: view.metrics().clone(),
        errors: view.errors().to_vec(),
        fault: view.fault(),
    }
}

fn custom_machine() -> Vm {
    let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: 2,
                height: 1,
                cells: vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                ],
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::from([(
            custom_id,
            CustomDefinition {
                program: ScopedProgram {
                    main: Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::CustomReturn, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                    functions: BTreeMap::new(),
                },
            },
        )]),
    })
    .expect("verified custom transaction test program must initialize");
    Vm::new(program, [], VmConfig::new(0))
        .expect("verified custom transaction test program must initialize")
}

#[test]
fn prepared_output_is_published_only_by_commit() {
    let mut vm = output_machine();
    vm.step();
    let before_output = vm.snapshot();

    let pending = vm.prepare_step();
    assert_eq!(pending.report().candidate_status, VmStatus::Running);
    assert_eq!(
        pending.report().footprint.operations[&codegrid_vm::InstructionKind::Output],
        1
    );
    assert_eq!(pending.report().work_units, 1);
    assert_eq!(pending.report().custom_internal_dispatches, 0);
    assert!(pending
        .report()
        .footprint
        .code_cells
        .iter()
        .any(|cell| { cell.position.x == 1 && cell.position.y == 0 }));

    let committed = pending.commit();
    assert_eq!(committed.newly_emitted_output, [7]);
    assert!(!committed.events.is_empty());
    assert_eq!(vm.snapshot().output, [7]);
    assert_eq!(vm.committed_steps(), before_output.committed_steps + 1);
}

#[test]
fn candidate_snapshot_view_matches_the_committed_vm_view() {
    let mut vm = output_machine();
    vm.step();

    let pending = vm.prepare_step();
    let candidate = snapshot_from_view(&pending.candidate_snapshot_view());
    assert_eq!(candidate.output, [7]);

    pending.commit();
    assert_eq!(snapshot_from_view(&vm.snapshot_view()), candidate);
}

#[test]
fn abort_restores_the_snapshot_view_before_the_candidate() {
    let mut vm = output_machine();
    vm.step();
    let before = snapshot_from_view(&vm.snapshot_view());

    let pending = vm.prepare_step();
    let candidate = snapshot_from_view(&pending.candidate_snapshot_view());
    assert_eq!(candidate.output, [7]);
    assert_ne!(candidate, before);

    pending.abort();
    assert_eq!(snapshot_from_view(&vm.snapshot_view()), before);
    assert_eq!(vm.snapshot(), before);
}

#[test]
fn abort_discards_staged_state_events_and_output() {
    let mut vm = output_machine();
    vm.step();
    let before = vm.snapshot();

    let pending = vm.prepare_step();
    let report = pending.abort();

    assert_eq!(report.candidate_status, VmStatus::Running);
    assert_eq!(report.work_units, 1);
    assert_eq!(
        report.footprint.operations[&codegrid_vm::InstructionKind::Output],
        1
    );
    assert_eq!(vm.snapshot(), before);

    let committed = vm.prepare_step().commit();
    assert_eq!(committed.newly_emitted_output, [7]);
    assert_eq!(vm.snapshot().output, [7]);
}

#[test]
fn candidate_footprint_contains_only_the_current_transition() {
    let mut vm = output_machine();

    let first = vm.prepare_step();
    assert!(first.report().footprint.operations.is_empty());
    assert!(first
        .report()
        .footprint
        .code_cells
        .iter()
        .any(|cell| { cell.position.x == 0 && cell.position.y == 0 }));
    first.commit();

    let second = vm.prepare_step();
    assert_eq!(
        second.report().footprint.operations[&codegrid_vm::InstructionKind::Output],
        1
    );
    assert!(second
        .report()
        .footprint
        .code_cells
        .iter()
        .all(|cell| { cell.position.x == 1 && cell.position.y == 0 }));
    second.abort();
}

#[test]
fn candidate_footprint_reports_exact_outer_memory_access_and_stack_peak() {
    let mut vm = machine(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::MemoryLoad, None),
        ],
        2,
    );
    vm.step();

    let pending = vm.prepare_step();
    let footprint = &pending.report().footprint;

    assert_eq!(
        footprint.operations[&codegrid_vm::InstructionKind::MemoryLoad],
        1
    );
    assert_eq!(
        footprint.memory_accesses,
        [MemoryAccessObservation {
            space: MemoryAccessSpace::Outer,
            address: BigInt::from(0),
        }]
        .into_iter()
        .collect()
    );
    assert_eq!(footprint.stack_peaks.data, 1);
    pending.abort();
}

#[test]
fn work_interruption_returns_no_pending_transition_and_leaves_state_unchanged() {
    let mut vm = machine(
        vec![Cell::entry(Direction::Right), Cell::entry(Direction::Left)],
        2,
    );
    let before = vm.snapshot();

    let work_limit = NonZeroU64::new(1).expect("one is positive");
    let interrupted = match vm.prepare_step_with_work_limit(work_limit) {
        Ok(_) => panic!("the second dispatch must exceed the work ceiling"),
        Err(interrupted) => interrupted,
    };

    assert_eq!(interrupted.maximum_work_units, work_limit.get());
    assert_eq!(vm.snapshot(), before);
    vm.replace_input(&[7])
        .expect("an interrupted preparation leaves a completed boundary");
    assert_eq!(vm.snapshot().input, [7]);
}

#[test]
fn candidate_report_counts_outer_and_custom_dispatches_without_persisting_them() {
    let mut vm = custom_machine();
    vm.step(); // Move the outer thread from its entry to the Custom call.
    let metrics_before = vm.snapshot().metrics;

    let pending = vm
        .prepare_step_with_work_limit(NonZeroU64::new(3).expect("three is positive"))
        .expect("outer call and two Custom dispatches fit exactly");
    assert_eq!(pending.report().work_units, 3);
    assert_eq!(pending.report().custom_internal_dispatches, 2);

    let aborted = pending.abort();
    assert_eq!(aborted.work_units, 3);
    assert_eq!(aborted.custom_internal_dispatches, 2);
    assert_eq!(vm.snapshot().metrics, metrics_before);

    let retried = vm.prepare_step();
    assert_eq!(retried.report().custom_internal_dispatches, 2);
    retried.commit();
    assert_eq!(vm.committed_steps(), metrics_before.committed_steps() + 1);

    let next_candidate = vm.prepare_step();
    assert_eq!(next_candidate.report().custom_internal_dispatches, 0);
    next_candidate.abort();
}

#[test]
fn candidate_custom_dispatch_count_sums_multiple_invocations() {
    let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
    let program = VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width: 4,
                height: 1,
                cells: vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                    Cell::entry(Direction::Right),
                    Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                ],
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::from([(
            custom_id,
            CustomDefinition {
                program: ScopedProgram {
                    main: Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::CustomReturn, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                    functions: BTreeMap::new(),
                },
            },
        )]),
    })
    .expect("multiple Custom transaction program must pass IR verification");
    let mut vm =
        Vm::new(program, [], VmConfig::new(0)).expect("verified Custom program must initialize");
    vm.step();

    let pending = vm.prepare_step();
    assert_eq!(pending.report().work_units, 6);
    assert_eq!(pending.report().custom_internal_dispatches, 4);
    pending.abort();
}

#[test]
fn dropping_a_pending_step_aborts_it() {
    let mut vm = output_machine();
    vm.step();
    let before = vm.snapshot();

    drop(vm.prepare_step());

    assert_eq!(vm.snapshot(), before);
}
