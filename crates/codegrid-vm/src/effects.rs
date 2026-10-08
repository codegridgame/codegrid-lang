use std::collections::BTreeMap;

use codegrid_model::{PrimaryInstruction, Value};

use crate::{ExecutionScope, MemoryLocationId, RuntimeError, RuntimeErrorKind, StaticCellId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RegisterWrite {
    pub thread_id: u64,
    pub register: u8,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct InputRead {
    pub thread_id: u64,
    pub register: u8,
    pub value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OutputWrite {
    pub thread_id: u64,
    pub value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MemoryWrite {
    pub thread_id: u64,
    pub location: MemoryLocationId,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CodeWrite {
    pub thread_id: u64,
    pub cell: StaticCellId,
    pub primary: Option<PrimaryInstruction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallerStackRead {
    pub thread_id: u64,
    pub register: u8,
    pub value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallerStackWrite {
    pub thread_id: u64,
    pub value: Value,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SharedEffectBatch {
    pub private_register_threads: std::collections::BTreeSet<u64>,
    pub register_writes: Vec<RegisterWrite>,
    pub input_reads: Vec<InputRead>,
    pub output_writes: Vec<OutputWrite>,
    pub memory_writes: Vec<MemoryWrite>,
    pub code_writes: Vec<CodeWrite>,
    pub caller_stack_reads: Vec<CallerStackRead>,
    pub caller_stack_writes: Vec<CallerStackWrite>,
}

/// Resolves conflicts for one synchronous execution-context tick.
///
/// This function only diagnoses conflicts; the caller commits the effects
/// atomically after all local and shared errors have been collected.
pub(super) fn resolve_shared_conflicts(
    effects: &SharedEffectBatch,
    input_was_nonempty: bool,
    caller_stack_was_nonempty: bool,
    global_tick: u64,
    scope: ExecutionScope,
) -> Vec<RuntimeError> {
    let mut errors = Vec::new();
    let mut rejected_readers = Vec::new();

    if input_was_nonempty && effects.input_reads.len() > 1 {
        let thread_ids = effects
            .input_reads
            .iter()
            .map(|read| read.thread_id)
            .collect();
        rejected_readers.extend(effects.input_reads.iter().map(|read| read.thread_id));
        errors.push(RuntimeError::new(
            global_tick,
            scope,
            RuntimeErrorKind::ConcurrentInputConflict { thread_ids },
        ));
    }

    let mut rejected_caller_readers = Vec::new();
    let mut caller_read_conflict = false;
    if caller_stack_was_nonempty && effects.caller_stack_reads.len() > 1 {
        caller_read_conflict = true;
        let internal_thread_ids = effects
            .caller_stack_reads
            .iter()
            .map(|read| read.thread_id)
            .collect();
        rejected_caller_readers
            .extend(effects.caller_stack_reads.iter().map(|read| read.thread_id));
        errors.push(RuntimeError::new(
            global_tick,
            scope,
            RuntimeErrorKind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            },
        ));
    }

    let caller_stack_read = if caller_stack_was_nonempty
        && effects.caller_stack_reads.len() == 1
        && effects.caller_stack_reads[0].value.is_some()
    {
        Some(effects.caller_stack_reads[0])
    } else {
        None
    };
    let caller_stack_write_conflict = effects.caller_stack_writes.len() > 1;
    if caller_stack_write_conflict {
        errors.push(RuntimeError::new(
            global_tick,
            scope,
            RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids: effects
                    .caller_stack_writes
                    .iter()
                    .map(|write| write.thread_id)
                    .collect(),
            },
        ));
    }

    let caller_stack_read_write_conflict =
        caller_stack_read.is_some() && !effects.caller_stack_writes.is_empty();
    if caller_stack_read_write_conflict {
        if let Some(read) = caller_stack_read {
            let mut internal_thread_ids = vec![read.thread_id];
            internal_thread_ids.extend(
                effects
                    .caller_stack_writes
                    .iter()
                    .map(|write| write.thread_id),
            );
            errors.push(RuntimeError::new(
                global_tick,
                scope,
                RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                    internal_thread_ids,
                },
            ));
        }
    }

    let successful_input_read = if input_was_nonempty
        && effects.input_reads.len() == 1
        && effects.input_reads[0].value.is_some()
    {
        Some(effects.input_reads[0])
    } else {
        None
    };

    let mut register_writers: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    for write in &effects.register_writes {
        if effects.private_register_threads.contains(&write.thread_id) {
            continue;
        }
        register_writers
            .entry(write.register)
            .or_default()
            .push(write.thread_id);
    }
    if let Some(read) = successful_input_read {
        if !effects.private_register_threads.contains(&read.thread_id) {
            register_writers
                .entry(read.register)
                .or_default()
                .push(read.thread_id);
        }
    }
    if !caller_read_conflict && !caller_stack_read_write_conflict {
        if let Some(read) = caller_stack_read {
            if !effects.private_register_threads.contains(&read.thread_id) {
                register_writers
                    .entry(read.register)
                    .or_default()
                    .push(read.thread_id);
            }
        }
    }
    for (register, thread_ids) in register_writers {
        let mut thread_ids = without_rejected(&thread_ids, &rejected_readers);
        thread_ids = without_rejected(&thread_ids, &rejected_caller_readers);
        if thread_ids.len() > 1 {
            errors.push(RuntimeError::new(
                global_tick,
                scope,
                RuntimeErrorKind::ConcurrentWriteConflict {
                    register,
                    thread_ids,
                },
            ));
        }
    }

    if effects.output_writes.len() > 1 {
        errors.push(RuntimeError::new(
            global_tick,
            scope,
            RuntimeErrorKind::ConcurrentOutputConflict {
                thread_ids: effects
                    .output_writes
                    .iter()
                    .map(|write| write.thread_id)
                    .collect(),
            },
        ));
    }

    let mut memory_writers: BTreeMap<&MemoryLocationId, Vec<u64>> = BTreeMap::new();
    for write in &effects.memory_writes {
        memory_writers
            .entry(&write.location)
            .or_default()
            .push(write.thread_id);
    }
    for (location, thread_ids) in memory_writers {
        if thread_ids.len() > 1 {
            errors.push(RuntimeError::new(
                global_tick,
                scope,
                RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                    address: location.address.clone(),
                    thread_ids,
                },
            ));
        }
    }

    let mut code_writers: BTreeMap<StaticCellId, Vec<u64>> = BTreeMap::new();
    for write in &effects.code_writes {
        code_writers
            .entry(write.cell)
            .or_default()
            .push(write.thread_id);
    }
    for (cell, thread_ids) in code_writers {
        if thread_ids.len() > 1 {
            errors.push(RuntimeError::new(
                global_tick,
                scope,
                RuntimeErrorKind::ConcurrentCodeWriteConflict { cell, thread_ids },
            ));
        }
    }

    RuntimeError::sort_canonical(&mut errors);
    errors
}

fn without_rejected(thread_ids: &[u64], rejected: &[u64]) -> Vec<u64> {
    thread_ids
        .iter()
        .copied()
        .filter(|id| !rejected.contains(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_shared_conflicts, CallerStackRead, CallerStackWrite, CodeWrite, InputRead,
        MemoryWrite, OutputWrite, RegisterWrite, SharedEffectBatch,
    };
    use crate::{Coordinate, ExecutionScope, MemoryLocationId, MemorySpaceId};
    use codegrid_ir::{BoardId, CodeGridId};
    use codegrid_model::{PrimaryInstruction, Slot};
    use num_bigint::BigInt;

    #[test]
    fn rejected_input_reads_do_not_create_register_conflicts() {
        let effects = SharedEffectBatch {
            register_writes: vec![RegisterWrite {
                thread_id: 4,
                register: 0,
                value: 1,
            }],
            input_reads: vec![
                InputRead {
                    thread_id: 1,
                    register: 0,
                    value: Some(7),
                },
                InputRead {
                    thread_id: 2,
                    register: 0,
                    value: Some(7),
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(&effects, true, false, 1, ExecutionScope::Outer);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "ConcurrentInputConflict");
    }

    #[test]
    fn empty_input_allows_multiple_reads_without_consumption_conflict() {
        let effects = SharedEffectBatch {
            input_reads: vec![
                InputRead {
                    thread_id: 0,
                    register: 1,
                    value: None,
                },
                InputRead {
                    thread_id: 1,
                    register: 2,
                    value: None,
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(&effects, false, false, 1, ExecutionScope::Outer);
        assert!(errors.is_empty());
    }

    #[test]
    fn equal_shared_writes_still_conflict_and_participants_are_sorted() {
        let effects = SharedEffectBatch {
            register_writes: vec![
                RegisterWrite {
                    thread_id: 8,
                    register: 3,
                    value: 9,
                },
                RegisterWrite {
                    thread_id: 2,
                    register: 3,
                    value: 9,
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(&effects, false, false, 1, ExecutionScope::Outer);

        assert_eq!(errors.len(), 1);
        let crate::RuntimeErrorKind::ConcurrentWriteConflict { thread_ids, .. } = errors[0].kind()
        else {
            panic!("same-register writes must conflict");
        };
        assert_eq!(thread_ids.as_slice(), &[2, 8]);
    }

    #[test]
    fn custom_memory_conflicts_are_scoped_to_the_address() {
        let custom_id = Slot::new(1).expect("ID is in range");
        let location = MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                global_tick: 3,
                caller_thread_id: 7,
                custom_id,
            },
            address: BigInt::from(-256),
        };
        let effects = SharedEffectBatch {
            memory_writes: vec![
                super::MemoryWrite {
                    thread_id: 0,
                    location: location.clone(),
                    value: 2,
                },
                super::MemoryWrite {
                    thread_id: 1,
                    location,
                    value: 3,
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(
            &effects,
            false,
            false,
            3,
            ExecutionScope::Custom {
                caller_thread_id: 7,
                custom_id,
                internal_tick: 1,
            },
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "ConcurrentMemoryWriteConflict");
    }

    #[test]
    fn code_write_conflict_uses_static_program_location() {
        let cell = crate::StaticCellId {
            code_grid: CodeGridId::Outer,
            board: BoardId::Function(Slot::new(2).expect("ID is in range")),
            folded_block: None,
            position: Coordinate { x: 1, y: 0 },
        };
        let effects = SharedEffectBatch {
            code_writes: vec![
                super::CodeWrite {
                    thread_id: 0,
                    cell,
                    primary: Some(PrimaryInstruction::Add),
                },
                super::CodeWrite {
                    thread_id: 1,
                    cell,
                    primary: None,
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(&effects, false, false, 1, ExecutionScope::Outer);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "ConcurrentCodeWriteConflict");
    }

    #[test]
    fn rejected_caller_stack_reads_do_not_create_register_conflicts() {
        let effects = SharedEffectBatch {
            register_writes: vec![RegisterWrite {
                thread_id: 3,
                register: 0,
                value: 1,
            }],
            caller_stack_reads: vec![
                CallerStackRead {
                    thread_id: 0,
                    register: 0,
                    value: Some(8),
                },
                CallerStackRead {
                    thread_id: 1,
                    register: 0,
                    value: Some(8),
                },
            ],
            ..SharedEffectBatch::default()
        };
        let errors = resolve_shared_conflicts(
            &effects,
            false,
            true,
            4,
            ExecutionScope::Custom {
                caller_thread_id: 5,
                custom_id: Slot::new(2).expect("ID is in range"),
                internal_tick: 1,
            },
        );

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "ConcurrentCallerStackReadConflict");
    }

    #[test]
    fn exhausted_caller_read_can_coexist_with_one_output() {
        let effects = SharedEffectBatch {
            caller_stack_reads: vec![CallerStackRead {
                thread_id: 0,
                register: 1,
                value: None,
            }],
            caller_stack_writes: vec![CallerStackWrite {
                thread_id: 1,
                value: 4,
            }],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(
            &effects,
            false,
            false,
            1,
            ExecutionScope::Custom {
                caller_thread_id: 2,
                custom_id: Slot::new(0).expect("ID is in range"),
                internal_tick: 1,
            },
        );
        assert!(errors.is_empty());
    }

    #[test]
    fn successful_caller_read_and_output_conflict_atomically() {
        let effects = SharedEffectBatch {
            caller_stack_reads: vec![CallerStackRead {
                thread_id: 0,
                register: 1,
                value: Some(4),
            }],
            caller_stack_writes: vec![CallerStackWrite {
                thread_id: 1,
                value: 9,
            }],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(
            &effects,
            false,
            true,
            1,
            ExecutionScope::Custom {
                caller_thread_id: 2,
                custom_id: Slot::new(0).expect("ID is in range"),
                internal_tick: 1,
            },
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "ConcurrentCallerStackReadWriteConflict");
    }

    #[test]
    fn successful_caller_read_with_multiple_outputs_reports_both_conflicts() {
        let effects = SharedEffectBatch {
            caller_stack_reads: vec![CallerStackRead {
                thread_id: 0,
                register: 1,
                value: Some(4),
            }],
            caller_stack_writes: vec![
                CallerStackWrite {
                    thread_id: 1,
                    value: 9,
                },
                CallerStackWrite {
                    thread_id: 2,
                    value: 10,
                },
            ],
            ..SharedEffectBatch::default()
        };

        let errors = resolve_shared_conflicts(
            &effects,
            false,
            true,
            1,
            ExecutionScope::Custom {
                caller_thread_id: 3,
                custom_id: Slot::new(0).expect("ID is in range"),
                internal_tick: 1,
            },
        );

        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .any(|error| error.code() == "ConcurrentCallerStackReadWriteConflict"));
        assert!(errors
            .iter()
            .any(|error| error.code() == "ConcurrentCallerStackWriteConflict"));
    }

    #[test]
    fn combined_outer_and_custom_conflicts_have_exact_canonical_order() {
        use crate::{RuntimeError, RuntimeErrorKind};

        let custom_id = Slot::new(0).expect("zero is a valid Custom ID");
        let outer_cell_first = crate::StaticCellId {
            code_grid: CodeGridId::Outer,
            board: BoardId::Main,
            folded_block: None,
            position: Coordinate { x: 0, y: 0 },
        };
        let outer_cell_second = crate::StaticCellId {
            position: Coordinate { x: 2, y: 0 },
            ..outer_cell_first
        };
        let custom_cell_first = crate::StaticCellId {
            code_grid: CodeGridId::Custom(custom_id),
            board: BoardId::Main,
            folded_block: None,
            position: Coordinate { x: 0, y: 0 },
        };
        let custom_cell_second = crate::StaticCellId {
            position: Coordinate { x: 2, y: 0 },
            ..custom_cell_first
        };
        let outer_memory_first = MemoryLocationId {
            space: MemorySpaceId::Outer,
            address: BigInt::from(-5),
        };
        let outer_memory_second = MemoryLocationId {
            address: BigInt::from(7),
            ..outer_memory_first.clone()
        };
        let custom_memory_first = MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                global_tick: 2,
                caller_thread_id: 5,
                custom_id,
            },
            address: BigInt::from(-3),
        };
        let custom_memory_second = MemoryLocationId {
            address: BigInt::from(8),
            ..custom_memory_first.clone()
        };

        let outer_effects = SharedEffectBatch {
            register_writes: vec![
                RegisterWrite {
                    thread_id: 8,
                    register: 3,
                    value: 1,
                },
                RegisterWrite {
                    thread_id: 3,
                    register: 3,
                    value: 1,
                },
                RegisterWrite {
                    thread_id: 9,
                    register: 1,
                    value: 2,
                },
                RegisterWrite {
                    thread_id: 4,
                    register: 1,
                    value: 2,
                },
            ],
            input_reads: vec![
                InputRead {
                    thread_id: 7,
                    register: 5,
                    value: Some(23),
                },
                InputRead {
                    thread_id: 2,
                    register: 5,
                    value: Some(23),
                },
            ],
            output_writes: vec![
                OutputWrite {
                    thread_id: 10,
                    value: 17,
                },
                OutputWrite {
                    thread_id: 1,
                    value: 17,
                },
            ],
            memory_writes: vec![
                MemoryWrite {
                    thread_id: 4,
                    location: outer_memory_first.clone(),
                    value: 0,
                },
                MemoryWrite {
                    thread_id: 1,
                    location: outer_memory_first,
                    value: 0,
                },
                MemoryWrite {
                    thread_id: 8,
                    location: outer_memory_second.clone(),
                    value: 3,
                },
                MemoryWrite {
                    thread_id: 3,
                    location: outer_memory_second,
                    value: 3,
                },
            ],
            code_writes: vec![
                CodeWrite {
                    thread_id: 6,
                    cell: outer_cell_second,
                    primary: Some(PrimaryInstruction::Add),
                },
                CodeWrite {
                    thread_id: 0,
                    cell: outer_cell_second,
                    primary: Some(PrimaryInstruction::Add),
                },
                CodeWrite {
                    thread_id: 7,
                    cell: outer_cell_first,
                    primary: None,
                },
                CodeWrite {
                    thread_id: 2,
                    cell: outer_cell_first,
                    primary: None,
                },
            ],
            ..SharedEffectBatch::default()
        };
        let outer_errors =
            resolve_shared_conflicts(&outer_effects, true, false, 2, ExecutionScope::Outer);

        let custom_scope = ExecutionScope::Custom {
            caller_thread_id: 5,
            custom_id,
            internal_tick: 3,
        };
        let custom_effects = SharedEffectBatch {
            register_writes: vec![
                RegisterWrite {
                    thread_id: 11,
                    register: 4,
                    value: 1,
                },
                RegisterWrite {
                    thread_id: 5,
                    register: 4,
                    value: 1,
                },
                RegisterWrite {
                    thread_id: 8,
                    register: 1,
                    value: 2,
                },
                RegisterWrite {
                    thread_id: 2,
                    register: 1,
                    value: 2,
                },
            ],
            output_writes: vec![
                OutputWrite {
                    thread_id: 6,
                    value: 13,
                },
                OutputWrite {
                    thread_id: 0,
                    value: 13,
                },
            ],
            memory_writes: vec![
                MemoryWrite {
                    thread_id: 9,
                    location: custom_memory_first.clone(),
                    value: 4,
                },
                MemoryWrite {
                    thread_id: 0,
                    location: custom_memory_first,
                    value: 4,
                },
                MemoryWrite {
                    thread_id: 5,
                    location: custom_memory_second.clone(),
                    value: 6,
                },
                MemoryWrite {
                    thread_id: 3,
                    location: custom_memory_second,
                    value: 6,
                },
            ],
            code_writes: vec![
                CodeWrite {
                    thread_id: 12,
                    cell: custom_cell_second,
                    primary: Some(PrimaryInstruction::Sub),
                },
                CodeWrite {
                    thread_id: 4,
                    cell: custom_cell_second,
                    primary: Some(PrimaryInstruction::Sub),
                },
                CodeWrite {
                    thread_id: 9,
                    cell: custom_cell_first,
                    primary: Some(PrimaryInstruction::Add),
                },
                CodeWrite {
                    thread_id: 2,
                    cell: custom_cell_first,
                    primary: Some(PrimaryInstruction::Add),
                },
            ],
            caller_stack_reads: vec![CallerStackRead {
                thread_id: 10,
                register: 7,
                value: Some(21),
            }],
            caller_stack_writes: vec![
                CallerStackWrite {
                    thread_id: 7,
                    value: 8,
                },
                CallerStackWrite {
                    thread_id: 1,
                    value: 8,
                },
            ],
            ..SharedEffectBatch::default()
        };
        let custom_errors = resolve_shared_conflicts(&custom_effects, false, true, 2, custom_scope);

        // Merge separate context-tick results in reverse order to verify that
        // the public canonical ordering still places Outer before Custom.
        let mut actual_errors = custom_errors;
        actual_errors.extend(outer_errors);
        RuntimeError::sort_canonical(&mut actual_errors);

        let expected_errors = vec![
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentCodeWriteConflict {
                    cell: outer_cell_first,
                    thread_ids: vec![2, 7],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentCodeWriteConflict {
                    cell: outer_cell_second,
                    thread_ids: vec![0, 6],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentInputConflict {
                    thread_ids: vec![2, 7],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                    address: BigInt::from(-5),
                    thread_ids: vec![1, 4],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                    address: BigInt::from(7),
                    thread_ids: vec![3, 8],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentOutputConflict {
                    thread_ids: vec![1, 10],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentWriteConflict {
                    register: 1,
                    thread_ids: vec![4, 9],
                },
            ),
            RuntimeError::new(
                2,
                ExecutionScope::Outer,
                RuntimeErrorKind::ConcurrentWriteConflict {
                    register: 3,
                    thread_ids: vec![3, 8],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                    internal_thread_ids: vec![1, 7, 10],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                    internal_thread_ids: vec![1, 7],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentCodeWriteConflict {
                    cell: custom_cell_first,
                    thread_ids: vec![2, 9],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentCodeWriteConflict {
                    cell: custom_cell_second,
                    thread_ids: vec![4, 12],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                    address: BigInt::from(-3),
                    thread_ids: vec![0, 9],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                    address: BigInt::from(8),
                    thread_ids: vec![3, 5],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentOutputConflict {
                    thread_ids: vec![0, 6],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentWriteConflict {
                    register: 1,
                    thread_ids: vec![2, 8],
                },
            ),
            RuntimeError::new(
                2,
                custom_scope,
                RuntimeErrorKind::ConcurrentWriteConflict {
                    register: 4,
                    thread_ids: vec![5, 11],
                },
            ),
        ];

        assert_eq!(actual_errors, expected_errors);
    }
}
