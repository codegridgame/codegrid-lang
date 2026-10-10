#[cfg(test)]
mod tests {
    use super::{Vm, VmStatus};
    use crate::{Coordinate, MemoryAddress, VmConfig};
    use codegrid_ir::{
        Board, Cell, CustomDefinition, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION,
    };
    use codegrid_model::{Direction};
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;

    fn verified(entries: &[Direction]) -> VerifiedProgram {
        VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: entries.len(),
                    height: 1,
                    cells: entries.iter().copied().map(Cell::entry).collect(),
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation")
    }

    fn verified_cells(cells: Vec<codegrid_ir::Cell>, width: usize) -> VerifiedProgram {
        verified_grid(cells, width, 1)
    }

    fn verified_grid(
        cells: Vec<codegrid_ir::Cell>,
        width: usize,
        height: usize,
    ) -> VerifiedProgram {
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
        .expect("test program must satisfy IR validation")
    }

    fn verified_with_custom(
        outer_cells: Vec<codegrid_ir::Cell>,
        custom_cells: Vec<codegrid_ir::Cell>,
    ) -> VerifiedProgram {
        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: outer_cells.len(),
                    height: 1,
                    cells: outer_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(
                custom_id,
                CustomDefinition {
                    program: ScopedProgram {
                        main: Board {
                            width: custom_cells.len(),
                            height: 1,
                            cells: custom_cells,
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("test program must satisfy IR validation")
    }

    fn verified_with_custom_grid(
        outer_cells: Vec<codegrid_ir::Cell>,
        custom_cells: Vec<codegrid_ir::Cell>,
        custom_width: usize,
        custom_height: usize,
    ) -> VerifiedProgram {
        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: outer_cells.len(),
                    height: 1,
                    cells: outer_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(
                custom_id,
                CustomDefinition {
                    program: ScopedProgram {
                        main: Board {
                            width: custom_width,
                            height: custom_height,
                            cells: custom_cells,
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("test program must satisfy IR validation")
    }

    fn verified_with_function(
        outer_cells: Vec<codegrid_ir::Cell>,
        function_cells: Vec<codegrid_ir::Cell>,
    ) -> VerifiedProgram {
        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");
        VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: outer_cells.len(),
                    height: 1,
                    cells: outer_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: function_cells.len(),
                        height: 1,
                        cells: function_cells,
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation")
    }

    #[test]
    fn implicit_main_entry_executes_origin_without_an_entry_operation() {
        use codegrid_model::PrimaryInstruction;
        for cells in [
            vec![Cell::empty(), Cell::instruction(PrimaryInstruction::Halt, None)],
            vec![Cell::instruction(PrimaryInstruction::Add, None), Cell::instruction(PrimaryInstruction::Halt, None)],
        ] {
            let adds = cells[0].primary.is_some();
            let mut vm = Vm::new(verified_cells(cells, 2), [], config()).unwrap();
            let snapshot = vm.snapshot();
            assert_eq!(snapshot.threads.len(), 1);
            assert_eq!(snapshot.threads[0].position, Coordinate { x: 0, y: 0 });
            assert_eq!(snapshot.threads[0].direction, Direction::Right);
            assert_eq!(vm.step().status, VmStatus::Running);
            assert_eq!(vm.snapshot().registers[0], u8::from(adds));
            assert_eq!(vm.step().status, VmStatus::Halted);
            assert_eq!(vm.snapshot().metrics.operation_count(), 1 + u64::from(adds));
        }
    }

    #[test]
    fn implicit_function_entry_executes_origin_and_returns_to_caller() {
        use codegrid_model::{PrimaryInstruction, Slot};
        let program = verified_with_function(
            vec![Cell::instruction(PrimaryInstruction::Call(Slot::new(0).unwrap()), None), Cell::instruction(PrimaryInstruction::Halt, None)],
            vec![Cell::instruction(PrimaryInstruction::Add, None), Cell::instruction(PrimaryInstruction::Return, None)],
        );
        let mut vm = Vm::new(program, [], config()).unwrap();
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 0, y: 0 });
        assert_eq!(vm.snapshot().threads[0].direction, Direction::Right);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].private_registers.unwrap()[0], 1);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running); // Function Resume moves past CALL.
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.operation_count(), 4);
    }

    #[test]
    fn implicit_custom_entry_executes_origin_and_returns() {
        use codegrid_model::{PrimaryInstruction, Slot};
        let program = verified_with_custom(
            vec![Cell::instruction(PrimaryInstruction::Custom(Slot::new(0).unwrap()), None), Cell::instruction(PrimaryInstruction::Halt, None)],
            vec![Cell::instruction(PrimaryInstruction::CustomReturn, None)],
        );
        let mut vm = Vm::new(program, [], config()).unwrap();
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.operation_count(), 2);
    }

    fn config() -> VmConfig {
        VmConfig::new(
            0,
            NonZeroU64::new(100).expect("limit is positive"),
        )
    }

    #[test]
    fn work_limit_rolls_back_outer_tick_and_normative_metrics() {
        let mut vm = Vm::new(
            verified(&[Direction::Right, Direction::Right]),
            [],
            config(),
        )
        .expect("two initial threads fit in u64");
        let before = vm.snapshot();

        let error = vm
            .step_with_work_limit(NonZeroU64::new(1).expect("limit is positive"))
            .expect_err("the second outer thread exceeds the one-unit ceiling");

        assert_eq!(error.maximum_work_units, 1);
        assert_eq!(vm.snapshot(), before);
        assert_eq!(vm.status(), VmStatus::Running);
        let accepted = vm
            .step_with_work_limit(NonZeroU64::new(2).expect("larger limit is positive"))
            .expect("the same tick must remain retryable with a sufficient ceiling");
        assert_eq!(accepted.committed_ticks, 1);
    }

    #[test]
    fn work_limit_covers_nested_custom_dispatch_and_rolls_back_the_global_tick() {
        use codegrid_model::PrimaryInstruction;

        let custom = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");
        assert_eq!(vm.step().status, VmStatus::Running);
        let before_invocation = vm.snapshot();

        let error = vm
            .step_with_work_limit(NonZeroU64::new(1).expect("limit is positive"))
            .expect_err("the Custom thread dispatch shares the outer work ceiling");

        assert_eq!(error.maximum_work_units, 1);
        assert_eq!(vm.snapshot(), before_invocation);
        let accepted = vm
            .step_with_work_limit(NonZeroU64::new(8).expect("larger limit is positive"))
            .expect("the Custom call must remain retryable with a sufficient ceiling");
        assert_eq!(accepted.committed_ticks, 2);
    }

    #[test]
    fn custom_invocation_waits_for_returns_from_every_internal_thread() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let mut custom_cells = vec![codegrid_ir::Cell::empty(); 7];
        custom_cells[0] = codegrid_ir::Cell::entry(Direction::Right);
        custom_cells[1] =
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None);
        custom_cells[4] = codegrid_ir::Cell::entry(Direction::Right);
        custom_cells[6] =
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None);
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            custom_cells,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer thread and two Custom threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let returned = vm.step();

        assert_eq!(returned.status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);
        let custom_ticks = returned
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::CellReached {
                    scope:
                        crate::ExecutionScope::Custom {
                            internal_tick, ..
                        },
                    ..
                } => Some(*internal_tick),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(custom_ticks, std::collections::BTreeSet::from([1, 2, 3]));
        assert_eq!(vm.snapshot().metrics.operation_count(), 2);
    }

    #[test]
    fn custom_threads_returning_on_the_same_tick_complete_together() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let mut custom_cells = vec![codegrid_ir::Cell::empty(); 16];
        custom_cells[0] = codegrid_ir::Cell::entry(Direction::Right);
        custom_cells[1] =
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None);
        custom_cells[7] = codegrid_ir::Cell::entry(Direction::Left);
        custom_cells[6] =
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None);
        let program = verified_with_custom_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            custom_cells,
            8,
            2,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer thread and two Custom threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let returned = vm.step();
        let returns = returned
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::CellReached {
                    scope:
                        crate::ExecutionScope::Custom {
                            internal_tick: 2,
                            ..
                        },
                    thread_id,
                    cell,
                } if cell.position == Coordinate { x: 1, y: 0 }
                    || cell.position == Coordinate { x: 6, y: 0 } =>
                {
                    Some((*thread_id, cell.position))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(returned.status, VmStatus::Running);
        assert_eq!(returns, vec![(0, Coordinate { x: 1, y: 0 }), (1, Coordinate { x: 6, y: 0 })]);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);
        assert_eq!(vm.snapshot().committed_ticks, 2);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn bounded_run_keeps_prior_commits_but_rolls_back_the_exhausted_tick() {
        let mut vm = Vm::new(
            verified(&[Direction::Right]),
            [],
            config(),
        )
        .expect("one initial thread fits in u64");

        let error = vm
            .run_with_work_limit(3, NonZeroU64::new(1).expect("limit is positive"))
            .expect_err("the second tick exceeds the shared run-slice ceiling");

        assert_eq!(error.maximum_work_units, 1);
        assert_eq!(vm.status(), VmStatus::Running);
        assert_eq!(vm.committed_ticks(), 1);
        assert_eq!(vm.snapshot().metrics.operation_count(), 0);
    }

    #[test]
    fn detailed_bounded_run_retains_prior_events_and_output_on_work_yield() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        let result = vm.run_with_work_limit_detailed(
            4,
            NonZeroU64::new(2).expect("the budget is positive"),
        );

        assert_eq!(result.outcome, crate::RunOutcome::WorkLimitReached);
        assert_eq!(result.work_limit_exceeded.unwrap().maximum_work_units, 2);
        assert_eq!(result.newly_emitted_output, vec![0]);
        assert_eq!(result.events.len(), 4);
        assert_eq!(vm.committed_ticks(), 2);
        assert_eq!(vm.snapshot().metrics.operation_count(), 1);
        assert_eq!(vm.status(), VmStatus::Running);
    }

    #[test]
    fn creates_fresh_threads_in_main_entry_grid_order() {
        let vm = Vm::new(
            verified(&[Direction::Right, Direction::Down, Direction::Left]),
            [8, 9],
            VmConfig::new(
                0,
                NonZeroU64::new(100).expect("limit is positive"),
            ),
        )
        .expect("entry count fits in u64");
        let snapshot = vm.snapshot();

        assert_eq!(vm.status(), VmStatus::Running);
        assert_eq!(snapshot.committed_ticks, 0);
        assert_eq!(snapshot.input, vec![8, 9]);
        assert_eq!(snapshot.registers, [0; 10]);
        assert_eq!(snapshot.threads.len(), 3);
        assert_eq!(snapshot.threads[0].id, 0);
        assert_eq!(snapshot.threads[0].position.x, 0);
        assert_eq!(snapshot.threads[0].direction, Direction::Right);
        assert_eq!(snapshot.threads[1].id, 1);
        assert_eq!(snapshot.threads[1].position.x, 1);
        assert_eq!(snapshot.threads[1].direction, Direction::Down);
        assert_eq!(snapshot.threads[2].id, 2);
        assert_eq!(snapshot.threads[2].position.x, 2);
        assert_eq!(snapshot.threads[2].direction, Direction::Left);
    }

    #[test]
    fn accepts_eleven_main_initial_threads_with_ordered_ids() {
        let directions = [
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Right,
            Direction::Down,
            Direction::Left,
        ];
        let vm = Vm::new(verified(&directions), [], config())
            .expect("eleven Main initial threads are valid");
        let snapshot = vm.snapshot();

        assert_eq!(snapshot.threads.len(), 11);
        for (index, (thread, direction)) in snapshot.threads.iter().zip(directions).enumerate() {
            assert_eq!(thread.id, index as u64);
            assert_eq!(thread.position, Coordinate { x: index, y: 0 });
            assert_eq!(thread.direction, direction);
        }
    }

    #[test]
    fn revisiting_a_main_entry_does_not_create_another_thread() {
        let mut vm = Vm::new(
            verified_cells(
                vec![Cell::entry(Direction::Left), Cell::empty(), Cell::empty()],
                3,
            ),
            [],
            config(),
        )
        .expect("one initial thread fits in u64");

        for _ in 0..4 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads.len(), 1);
        assert_eq!(snapshot.threads[0].id, 0);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(snapshot.threads[0].direction, Direction::Left);
        assert_eq!(snapshot.committed_ticks, 4);
    }

    #[test]
    fn accepts_explicit_sparse_initial_memory() {
        let address = crate::MemoryAddress::from(-33);
        let mut initial_memory = BTreeMap::new();
        initial_memory.insert(address.clone(), 12);
        initial_memory.insert(crate::MemoryAddress::from(200), 0);
        let vm = Vm::with_initial_memory(
            verified(&[Direction::Right]),
            std::iter::empty::<u8>(),
            initial_memory,
            VmConfig::new(
                1,
                NonZeroU64::new(1).expect("limit is positive"),
            ),
        )
        .expect("entry count fits in u64");

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.memory.read(&address), 12);
        assert_eq!(snapshot.memory.read(&crate::MemoryAddress::from(200)), 0);
        assert_eq!(snapshot.memory.allocated_cells(), 1);
    }

    #[test]
    fn step_executes_one_cell_per_tick_and_halt_commits_its_tick() {
        use crate::VmEvent;
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        let first = vm.step();
        assert_eq!(first.status, VmStatus::Running);
        assert!(matches!(
            first.events.as_slice(),
            [
                VmEvent::CellReached {
                    scope: crate::ExecutionScope::Outer,
                    thread_id: 0,
                    cell,
                },
                VmEvent::ThreadChanged { before, after, .. },
            ] if cell.position.x == 0
                && before.position.x == 0
                && after.position.x == 1
        ));
        assert_eq!(vm.snapshot().threads[0].position.x, 1);
        assert_eq!(vm.snapshot().committed_ticks, 1);

        let second = vm.step();
        assert_eq!(second.status, VmStatus::Running);
        assert!(matches!(
            second.events.as_slice(),
            [
                VmEvent::CellReached {
                    scope: crate::ExecutionScope::Outer,
                    thread_id: 0,
                    cell,
                },
                VmEvent::RegisterChanged {
                    scope: crate::ExecutionScope::Outer,
                    register: 0,
                    old: 0,
                    new: 1,
                    ..
                },
                VmEvent::ThreadChanged { before, after, .. },
            ] if cell.position.x == 1
                && before.position.x == 1
                && after.position.x == 2
        ));
        assert_eq!(vm.snapshot().registers[0], 1);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);

        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().committed_ticks, 3);
        assert_eq!(vm.snapshot().metrics.operation_count(), 2);
    }

    #[test]
    fn multiple_halt_requests_in_one_global_tick_do_not_conflict() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let halted = vm.step();

        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(halted.committed_ticks, 2);
        assert_eq!(vm.snapshot().metrics.operation_count(), 2);
        let halt_cells = halted
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::CellReached {
                    scope: crate::ExecutionScope::Outer,
                    cell,
                    ..
                } if cell.position.x == 1 || cell.position.x == 3 => Some(cell.position.x),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(halt_cells, vec![1, 3]);
    }

    #[test]
    fn successful_custom_stack_transaction_commits_with_an_outer_halt_request() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        let halted = vm.step();

        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(halted.committed_ticks, 3);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![0, 1]);
        assert_eq!(vm.snapshot().threads[1].position.x, 3);
    }

    #[test]
    fn custom_halt_commits_caller_stack_transaction_without_moving_outer_thread() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        vm.threads[0].data_stack.push(7);

        let halted = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.threads[0].position.x, 1);
        assert_eq!(snapshot.threads[0].data_stack, vec![7, 1]);
        assert_eq!(snapshot.metrics.operation_count(), 3);
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 2);
    }

    #[test]
    fn halt_does_not_skip_other_threads_current_tick_evaluation() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::empty(),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Halted);
        assert!(failed.errors.is_empty());
        assert_eq!(failed.committed_ticks, 2);
        assert_eq!(vm.snapshot().status, VmStatus::Halted);

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let custom_halt_program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::empty(),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
        );
        let custom_limit = VmConfig::new(
            0,
            NonZeroU64::new(2).expect("limit is positive"),
        );
        let mut custom_vm =
            Vm::new(custom_halt_program, [], custom_limit).expect("two initial threads fit in u64");

        assert_eq!(custom_vm.step().status, VmStatus::Running);
        let custom_failed = custom_vm.step();

        assert_eq!(custom_failed.status, VmStatus::Halted);
        assert!(custom_failed.errors.is_empty());
        assert_eq!(custom_failed.committed_ticks, 2);
    }

    #[test]
    fn threads_wrap_from_the_same_cell_and_keep_their_directions() {
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 2,
                    height: 2,
                    cells: vec![
                        Cell::empty(),
                        Cell::entry(Direction::Left),
                        Cell::entry(Direction::Up),
                        Cell::empty(),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("two distinct Main entries make a valid program");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let step = vm.step();
        assert_eq!(step.status, VmStatus::Running);
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 0, y: 1 });
        assert_eq!(snapshot.threads[0].direction, Direction::Left);
        assert_eq!(snapshot.threads[1].direction, Direction::Up);
    }

    #[test]
    fn custom_halt_takes_priority_over_same_tick_custom_return() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
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
                            height: 2,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(PrimaryInstruction::CustomReturn, None),
                                Cell::instruction(PrimaryInstruction::Halt, None),
                                Cell::entry(Direction::Left),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("Custom Main may contain multiple entries and both control instructions");
        let mut vm = Vm::new(
            program,
            [],
            VmConfig::new(
                0,
                NonZeroU64::new(10).expect("limit is positive"),
            ),
        )
        .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let halted = vm.step();
        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(halted.committed_ticks, 2);
        assert_eq!(vm.snapshot().threads[0].position.x, 1);

        let internal_tick_two_cells = halted
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::CellReached {
                    scope:
                        crate::ExecutionScope::Custom {
                            internal_tick: 2, ..
                        },
                    cell,
                    ..
                } => Some((cell.position.x, cell.position.y)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(internal_tick_two_cells, vec![(1, 0), (0, 1)]);
    }

    #[test]
    fn custom_internal_error_takes_priority_over_a_same_tick_halt_request() {
        use codegrid_model::PrimaryInstruction;

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
                            height: 3,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(PrimaryInstruction::Halt, None),
                                Cell::entry(Direction::Right),
                                Cell::instruction(PrimaryInstruction::Add, None),
                                Cell::entry(Direction::Right),
                                Cell::instruction(PrimaryInstruction::Add, None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("the Custom Main may define multiple initial threads");
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed.errors.iter().any(|error| {
            error.code() == "ConcurrentWriteConflict"
                && error.scope()
                    == crate::ExecutionScope::Custom {
                        caller_thread_id: 0,
                        custom_id,
                        internal_tick: 2,
                    }
        }));
        assert_eq!(vm.snapshot().status, VmStatus::Error);
        assert_eq!(vm.snapshot().committed_ticks, 1);
        assert!(failed.events.is_empty());
    }

    #[test]
    fn step_events_report_input_and_memory_commits_with_scope() {
        use codegrid_model::PrimaryInstruction;

        let input_program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut input_vm = Vm::new(input_program, [17], config())
            .expect("one initial thread fits in u64");
        assert_eq!(input_vm.step().status, VmStatus::Running);
        let input_step = input_vm.step();
        assert!(matches!(
            input_step.events.as_slice(),
            [
                crate::VmEvent::CellReached { .. },
                crate::VmEvent::InputConsumed {
                    scope: crate::ExecutionScope::Outer,
                    thread_id: 0,
                    value: 17,
                },
                crate::VmEvent::RegisterChanged {
                    scope: crate::ExecutionScope::Outer,
                    register: 0,
                    old: 0,
                    new: 17,
                },
                crate::VmEvent::ThreadChanged { .. },
            ]
        ));

        let memory_program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut memory_vm = Vm::new(memory_program, [], config())
            .expect("one initial thread fits in u64");
        memory_vm.registers[0] = 7;
        memory_vm.threads[0].data_stack.push(42);
        assert_eq!(memory_vm.step().status, VmStatus::Running);
        let memory_step = memory_vm.step();
        assert!(matches!(
            memory_step.events.as_slice(),
            [
                crate::VmEvent::CellReached { .. },
                crate::VmEvent::MemoryChanged {
                    scope: crate::ExecutionScope::Outer,
                    location,
                    old: 0,
                    new: 42,
                },
                crate::VmEvent::ThreadChanged { .. },
            ] if location.space == crate::MemorySpaceId::Outer
                && location.address == crate::MemoryAddress::from(7)
        ));
    }

    #[test]
    fn empty_stack_memory_store_does_not_access_an_address() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let store = vm.step();
        assert_eq!(store.status, VmStatus::Running);
        assert_eq!(store.metrics.operation_count(), 1);
        assert_eq!(store.metrics.used_memory_address_count(), 0);
        assert!(store
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::MemoryStore));
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.memory.allocated_cells(), 0);
        assert!(snapshot.threads[0].data_stack.is_empty());
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn wrapped_tick_retains_accessed_memory_address_metric() {
        use codegrid_model::PrimaryInstruction;
        use num_bigint::BigInt;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryLoad, None),
            ],
            2,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Running);
        assert!(failed.errors.is_empty());

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 0, y: 0 });
        assert_eq!(snapshot.threads[0].data_stack, vec![0]);
        assert_eq!(snapshot.metrics.operation_count(), 1);
        assert_eq!(snapshot.metrics.used_memory_address_count(), 1);
        assert!(snapshot
            .metrics
            .used_memory_addresses()
            .contains(&crate::MemoryLocationId {
                space: crate::MemorySpaceId::Outer,
                address: BigInt::from(0u8),
            }));
    }

    #[test]
    fn page_instructions_preserve_signed_and_arbitrary_precision_addresses() {
        use codegrid_model::{PageDirection, PrimaryInstruction};
        use num_bigint::BigInt;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::MovePage(PageDirection::Decrement),
                    None,
                ),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryLoad, None),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::MovePage(PageDirection::Increment),
                    None,
                ),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            6,
        );
        let initial_memory = BTreeMap::from([(BigInt::from(-256), 42)]);
        let mut vm =
            Vm::with_initial_memory(program, [], initial_memory, config())
                .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].page, BigInt::from(-1));
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![42]);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].page, BigInt::from(0));
        assert_eq!(vm.step().status, VmStatus::Running);
        let returned = vm.snapshot();
        assert!(returned.threads[0].data_stack.is_empty());
        assert_eq!(returned.memory.read(&BigInt::from(0)), 42);
        assert_eq!(returned.memory.read(&BigInt::from(-256)), 42);
        assert_eq!(returned.metrics.used_memory_address_count(), 2);
        assert_eq!(vm.step().status, VmStatus::Halted);

        let large_program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut large_page_vm = Vm::new(large_program, [], config())
            .expect("one initial thread fits in u64");
        let large_page = BigInt::from(1u8) << 256usize;
        large_page_vm.threads[0].page = large_page.clone();
        large_page_vm.registers[0] = 17;
        large_page_vm.threads[0].data_stack.push(91);
        assert_eq!(large_page_vm.step().status, VmStatus::Running);
        assert_eq!(large_page_vm.step().status, VmStatus::Running);
        let expected_address = large_page * BigInt::from(256u16) + BigInt::from(17u8);
        let large_snapshot = large_page_vm.snapshot();
        assert_eq!(
            large_snapshot.threads[0].page,
            BigInt::from(1u8) << 256usize
        );
        assert_eq!(large_snapshot.memory.read(&expected_address), 91);
        assert_eq!(large_snapshot.metrics.used_memory_address_count(), 1);
    }

    #[test]
    fn page_instructions_increment_and_decrement_above_u64_exactly() {
        use codegrid_model::{PageDirection, PrimaryInstruction};
        use num_bigint::BigInt;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::MovePage(PageDirection::Increment), None),
                Cell::instruction(PrimaryInstruction::MovePage(PageDirection::Decrement), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        let initial_page = (BigInt::from(1u8) << 64usize) + BigInt::from(41u8);
        vm.threads[0].page = initial_page.clone();

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].page, initial_page);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(
            vm.snapshot().threads[0].page,
            (BigInt::from(1u8) << 64usize) + BigInt::from(42u8)
        );
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(
            vm.snapshot().threads[0].page,
            (BigInt::from(1u8) << 64usize) + BigInt::from(41u8)
        );
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn wrapped_move_commits_instruction_effects_and_metrics() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
            ],
            2,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let result = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(result.status, VmStatus::Running);
        assert_eq!(result.attempted_tick, 2);
        assert_eq!(result.committed_ticks, 2);
        assert_eq!(result.metrics.global_tick(), 2);
        assert_eq!(result.metrics.operation_count(), 1);
        assert_eq!(result.metrics.used_cell_count(), 2);
        assert!(result
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Add));
        assert!(result.errors.is_empty());
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.metrics.operation_count(), 1);
        assert_eq!(snapshot.metrics.used_cell_count(), 2);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Add));
    }

    #[test]
    fn concurrent_writes_at_wrapped_edges_are_reported() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(
            failed
                .errors
                .iter()
                .map(|error| error.code())
                .collect::<Vec<_>>(),
            ["ConcurrentWriteConflict"]
        );
        assert!(failed.events.is_empty());
        assert_eq!(vm.snapshot().registers[0], 0);
        assert_eq!(vm.snapshot().committed_ticks, 1);
    }

    #[test]
    fn wrapped_tick_commits_call_frame_and_preserves_peak_usage() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 4,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Direction(Direction::Right), None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 1,
                        height: 1,
                        cells: vec![Cell::entry(Direction::Right)],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("CALL and function Entry satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Running);
        assert!(failed.errors.is_empty());
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.threads[0].board, codegrid_ir::BoardId::Function(function_id));
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 0, y: 0 });
        assert_eq!(snapshot.threads[0].call_stack.len(), 1);
        assert_eq!(snapshot.metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn simultaneous_writes_conflict_without_thread_order_winners() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let result = vm.step();
        let snapshot = vm.snapshot();
        assert_eq!(result.status, VmStatus::Error);
        assert_eq!(snapshot.registers[0], 0);
        assert_eq!(snapshot.committed_ticks, 1);
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(result.errors.len(), 1);
        let crate::RuntimeErrorKind::ConcurrentWriteConflict { thread_ids, .. } =
            result.errors[0].kind()
        else {
            panic!("expected a register write conflict");
        };
        assert_eq!(thread_ids, &[0, 1]);
    }

    #[test]
    fn rejected_pop_add_effects_do_not_reduce_failed_tick_stack_peak() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::PopAdd, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::PopAdd, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
            ],
            4,
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("three initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 2);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentWriteConflict"));
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.threads[0].data_stack, vec![0]);
        assert_eq!(snapshot.threads[1].data_stack, vec![0]);
        assert!(snapshot.threads[2].data_stack.is_empty());
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 3);
    }

    #[test]
    fn rejected_encode_effects_do_not_reduce_failed_tick_instruction_stack_peak() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Encode, None),
                codegrid_ir::Cell::entry(Direction::Left),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Direction(Direction::Right),
                    Some(AttachmentInstruction::ReadCode),
                ),
                codegrid_ir::Cell::empty(),
            ],
            3,
            3,
        );
        let encoded_add =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Add)
                .expect("ADD has a valid instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("three initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        vm.threads[0].instruction_stack.push(encoded_add);
        vm.threads[1].instruction_stack.push(encoded_add);
        let failed = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentWriteConflict"));
        assert_eq!(snapshot.threads[0].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.threads[1].instruction_stack, vec![encoded_add]);
        assert!(snapshot.threads[2].instruction_stack.is_empty());
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 3);
    }

    #[test]
    fn register_reads_observe_tick_start_values_when_a_sibling_writes() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None).with_prefix(codegrid_model::ConditionPrefix::Zero),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
            ],
            3,
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Running);
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(snapshot.threads[1].direction, Direction::Down);
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 1, y: 2 });
    }

    #[test]
    fn successful_outer_input_read_conflicts_with_a_sibling_register_write() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::entry(Direction::Left),
            ],
            4,
            1,
        );
        let mut vm = Vm::new(program, [9], config())
            .expect("two Main entries must initialize");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(
            failed.errors.iter().map(|error| error.code()).collect::<Vec<_>>(),
            ["ConcurrentWriteConflict"]
        );
        assert!(failed.events.is_empty());
        assert_eq!(vm.snapshot().registers[0], 0);
        assert_eq!(vm.snapshot().input.iter().copied().collect::<Vec<_>>(), [9]);
        assert_eq!(vm.snapshot().committed_ticks, 1);
    }

    #[test]
    fn successful_custom_caller_read_conflicts_with_a_sibling_register_write() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::instruction(PrimaryInstruction::Push, None),
                Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("the Custom program has two internal Entry threads");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(
            failed.errors.iter().map(|error| error.code()).collect::<Vec<_>>(),
            ["ConcurrentWriteConflict"]
        );
        assert!(failed.events.is_empty());
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        assert_eq!(vm.snapshot().committed_ticks, 3);
    }

    #[test]
    fn false_zero_prefix_preserves_direction() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None).with_prefix(codegrid_model::ConditionPrefix::Zero),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
            ],
            3,
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 1;

        assert_eq!(vm.step().status, VmStatus::Running);
        let branch = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(branch.status, VmStatus::Running);
        assert_eq!(snapshot.threads[0].direction, Direction::Right);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 1 });
    }

    #[test]
    fn memory_reads_observe_tick_start_values_when_a_sibling_writes() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryLoad, None),
                codegrid_ir::Cell::empty(),
            ],
            3,
            2,
        );
        let address = MemoryAddress::from(0);
        let mut initial_memory = BTreeMap::new();
        initial_memory.insert(address.clone(), 7);
        let mut vm =
            Vm::with_initial_memory(program, [], initial_memory, config())
                .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        vm.threads[0].data_stack.push(9);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Running);
        assert_eq!(snapshot.threads[1].data_stack, vec![7]);
        assert_eq!(snapshot.memory.read(&address), 9);
    }

    #[test]
    fn memory_store_is_read_back_by_a_load_on_the_next_tick() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryLoad, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let address = MemoryAddress::from(0);
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.threads[0].data_stack.push(42);

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        let stored = vm.snapshot();
        assert_eq!(stored.memory.read(&address), 42);
        assert!(stored.threads[0].data_stack.is_empty());

        assert_eq!(vm.step().status, VmStatus::Running);
        let loaded = vm.snapshot();
        assert_eq!(loaded.threads[0].data_stack, vec![42]);
        assert_eq!(loaded.memory.read(&address), 42);
    }

    #[test]
    fn equal_memory_writes_conflict_and_roll_back_the_entire_tick() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::MemoryStore, None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::empty(),
            ],
            3,
            3,
        );
        let address = MemoryAddress::from(0);
        let mut vm = Vm::new(program, [], config())
            .expect("three initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        vm.threads[0].data_stack.push(9);
        vm.threads[1].data_stack.push(9);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Error);
        assert!(step
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentMemoryWriteConflict"));
        assert_eq!(snapshot.memory.read(&address), 0);
        assert_eq!(snapshot.threads[0].data_stack, vec![9]);
        assert_eq!(snapshot.threads[1].data_stack, vec![9]);
        assert!(snapshot.threads[2].data_stack.is_empty());
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 3);
        assert_eq!(snapshot.committed_ticks, 1);
    }

    #[test]
    fn equal_code_writes_conflict_and_leave_the_primary_unchanged() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Direction(Direction::Right),
                    Some(AttachmentInstruction::WriteCode),
                ),
                codegrid_ir::Cell::entry(Direction::Left),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Direction(Direction::Right),
                    Some(AttachmentInstruction::ReadCode),
                ),
                codegrid_ir::Cell::empty(),
            ],
            3,
            3,
        );
        let encoded_add =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Add)
                .expect("ADD has a valid instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads.len(), 3);
        vm.threads[0].instruction_stack.push(encoded_add);
        vm.threads[1].instruction_stack.push(encoded_add);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Error);
        assert!(step
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentCodeWriteConflict"));
        assert_eq!(
            snapshot.runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Direction(Direction::Right))
        );
        assert_eq!(snapshot.threads[0].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.threads[1].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 3);
        assert_eq!(snapshot.committed_ticks, 1);
    }

    #[test]
    fn attachment_code_write_remains_a_conflict_candidate_at_wrapped_edge() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Direction(Direction::Up),
                    Some(AttachmentInstruction::WriteCode),
                ),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            3,
        );
        let encoded_add =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Add)
                .expect("ADD has a valid instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        vm.threads[0].instruction_stack.push(encoded_add);
        vm.threads[1].instruction_stack.push(encoded_add);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Error);
        assert_eq!(step.errors.len(), 1);
        assert!(step
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentCodeWriteConflict"));
        assert_eq!(
            snapshot.runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Direction(Direction::Up))
        );
        assert_eq!(snapshot.threads[0].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.threads[1].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.committed_ticks, 1);
    }

    #[test]
    fn custom_limit_failure_rolls_back_outer_wrapping_in_the_same_tick() {
        use codegrid_ir::{Program, ScopedProgram};
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 2,
                    height: 2,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None),
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
                            width: 1,
                            height: 1,
                            cells: vec![Cell::entry(Direction::Right)],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("the test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Error);
        assert_eq!(step.errors.len(), 1);
        assert!(step.errors.iter().any(|error| {
            error.code() == "CustomExecutionLimitExceeded"
                && error.scope()
                    == crate::ExecutionScope::Custom {
                        caller_thread_id: 0,
                        custom_id,
                        internal_tick: 100,
                    }
        }));
        assert_eq!(snapshot.committed_ticks, 1);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 1, y: 1 });
    }

    #[test]
    fn repeated_random_direction_advances_the_thread_stream_once_per_tick() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::RandomDirection,
                    Some(AttachmentInstruction::Repeat(3)),
                ),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let initial_state = vm.snapshot().threads[0].random_state;
        let mut expected_rng = crate::SplitMix64::new(initial_state);
        let expected_draws = (0..3)
            .map(|_| {
                let direction = expected_rng.next_direction();
                (direction, expected_rng.state())
            })
            .collect::<Vec<_>>();

        for (repeat_index, (expected_direction, expected_state)) in
            expected_draws.iter().copied().enumerate()
        {
            assert_eq!(vm.step().status, VmStatus::Running);
            let thread = &vm.snapshot().threads[0];
            assert_eq!(thread.direction, expected_direction);
            assert_eq!(thread.random_state, expected_state);
            if repeat_index < 2 {
                assert_eq!(thread.position, Coordinate { x: 1, y: 0 });
            }
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].random_state, expected_rng.state());
        assert_eq!(snapshot.metrics.operation_count(), 3);
        let expected_final_x = match expected_draws[2].0 {
            Direction::Right => 2,
            Direction::Left => 0,
            Direction::Up | Direction::Down => 1,
        };
        assert_eq!(
            snapshot.threads[0].position,
            Coordinate {
                x: expected_final_x,
                y: 0
            },
            "movement occurs only after the final RandomDirection execution"
        );
    }

    #[test]
    fn vertical_random_direction_wraps_a_single_row_and_commits_rng() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::RandomDirection, None),
            ],
            2,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let initial_state = vm.snapshot().threads[0].random_state;
        assert_eq!(
            crate::SplitMix64::new(initial_state).next_direction(),
            Direction::Down,
            "seed zero makes the attempted direction wrap this one-row board"
        );

        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Running);
        assert!(step.errors.is_empty());
        assert_ne!(snapshot.threads[0].random_state, initial_state);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(snapshot.metrics.operation_count(), 1);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::RandomDirection));
    }

    #[test]
    fn repeat_attachment_reexecutes_primary_across_ticks() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::Repeat(2)),
                ),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 1);
        assert_eq!(vm.snapshot().threads[0].position.x, 1);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 2);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.operation_count(), 3);
    }

    #[test]
    fn folded_block_enters_immediately_and_uses_a_resume_tick() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::PrimaryInstruction;

        let slot = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let main = Board {
            width: 3,
            height: 1,
            cells: vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::FoldedBlock(slot), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            folded_blocks: BTreeMap::from([(
                slot,
                FoldedBlock {
                    prefixes: Default::default(), cells: vec![Some(PrimaryInstruction::Direction(Direction::Down)); 3],
                },
            )]),
        };
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main,
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let fold_entry = vm.step();
        assert_eq!(fold_entry.status, VmStatus::Running);
        assert!(matches!(
            fold_entry.events.as_slice(),
            [
                crate::VmEvent::CellReached { cell: shell, .. },
                crate::VmEvent::CellReached { cell: folded, .. },
                crate::VmEvent::ThreadChanged { .. },
            ] if shell.folded_block.is_none()
                && shell.position.x == 1
                && folded.folded_block == Some(slot)
                && folded.position.x == 0
        ));
        assert!(matches!(
            vm.snapshot().threads[0].phase,
            super::ThreadPhaseSnapshot::FoldResume { .. }
        ));
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.global_tick(), 4);
        assert_eq!(vm.snapshot().metrics.operation_count(), 1);
        assert_eq!(vm.snapshot().metrics.used_cell_count(), 4);
    }

    #[test]
    fn custom_return_inside_outer_fold_moves_in_saved_fold_direction() {
        use codegrid_ir::{CustomDefinition, FoldedBlock};
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Custom(custom_id)),
                                Some(PrimaryInstruction::Halt),
                                None,
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(
                custom_id,
                CustomDefinition {
                    program: ScopedProgram {
                        main: Board {
                            width: 2,
                            height: 2,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(
                                    PrimaryInstruction::Direction(Direction::Down),
                                    None,
                                ),
                                Cell::empty(),
                                Cell::instruction(PrimaryInstruction::CustomReturn, None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let invoked = vm.step();
        assert_eq!(invoked.status, VmStatus::Running);
        assert!(invoked.errors.is_empty());
        assert!(invoked.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::CellReached {
                scope: crate::ExecutionScope::Custom {
                    custom_id: id,
                    internal_tick: 3,
                    ..
                },
                cell,
                ..
            } if *id == custom_id
                && cell.position == Coordinate { x: 1, y: 1 }
        )));
        assert!(matches!(
            vm.snapshot().threads[0].phase,
            super::ThreadPhaseSnapshot::Fold {
                fold_id: id,
                internal_position: Coordinate { x: 1, y: 0 },
                internal_direction: Direction::Right,
                saved_outer_direction: Direction::Right,
            } if id == fold_id
        ));

        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn folded_block_register_pointer_and_page_changes_are_visible_after_exit() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::{PageDirection, PointerDirection, PrimaryInstruction};
        use num_bigint::BigInt;

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 4,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::MemoryStore, None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::MoveRegisterPointer(
                                    PointerDirection::Right,
                                )),
                                Some(PrimaryInstruction::MovePage(PageDirection::Increment)),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                                None,
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[1] = 17;
        vm.threads[0].data_stack.push(42);

        for _ in 0..6 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].register_pointer, 1);
        assert_eq!(snapshot.threads[0].page, BigInt::from(1));
        assert_eq!(snapshot.memory.read(&BigInt::from(273)), 42);
        assert_eq!(snapshot.metrics.used_memory_address_count(), 1);
    }

    #[test]
    fn folded_block_uses_the_callers_instruction_stack() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 4,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(
                            PrimaryInstruction::Add,
                            Some(AttachmentInstruction::ReadCode),
                        ),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Encode),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                                None,
                                None,
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        let add = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].instruction_stack, vec![add]);

        assert_eq!(vm.step().status, VmStatus::Running);
        let folded = vm.snapshot();
        assert!(folded.threads[0].instruction_stack.is_empty());
        assert_eq!(folded.registers[0], add.code());

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 3, y: 0 });
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn folded_block_uses_the_callers_data_stack() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::PrimaryInstruction;

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 5,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::Clear, None),
                        Cell::instruction(PrimaryInstruction::PopAdd, None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Push),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                                None,
                                None,
                                None,
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 0x2A;

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![0x2A]);

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![0x2A]);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 0);
        assert_eq!(vm.step().status, VmStatus::Running);

        let after_pop = vm.snapshot();
        assert_eq!(after_pop.registers[0], 0x2A);
        assert!(after_pop.threads[0].data_stack.is_empty());
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn folded_block_read_and_output_use_the_callers_io_context() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::PrimaryInstruction;

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Read),
                                Some(PrimaryInstruction::Output),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [0xA5], config())
            .expect("one initial thread fits in u64");

        for _ in 0..5 {
            assert_ne!(vm.step().status, VmStatus::Error);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Running);
        assert!(snapshot.input.is_empty());
        assert_eq!(snapshot.output, vec![0xA5]);
        assert_eq!(snapshot.registers[0], 0xA5);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn folded_block_randomness_advances_the_callers_thread_stream() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::PrimaryInstruction;

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::RandomDirection),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let initial_state = vm.snapshot().threads[0].random_state;
        let mut expected_rng = crate::SplitMix64::new(initial_state);
        expected_rng.next_direction();

        for _ in 0..3 {
            assert_ne!(vm.step().status, VmStatus::Error);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(snapshot.threads[0].random_state, expected_rng.state());
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::RandomDirection));
    }

    #[test]
    fn calls_consume_a_tick_and_return_uses_a_separate_resume_tick() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let function = Board {
            width: 2,
            height: 1,
            cells: vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Return,
                    Some(AttachmentInstruction::ReadCode),
                ),
            ],
            folded_blocks: BTreeMap::new(),
        };
        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        codegrid_ir::Cell::entry(Direction::Right),
                        codegrid_ir::Cell::instruction(
                            PrimaryInstruction::Call(function_id),
                            Some(AttachmentInstruction::ReadCode),
                        ),
                        codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(function_id, function)]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        let mut invocation_events = Vec::new();
        for tick in 1..=4 {
            let result = vm.step();
            assert_eq!(result.status, VmStatus::Running);
            if tick == 4 {
                invocation_events = result.events;
            }
        }
        assert!(matches!(
            invocation_events.as_slice(),
            [
                crate::VmEvent::CellReached {
                    scope: crate::ExecutionScope::Outer,
                    cell,
                    ..
                },
                crate::VmEvent::ThreadChanged {
                    scope: crate::ExecutionScope::Outer,
                    before,
                    after,
                },
            ] if cell.board == codegrid_ir::BoardId::Function(function_id)
                && cell.position.x == 1
                && before.board == codegrid_ir::BoardId::Function(function_id)
                && after.board == codegrid_ir::BoardId::Main
                && after.position.x == 1
        ));
        let returned = vm.snapshot();
        assert_eq!(returned.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(returned.threads[0].position.x, 1);
        assert_eq!(returned.threads[0].call_stack.len(), 0);
        assert!(returned.threads[0].instruction_stack.is_empty());
        assert_eq!(
            returned.threads[0].phase,
            super::ThreadPhaseSnapshot::AfterCall
        );

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position.x, 2);
        assert_eq!(
            vm.snapshot().threads[0].instruction_stack[0].primary(),
            Some(PrimaryInstruction::Call(function_id))
        );
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.global_tick(), 6);
        assert_eq!(vm.snapshot().metrics.operation_count(), 4);
        assert_eq!(vm.snapshot().metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn recursive_callee_aftercall_uses_the_current_mutable_call_site() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");
        let program = Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 6,
                        height: 2,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::Sub, None),
                            Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None).with_prefix(codegrid_model::ConditionPrefix::Zero),
                            Cell::instruction(
                                PrimaryInstruction::Call(function_id),
                                Some(AttachmentInstruction::WriteCode),
                            ),
                            Cell::instruction(PrimaryInstruction::Return, None),
                            Cell::empty(),
                            Cell::empty(),
                            Cell::empty(),
                            Cell::instruction(
                                PrimaryInstruction::Direction(Direction::Right),
                                None,
                            ),
                            Cell::instruction(
                                PrimaryInstruction::Add,
                                Some(AttachmentInstruction::ReadCode),
                            ),
                            Cell::instruction(
                                PrimaryInstruction::Sub,
                                Some(AttachmentInstruction::ReadCode),
                            ),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        };
        let program = VerifiedProgram::new(program).expect("recursive program must verify");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 3;

        let mut call_site_writes = Vec::new();
        for _ in 0..40 {
            let step = vm.step();
            for event in &step.events {
                if let crate::VmEvent::CodeChanged {
                    cell,
                    old,
                    new,
                    ..
                } = event
                {
                    if cell.board == codegrid_ir::BoardId::Function(function_id)
                        && cell.position == (Coordinate { x: 3, y: 0 })
                    {
                        call_site_writes.push((*old, *new));
                    }
                }
            }
            match step.status {
                VmStatus::Running => {}
                VmStatus::Halted => break,
                other => panic!(
                    "recursive attachment case ended with {other:?}: {:?}",
                    step.errors.iter().map(|error| error.code()).collect::<Vec<_>>()
                ),
            }
        }

        assert_eq!(
            call_site_writes,
            vec![
                (
                    Some(PrimaryInstruction::Call(function_id)),
                    Some(PrimaryInstruction::Sub),
                ),
                (
                    Some(PrimaryInstruction::Sub),
                    Some(PrimaryInstruction::Add),
                ),
            ]
        );
        assert_eq!(
            vm.snapshot().runtime_program.functions[&function_id].cells[3].primary,
            Some(PrimaryInstruction::Add)
        );
        assert_eq!(vm.snapshot().status, VmStatus::Halted);
    }

    #[test]
    fn write_code_attached_to_return_is_inert() {
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(
                                PrimaryInstruction::Return,
                                Some(AttachmentInstruction::WriteCode),
                            ),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("RETURN with WriteCode is valid and its attachment is inert");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        let encoded_add = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code");
        vm.threads[0].instruction_stack.push(encoded_add);

        for _ in 0..8 {
            let step = vm.step();
            assert!(!step.events.iter().any(|event| matches!(
                event,
                crate::VmEvent::CodeChanged {
                    cell,
                    ..
                } if cell.board == codegrid_ir::BoardId::Function(function_id)
            )));
            if step.status == VmStatus::Halted {
                break;
            }
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Halted);
        assert_eq!(snapshot.threads[0].instruction_stack, vec![encoded_add]);
        assert_eq!(
            snapshot.runtime_program.functions[&function_id].cells[1].primary,
            Some(PrimaryInstruction::Return)
        );
        assert_eq!(snapshot.metrics.operation_count(), 3);
        assert!(!snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::WriteCode));
    }

    #[test]
    fn call_write_code_runs_after_return_using_the_callees_instruction_stack() {
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");
        let program = verified_with_function(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Call(function_id),
                    Some(AttachmentInstruction::WriteCode),
                ),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Decode, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Return, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code")
            .code();

        for _ in 0..5 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(
            vm.snapshot().threads[0].phase,
            super::ThreadPhaseSnapshot::AfterCall
        );
        assert_eq!(
            vm.snapshot().threads[0].instruction_stack,
            vec![InstructionStackItem::from_primary(PrimaryInstruction::Add)
                .expect("ADD has a valid instruction code")]
        );
        let after_call = vm.step();

        assert_eq!(after_call.status, VmStatus::Running);
        assert!(after_call.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::CodeChanged {
                cell,
                old: Some(PrimaryInstruction::Call(id)),
                new: Some(PrimaryInstruction::Add),
                ..
            } if cell.position.x == 1 && *id == function_id
        )));
        let snapshot = vm.snapshot();
        assert_eq!(
            snapshot.runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Add)
        );
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.threads[0].position.x, 2);
    }

    #[test]
    fn ordinary_call_return_restores_caller_direction_after_function_entry() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 1,
                        height: 2,
                        cells: vec![
                            Cell::entry(Direction::Down),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        for _ in 0..4 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }

        let returned = vm.snapshot();
        assert_eq!(returned.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(returned.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(returned.threads[0].direction, Direction::Right);
        assert!(returned.threads[0].call_stack.is_empty());
        assert_eq!(
            returned.threads[0].phase,
            super::ThreadPhaseSnapshot::AfterCall
        );
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn distinct_main_threads_call_and_resume_the_same_function_independently() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let mut main_cells = vec![Cell::empty(); 7];
        main_cells[0] = Cell::entry(Direction::Right);
        main_cells[1] = Cell::instruction(PrimaryInstruction::Call(function_id), None);
        main_cells[2] = Cell::instruction(PrimaryInstruction::Halt, None);
        main_cells[4] = Cell::instruction(PrimaryInstruction::Halt, None);
        main_cells[5] = Cell::instruction(PrimaryInstruction::Call(function_id), None);
        main_cells[6] = Cell::entry(Direction::Left);
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: main_cells.len(),
                    height: 1,
                    cells: main_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("both Main threads may call the same function");
        let mut vm = Vm::new(program, [], config())
            .expect("two Main initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(vm.snapshot().threads[1].position, Coordinate { x: 5, y: 0 });

        let calls = vm.step();
        assert_eq!(calls.status, VmStatus::Running);
        let called = vm.snapshot();
        for thread in &called.threads {
            assert_eq!(thread.board, codegrid_ir::BoardId::Function(function_id));
            assert_eq!(thread.call_stack.len(), 1);
        }
        assert_eq!(called.threads[0].id, 0);
        assert_eq!(called.threads[1].id, 1);

        assert_eq!(vm.step().status, VmStatus::Running);
        let returns = vm.step();
        assert_eq!(returns.status, VmStatus::Running);
        assert!(returns.errors.is_empty());
        let returned = vm.snapshot();
        assert_eq!(returned.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(returned.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(returned.threads[0].direction, Direction::Right);
        assert!(returned.threads[0].call_stack.is_empty());
        assert_eq!(
            returned.threads[0].phase,
            super::ThreadPhaseSnapshot::AfterCall
        );
        assert_eq!(returned.threads[1].board, codegrid_ir::BoardId::Main);
        assert_eq!(returned.threads[1].position, Coordinate { x: 5, y: 0 });
        assert_eq!(returned.threads[1].direction, Direction::Left);
        assert!(returned.threads[1].call_stack.is_empty());
        assert_eq!(
            returned.threads[1].phase,
            super::ThreadPhaseSnapshot::AfterCall
        );

        assert_eq!(vm.step().status, VmStatus::Running);
        let resumed = vm.snapshot();
        assert_eq!(resumed.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(resumed.threads[1].position, Coordinate { x: 4, y: 0 });
        assert_eq!(resumed.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(resumed.threads[1].board, codegrid_ir::BoardId::Main);
        assert_eq!(resumed.metrics.peak_call_stack_usage(), 2);

        let halted = vm.step();
        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(vm.snapshot().committed_ticks, 6);
    }

    #[test]
    fn ordinary_function_restores_register_pointer_but_page_changes_survive_return() {
        use codegrid_model::{PageDirection, PointerDirection, PrimaryInstruction};
        use num_bigint::BigInt;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 8,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(
                            PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
                            None,
                        ),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Read, None),
                        Cell::instruction(PrimaryInstruction::Add, None),
                        Cell::instruction(PrimaryInstruction::Push, None),
                        Cell::instruction(PrimaryInstruction::MemoryStore, None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 4,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(
                                PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
                                None,
                            ),
                            Cell::instruction(
                                PrimaryInstruction::MovePage(PageDirection::Increment),
                                None,
                            ),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [41], config())
            .expect("one initial thread fits in u64");

        for _ in 0..13 {
            let status = vm.step().status;
            if status == VmStatus::Error {
                panic!("test program unexpectedly failed during VM execution");
            }
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Halted);
        assert_eq!(snapshot.threads[0].register_pointer, 1);
        assert_eq!(snapshot.threads[0].page, BigInt::from(1));
        assert_eq!(snapshot.registers[1], 42);
        assert_eq!(snapshot.memory.read(&BigInt::from(298)), 42);
        assert!(snapshot.threads[0].data_stack.is_empty());
    }

    #[test]
    fn ordinary_function_uses_the_callers_instruction_stack() {
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 4,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(
                            PrimaryInstruction::Add,
                            Some(AttachmentInstruction::ReadCode),
                        ),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 3,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::Encode, None),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        let add = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].instruction_stack, vec![add]);
        for _ in 0..5 {
            assert_ne!(vm.step().status, VmStatus::Error);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].board, codegrid_ir::BoardId::Main);
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn ordinary_function_randomness_advances_the_callers_thread_stream() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 2,
                        height: 2,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::RandomDirection, None),
                            Cell::empty(),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let initial_state = vm.snapshot().threads[0].random_state;
        let mut expected_rng = crate::SplitMix64::new(initial_state);
        assert_eq!(expected_rng.next_direction(), Direction::Down);

        for _ in 0..5 {
            assert_ne!(vm.step().status, VmStatus::Error);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].board, codegrid_ir::BoardId::Main);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(snapshot.threads[0].random_state, expected_rng.state());
    }

    #[test]
    fn ordinary_function_read_and_output_use_the_callers_io_context() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Call(function_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 1,
                        height: 4,
                        cells: vec![
                            Cell::entry(Direction::Down),
                            Cell::instruction(PrimaryInstruction::Read, None),
                            Cell::instruction(PrimaryInstruction::Output, None),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [0x5A], config())
            .expect("one initial thread fits in u64");

        for _ in 0..7 {
            assert_ne!(vm.step().status, VmStatus::Error);
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Running);
        assert!(snapshot.input.is_empty());
        assert_eq!(snapshot.output, vec![0x5A]);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn statically_proven_self_tail_calls_reuse_the_current_call_frame() {
        use codegrid_model::PrimaryInstruction;

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 2,
                    height: 1,
                    cells: vec![
                        codegrid_ir::Cell::entry(Direction::Right),
                        codegrid_ir::Cell::instruction(PrimaryInstruction::Call(function_id), None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 4,
                        height: 1,
                        cells: vec![
                            codegrid_ir::Cell::entry(Direction::Right),
                            codegrid_ir::Cell::instruction(
                                PrimaryInstruction::Call(function_id),
                                None,
                            ),
                            codegrid_ir::Cell::empty(),
                            codegrid_ir::Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::new(),
        })
        .expect("recursive test program must satisfy IR validation");
        assert!(program.is_tail_call(
            codegrid_ir::CodeGridId::Outer,
            function_id,
            1,
            ));
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.run(40), crate::RunOutcome::TickLimitReached);
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.committed_ticks, 40);
        assert_eq!(snapshot.threads[0].call_stack.len(), 1);
        assert_eq!(snapshot.metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn custom_function_self_tail_calls_reuse_the_current_call_frame() {
        use codegrid_ir::{CodeGridId, Program, ScopedProgram};
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
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
                                Cell::instruction(PrimaryInstruction::Call(function_id), None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::from([(
                            function_id,
                            Board {
                                width: 4,
                                height: 1,
                                cells: vec![
                                    Cell::entry(Direction::Right),
                                    Cell::instruction(PrimaryInstruction::Call(function_id), None),
                                    Cell::empty(),
                                    Cell::instruction(PrimaryInstruction::Return, None),
                                ],
                                folded_blocks: BTreeMap::new(),
                            },
                        )]),
                    },
                },
            )]),
        })
        .expect("Custom self-tail-call fixture must satisfy IR validation");
        assert!(program.is_tail_call(
            CodeGridId::Custom(custom_id),
            function_id,
            1,
            ));
        let mut vm = Vm::new(
            program,
            [],
            VmConfig::new(
                0,
                NonZeroU64::new(12).expect("Custom limit is positive"),
            ),
        )
        .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed
            .errors
            .iter()
            .any(|error| error.code() == "CustomExecutionLimitExceeded"));
        assert_eq!(vm.snapshot().committed_ticks, 1);
        assert_eq!(vm.snapshot().metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn outer_read_consumes_one_byte_and_output_is_returned_as_a_tick_delta() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [19], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 19);
        assert!(vm.snapshot().input.is_empty());
        let output = vm.step();
        assert_eq!(output.newly_emitted_output, vec![19]);
        assert_eq!(vm.snapshot().output, vec![19]);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn concurrent_outer_outputs_conflict_and_roll_back_the_global_tick() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed.events.is_empty());
        assert!(failed.newly_emitted_output.is_empty());
        assert_eq!(failed.committed_ticks, 1);
        assert!(failed.errors.iter().any(|error| {
            error.code() == "ConcurrentOutputConflict"
                && error.scope() == crate::ExecutionScope::Outer
                && matches!(
                    error.kind(),
                    crate::RuntimeErrorKind::ConcurrentOutputConflict { thread_ids }
                        if thread_ids == &[0, 1]
                )
        }));
        assert!(vm.snapshot().output.is_empty());
        assert_eq!(vm.snapshot().committed_ticks, 1);
    }

    #[test]
    fn custom_read_transactionally_pops_the_outer_callers_data_stack() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        for _ in 0..4 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let returned = vm.snapshot();
        assert!(returned.threads[0].data_stack.is_empty());
        assert_eq!(returned.registers[0], 1);
        assert_eq!(returned.committed_ticks, 4);
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn custom_output_transactionally_pushes_a_local_register_value() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        let mut invocation = None;
        for outer_tick in 1..=4 {
            let result = vm.step();
            assert_eq!(result.status, VmStatus::Running);
            if outer_tick == 4 {
                invocation = Some(result);
            }
        }
        let invocation = invocation.expect("Custom invocation tick was captured");
        let internal_visits = invocation
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::CellReached {
                    scope: crate::ExecutionScope::Custom { internal_tick, .. },
                    cell,
                    ..
                } => Some((*internal_tick, cell.position.x)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(internal_visits, vec![(1, 0), (2, 1), (3, 2), (4, 3)]);
        let last_cell_event = invocation
            .events
            .iter()
            .rposition(|event| matches!(event, crate::VmEvent::CellReached { .. }))
            .expect("the tick visits at least one cell");
        let internal_register_event = invocation
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    crate::VmEvent::RegisterChanged {
                        scope: crate::ExecutionScope::Custom {
                            caller_thread_id: 0,
                            internal_tick: 2,
                            ..
                        },
                        register: 0,
                        old: 0,
                        new: 1,
                    }
                )
            })
            .expect("Custom register change is observable");
        assert!(internal_register_event > last_cell_event);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1, 1]);
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 2);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn simultaneous_custom_state_events_are_ordered_by_aligned_internal_tick() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let call = || codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None);
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                call(),
                codegrid_ir::Cell::entry(Direction::Right),
                call(),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("both Main entries must initialize");
        assert_eq!(vm.snapshot().threads.len(), 2);
        assert_eq!(vm.step().status, VmStatus::Running);

        let invocation = vm.step();
        let register_event_scopes = invocation
            .events
            .iter()
            .filter_map(|event| match event {
                crate::VmEvent::RegisterChanged {
                    scope:
                        crate::ExecutionScope::Custom {
                            caller_thread_id,
                            internal_tick,
                            ..
                        },
                    old,
                    new,
                    ..
                } => Some((*internal_tick, *caller_thread_id, *old, *new)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            register_event_scopes,
            vec![(2, 0, 0, 1), (2, 1, 0, 1), (3, 0, 1, 2), (3, 1, 1, 2)]
        );

        let cell_visit_count = invocation
            .events
            .iter()
            .take_while(|event| matches!(event, crate::VmEvent::CellReached { .. }))
            .count();
        assert!(cell_visit_count > 0);
        assert!(invocation.events[cell_visit_count..]
            .iter()
            .all(|event| !matches!(event, crate::VmEvent::CellReached { .. })));

        let last_custom_state_event = invocation
            .events
            .iter()
            .rposition(|event| {
                matches!(
                    event,
                    crate::VmEvent::InputConsumed {
                        scope: crate::ExecutionScope::Custom { .. },
                        ..
                    } | crate::VmEvent::RegisterChanged {
                        scope: crate::ExecutionScope::Custom { .. },
                        ..
                    } | crate::VmEvent::MemoryChanged {
                        scope: crate::ExecutionScope::Custom { .. },
                        ..
                    } | crate::VmEvent::CodeChanged {
                        scope: crate::ExecutionScope::Custom { .. },
                        ..
                    } | crate::VmEvent::ThreadChanged {
                        scope: crate::ExecutionScope::Custom { .. },
                        ..
                    }
                )
            })
            .expect("both Custom invocations must emit committed state events");
        let first_outer_state_event = invocation
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    crate::VmEvent::ThreadChanged {
                        scope: crate::ExecutionScope::Outer,
                        ..
                    }
                )
            })
            .expect("outer callers must emit committed thread events");
        assert!(last_custom_state_event < first_outer_state_event);
    }

    #[test]
    fn failed_custom_tick_counts_staged_caller_stack_output_but_rolls_it_back() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let pop_add = vm.step();
        assert_eq!(pop_add.status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(vm.snapshot().threads[0].direction, Direction::Right);
        let nand = vm.step();
        assert_eq!(nand.status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 3, y: 0 });
        assert_eq!(vm.snapshot().threads[0].direction, Direction::Right);
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed
            .errors
            .iter()
            .any(|error| error.code() == "CustomExecutionLimitExceeded"));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].data_stack, vec![1]);
        assert_eq!(snapshot.committed_ticks, 3);
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 51);
    }

    #[test]
    fn custom_return_and_halt_at_edges_do_not_move_the_outer_caller() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let returning = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let limit_two = VmConfig::new(
            0,
            NonZeroU64::new(2).expect("limit is positive"),
        );
        let mut returning_vm =
            Vm::new(returning, [], limit_two).expect("one outer initial thread fits in u64");
        assert_eq!(returning_vm.step().status, VmStatus::Running);
        let returned_at_limit = returning_vm.step();
        assert_eq!(returned_at_limit.status, VmStatus::Running);
        assert!(returned_at_limit.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::CellReached {
                scope: crate::ExecutionScope::Custom {
                    internal_tick: 2,
                    ..
                },
                cell,
                ..
            } if cell.position.x == 1
        )));
        assert_eq!(returning_vm.snapshot().threads[0].position.x, 2);
        assert_eq!(returning_vm.step().status, VmStatus::Halted);

        let halting = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
        );
        let mut halting_vm =
            Vm::new(halting, [], limit_two).expect("one outer initial thread fits in u64");
        assert_eq!(halting_vm.step().status, VmStatus::Running);
        let halted = halting_vm.step();
        assert_eq!(halted.status, VmStatus::Halted);
        assert!(halted.errors.is_empty());
        assert_eq!(halting_vm.snapshot().threads[0].position.x, 1);

        let halt_with_other_error = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
        );
        let mut competing_vm = Vm::new(halt_with_other_error, [23], limit_two)
            .expect("three outer initial threads fit in u64");
        assert_eq!(competing_vm.step().status, VmStatus::Running);
        let failed = competing_vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed
            .errors
            .iter()
            .any(|error| error.code() == "ConcurrentInputConflict"));
        assert_eq!(competing_vm.snapshot().committed_ticks, 1);
    }

    #[test]
    fn exhausted_custom_read_can_coexist_with_one_caller_stack_output() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![0]);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn empty_custom_read_preserves_register_and_sets_flag_before_later_output() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let mut custom_cells = vec![codegrid_ir::Cell::empty(); 10];
        custom_cells[0] = codegrid_ir::Cell::entry(Direction::Right);
        custom_cells[1] = codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None);
        custom_cells[2] = codegrid_ir::Cell::instruction(
            PrimaryInstruction::Read,
            None,
        );
        custom_cells[3] = codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None);
        custom_cells[4] =
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None);
        let program = verified_with_custom_grid(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            custom_cells,
            5,
            2,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let returned = vm.step();

        assert_eq!(returned.status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        assert!(returned.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::ThreadChanged {
                scope: crate::ExecutionScope::Custom {
                    internal_tick: 3,
                    ..
                },
                before,
                after,
            } if before.direction == Direction::Right
                && after.direction == Direction::Right
                && after.status_flag == 1
                && after.position == Coordinate { x: 3, y: 0 }
        )));
        assert_eq!(vm.snapshot().metrics.operation_count(), 4);
    }

    #[test]
    fn successful_custom_read_and_output_conflict_atomically() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(
            failed.errors[0].code(),
            "ConcurrentCallerStackReadWriteConflict"
        );
        assert!(matches!(
            failed.errors[0].scope(),
            crate::ExecutionScope::Custom {
                internal_tick: 2,
                ..
            }
        ));
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        assert_eq!(vm.snapshot().committed_ticks, 3);
    }

    #[test]
    fn concurrent_custom_outputs_conflict_and_roll_back_the_caller_stack() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer thread and two Custom threads fit in u64");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert!(failed.events.is_empty());
        assert!(failed.errors.iter().any(|error| {
            error.code() == "ConcurrentCallerStackWriteConflict"
                && error.scope()
                    == crate::ExecutionScope::Custom {
                        caller_thread_id: 0,
                        custom_id,
                        internal_tick: 2,
                    }
                && matches!(
                    error.kind(),
                    crate::RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                        internal_thread_ids
                    } if internal_thread_ids == &[0, 1]
                )
        }));
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        assert_eq!(vm.snapshot().committed_ticks, 3);
    }

    #[test]
    fn concurrent_custom_reads_of_a_nonempty_caller_stack_conflict() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.errors[0].code(), "ConcurrentCallerStackReadConflict");
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![1]);
        assert_eq!(vm.snapshot().committed_ticks, 3);
    }

    #[test]
    fn aligned_custom_internal_stacks_contribute_to_the_global_peak() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        for _ in 0..4 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 3);
        assert_eq!(vm.step().status, VmStatus::Halted);
    }

    #[test]
    fn failed_custom_tick_preserves_transient_internal_stack_peak() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.errors[0].code(), "CustomExecutionLimitExceeded");
        assert!(matches!(
            failed.errors[0].scope(),
            crate::ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: id,
                internal_tick: 100,
            } if id == custom_id
        ));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.committed_ticks, 1);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert!(snapshot.threads[0].data_stack.is_empty());
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 50);
    }

    #[test]
    fn simultaneous_custom_invocations_isolate_registers_and_local_memory() {
        use codegrid_ir::{Program, ScopedProgram};
        use codegrid_model::{PointerDirection, PrimaryInstruction};

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let mut custom_cells = vec![Cell::empty(); 20];
        custom_cells[0] = Cell::entry(Direction::Right);
        custom_cells[1] = Cell::instruction(PrimaryInstruction::Read, None);
        custom_cells[2] = Cell::instruction(PrimaryInstruction::Direction(Direction::Down), None).with_prefix(codegrid_model::ConditionPrefix::Zero);
        custom_cells[3] = Cell::instruction(PrimaryInstruction::Push, None);
        custom_cells[4] = Cell::instruction(
            PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
            None,
        );
        custom_cells[5] = Cell::instruction(PrimaryInstruction::Add, None);
        custom_cells[6] = Cell::instruction(
            PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
            None,
        );
        custom_cells[7] = Cell::instruction(PrimaryInstruction::MemoryStore, None);
        custom_cells[8] = Cell::instruction(PrimaryInstruction::CustomReturn, None);
        custom_cells[12] = Cell::instruction(PrimaryInstruction::Direction(Direction::Right), None);
        custom_cells[13] = Cell::instruction(
            PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
            None,
        );
        custom_cells[14] = Cell::instruction(PrimaryInstruction::Output, None);
        custom_cells[15] = Cell::instruction(
            PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
            None,
        );
        custom_cells[16] = Cell::instruction(PrimaryInstruction::MemoryLoad, None);
        custom_cells[17] = Cell::instruction(PrimaryInstruction::PopAdd, None);
        custom_cells[18] = Cell::instruction(PrimaryInstruction::Output, None);
        custom_cells[19] = Cell::instruction(PrimaryInstruction::CustomReturn, None);
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 5,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                        Cell::empty(),
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
                            width: 10,
                            height: 2,
                            cells: custom_cells,
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("the test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");
        vm.threads[0].data_stack.push(1);
        vm.threads[1].data_stack.push(0);

        assert_eq!(vm.step().status, VmStatus::Running);
        let step = vm.step();
        let snapshot = vm.snapshot();

        assert_eq!(step.status, VmStatus::Running, "errors: {:?}", step.errors);
        assert_eq!(snapshot.threads[0].data_stack, Vec::<u8>::new());
        assert_eq!(snapshot.threads[1].data_stack, vec![0, 0]);
        assert_eq!(snapshot.metrics.used_memory_address_count(), 2);
    }

    #[test]
    fn custom_repeat_runs_on_internal_ticks_and_obeys_the_execution_budget() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::Repeat(3)),
                ),
                Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let config_with_limit = |limit| {
            VmConfig::new(
                0,
                NonZeroU64::new(limit).expect("test limits are positive"),
            )
        };

        let mut succeeds_on_last_attempt =
            Vm::new(program.clone(), [], config_with_limit(5)).expect("valid VM");
        assert_eq!(succeeds_on_last_attempt.step().status, VmStatus::Running);
        let success = succeeds_on_last_attempt.step();
        assert_eq!(success.status, VmStatus::Running);
        assert_eq!(success.metrics.operation_count(), 4);
        assert_eq!(success.committed_ticks, 2);

        let mut limit_exhaustion = Vm::new(program, [], config_with_limit(4)).expect("valid VM");
        assert_eq!(limit_exhaustion.step().status, VmStatus::Running);
        let failure = limit_exhaustion.step();
        assert_eq!(failure.status, VmStatus::Error);
        assert_eq!(failure.errors[0].code(), "CustomExecutionLimitExceeded");
        assert_eq!(failure.metrics.operation_count(), 3);
        assert_eq!(failure.committed_ticks, 1);
    }

    #[test]
    fn concurrent_custom_invocations_align_their_internal_stack_samples() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two outer initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 2);
    }

    #[test]
    fn concurrent_custom_stack_peaks_align_across_different_completion_ticks() {
        use codegrid_ir::{Board, CustomDefinition, Program, ScopedProgram, VerifiedProgram};
        use codegrid_model::PrimaryInstruction;

        let custom_zero = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let custom_one = codegrid_model::Slot::new(1).expect("one is a valid Custom ID");
        let function_zero = codegrid_model::Slot::new(0).expect("zero is a valid function ID");
        let outer = Board {
            width: 8,
            height: 1,
            cells: vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_zero), None),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::empty(),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_one), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::entry(Direction::Left),
            ],
            folded_blocks: BTreeMap::new(),
        };

        let short_custom = CustomDefinition {
            program: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        codegrid_ir::Cell::entry(Direction::Right),
                        codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                        codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
        };

        let mut long_custom_cells = vec![codegrid_ir::Cell::entry(Direction::Right)];
        long_custom_cells
            .extend((0..32).map(|_| codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None)));
        long_custom_cells.extend([
            codegrid_ir::Cell::instruction(PrimaryInstruction::Decode, None),
            codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
            codegrid_ir::Cell::instruction(PrimaryInstruction::Call(function_zero), None),
            codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
        ]);
        let long_custom = CustomDefinition {
            program: ScopedProgram {
                main: Board {
                    width: long_custom_cells.len(),
                    height: 1,
                    cells: long_custom_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_zero,
                    Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            codegrid_ir::Cell::entry(Direction::Right),
                            codegrid_ir::Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
        };

        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: outer,
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(custom_zero, short_custom), (custom_one, long_custom)]),
        })
        .expect("the test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);

        let metrics = vm.snapshot().metrics;
        assert_eq!(metrics.peak_data_stack_usage(), 4);
        assert_eq!(metrics.peak_instruction_stack_usage(), 1);
        assert_eq!(metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn aligned_custom_samples_include_transient_caller_stack_growth() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Output, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one outer thread fits in u64");
        vm.threads[0].data_stack.push(99);

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);

        assert_eq!(vm.snapshot().threads[0].data_stack, vec![99]);
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 2);
    }

    #[test]
    fn custom_execution_limit_rolls_back_the_caller_stack_transaction() {
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let program = verified_with_custom(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Add, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Push, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
            ],
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Read, None),
                codegrid_ir::Cell::instruction(PrimaryInstruction::CustomReturn, None),
            ],
        );
        let mut vm = Vm::new(
            program,
            [],
            VmConfig::new(
                0,
                NonZeroU64::new(2).expect("limit is positive"),
            ),
        )
        .expect("one initial thread fits in u64");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.errors[0].code(), "CustomExecutionLimitExceeded");
        assert!(matches!(
            failed.errors[0].scope(),
            crate::ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: id,
                internal_tick: 2,
            } if id == custom_id
        ));
        let rolled_back = vm.snapshot();
        assert_eq!(rolled_back.threads[0].data_stack, vec![1]);
        assert_eq!(rolled_back.committed_ticks, 3);
    }

    #[test]
    fn code_write_attachment_commits_the_primary_replacement() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                codegrid_ir::Cell::entry(Direction::Right),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Decode, None),
                codegrid_ir::Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::WriteCode),
                ),
                codegrid_ir::Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        // Tests can seed state to isolate the ENCODE/DECODE machine path; the
        // public language still initializes every register to zero.
        vm.registers[0] = 32;

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].instruction_stack.len(), 1);
        let rewrite = vm.step();
        assert_eq!(rewrite.status, VmStatus::Running);
        assert!(rewrite.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::CodeChanged {
                scope: crate::ExecutionScope::Outer,
                cell,
                old: Some(PrimaryInstruction::Add),
                new: None,
            } if cell.position.x == 2
        )));
        let rewritten = vm.snapshot();
        assert_eq!(rewritten.registers[0], 33);
        assert_eq!(rewritten.runtime_program.main.cells[2].primary, None);
        assert!(rewritten.threads[0].instruction_stack.is_empty());
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.operation_count(), 4);
    }

    #[test]
    fn cleared_primary_keeps_write_code_attachment_for_later_noop_visits() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Decode, None),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::WriteCode),
                ),
                Cell::instruction(PrimaryInstruction::Direction(Direction::Left), None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 32;

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        let clear = vm.step();
        assert_eq!(clear.status, VmStatus::Running);
        assert!(clear.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::CodeChanged {
                cell,
                old: Some(PrimaryInstruction::Add),
                new: None,
                ..
            } if cell.position.x == 2
        )));
        assert_eq!(vm.snapshot().runtime_program.main.cells[2].primary, None);
        assert_eq!(
            vm.snapshot().runtime_program.main.cells[2].attachment,
            Some(AttachmentInstruction::WriteCode)
        );

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 2, y: 0 });
        let operations_before_revisit = vm.snapshot().metrics.operation_count();
        let revisit = vm.step();
        assert_eq!(revisit.status, VmStatus::Running);
        assert!(!revisit
            .events
            .iter()
            .any(|event| matches!(event, crate::VmEvent::CodeChanged { .. })));

        let snapshot = vm.snapshot();
        assert_eq!(
            snapshot.metrics.operation_count(),
            operations_before_revisit + 1
        );
        assert_eq!(snapshot.runtime_program.main.cells[2].primary, None);
        assert_eq!(
            snapshot.runtime_program.main.cells[2].attachment,
            Some(AttachmentInstruction::WriteCode)
        );
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
    }

    #[test]
    fn later_visits_execute_the_committed_self_modified_primary() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::WriteCode),
                ),
                Cell::instruction(PrimaryInstruction::Direction(Direction::Left), None),
            ],
            3,
        );
        let encoded_sub =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Sub)
                .expect("SUB has a valid instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.threads[0].instruction_stack.push(encoded_sub);

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 1);
        assert_eq!(
            vm.snapshot().runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Sub)
        );
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(vm.step().status, VmStatus::Running);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 0);
        assert_eq!(
            snapshot.runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Sub)
        );
        assert_eq!(snapshot.metrics.operation_count(), 4);
    }

    #[test]
    fn custom_self_modification_is_discarded_between_invocations() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

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
                        Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                        Cell::instruction(PrimaryInstruction::Halt, None),
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
                            width: 4,
                            height: 1,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(
                                    PrimaryInstruction::Add,
                                    Some(AttachmentInstruction::ReadCode),
                                ),
                                Cell::instruction(
                                    PrimaryInstruction::Sub,
                                    Some(AttachmentInstruction::WriteCode),
                                ),
                                Cell::instruction(PrimaryInstruction::CustomReturn, None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("the static Custom program must be valid");
        let mut vm = Vm::new(program, [], config())
            .expect("one outer initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let first_invocation = vm.step();
        assert_eq!(first_invocation.status, VmStatus::Running);
        let second_invocation = vm.step();
        assert_eq!(second_invocation.status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Halted);

        for result in [&first_invocation, &second_invocation] {
            assert!(result.events.iter().any(|event| matches!(
                event,
                crate::VmEvent::CodeChanged {
                    scope: crate::ExecutionScope::Custom {
                        custom_id: event_custom_id,
                        ..
                    },
                    cell,
                    old: Some(PrimaryInstruction::Sub),
                    new: Some(PrimaryInstruction::Add),
                } if *event_custom_id == custom_id && cell.position.x == 2
            )));
        }
    }

    #[test]
    fn sub_wraps_zero_to_u8_max() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Sub, None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(vm.step().status, VmStatus::Halted);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], u8::MAX);
        assert_eq!(snapshot.output, vec![u8::MAX]);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Sub));
    }

    #[test]
    fn add_wraps_u8_max_to_zero() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = u8::MAX;

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(vm.step().status, VmStatus::Halted);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 0);
        assert_eq!(snapshot.output, vec![0]);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Add));
    }

    #[test]
    fn clear_resets_a_nonzero_register() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Clear, None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            5,
        );
        let mut vm = Vm::new(program, [37], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 37);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 0);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Halted);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.output, vec![0]);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Clear));
    }

    #[test]
    fn nand_complements_the_bitwise_and_of_register_and_stack_values() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Push, None),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Nand, None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            7,
        );
        let mut vm = Vm::new(program, [0xAA, 0x0F], config())
            .expect("one initial thread fits in u64");

        for _ in 0..7 {
            let result = vm.step();
            if result.committed_ticks < 7 {
                assert_eq!(result.status, VmStatus::Running);
            } else {
                assert_eq!(result.status, VmStatus::Halted);
            }
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.output, vec![0xF5]);
        assert!(snapshot.threads[0].data_stack.is_empty());
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 1);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Nand));
    }

    #[test]
    fn empty_popadd_and_nand_are_counted_noops() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::PopAdd, None),
                Cell::instruction(PrimaryInstruction::Nand, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 73;

        for _ in 0..3 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        assert_eq!(vm.snapshot().registers[0], 73);
        assert!(vm.snapshot().threads[0].data_stack.is_empty());
        assert_eq!(vm.step().status, VmStatus::Halted);

        let metrics = vm.snapshot().metrics;
        assert_eq!(metrics.operation_count(), 3);
        assert!(metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::PopAdd));
        assert!(metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Nand));
        assert_eq!(metrics.peak_data_stack_usage(), 0);
    }

    #[test]
    fn nonempty_popadd_updates_register_and_consumes_one_stack_value() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Push, None),
                Cell::instruction(PrimaryInstruction::PopAdd, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 7;

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().threads[0].data_stack, vec![7]);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.snapshot().registers[0], 14);
        assert!(vm.snapshot().threads[0].data_stack.is_empty());
        assert_eq!(vm.snapshot().threads[0].position.x, 3);
        assert_eq!(vm.step().status, VmStatus::Halted);
        assert_eq!(vm.snapshot().metrics.operation_count(), 3);
        assert_eq!(vm.snapshot().metrics.peak_data_stack_usage(), 1);
    }

    #[test]
    fn shifts_truncate_left_bits_and_shift_right_logically() {
        use codegrid_model::{PrimaryInstruction, ShiftDirection};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Shift(ShiftDirection::Left), None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Read, None),
                Cell::instruction(PrimaryInstruction::Shift(ShiftDirection::Right), None),
                Cell::instruction(PrimaryInstruction::Output, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            8,
        );
        let mut vm = Vm::new(program, [0x81, 0x81], config())
            .expect("one initial thread fits in u64");

        for _ in 0..8 {
            vm.step();
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Halted);
        assert_eq!(snapshot.output, vec![0x02, 0x40]);
        assert_eq!(snapshot.registers[0], 0x40);
        assert_eq!(snapshot.metrics.operation_count(), 7);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Shift));
    }

    #[test]
    fn register_pointer_wraps_between_r0_and_r9_in_both_directions() {
        use codegrid_model::{PointerDirection, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::MoveRegisterPointer(PointerDirection::Left),
                    None,
                ),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::instruction(
                    PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
                    None,
                ),
                Cell::instruction(PrimaryInstruction::Add, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            6,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        for _ in 0..6 {
            vm.step();
        }

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.status, VmStatus::Halted);
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(snapshot.registers[9], 1);
        assert_eq!(snapshot.threads[0].register_pointer, 0);
    }

    #[test]
    fn empty_outer_read_preserves_register_and_direction_before_moving() {
        use codegrid_model::PrimaryInstruction;

        let mut cells = vec![Cell::empty(); 8];
        cells[0] = Cell::entry(Direction::Right);
        cells[1] = Cell::instruction(PrimaryInstruction::Read, None);
        cells[2] = Cell::instruction(PrimaryInstruction::Read, None);
        cells[3] = Cell::instruction(PrimaryInstruction::Halt, None);
        let program = verified_grid(cells, 4, 2);
        let mut vm = Vm::new(program, [0xAB], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 0xAB);
        assert!(snapshot.input.is_empty());
        assert_eq!(snapshot.threads[0].status_flag, 1);
        assert_eq!(snapshot.threads[0].direction, Direction::Right);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 3, y: 0 });
        assert_eq!(snapshot.metrics.operation_count(), 2);
    }

    #[test]
    fn return_without_call_is_reported_for_runtime_generated_main_return() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::WriteCode),
                ),
                Cell::instruction(PrimaryInstruction::Direction(Direction::Left), None),
            ],
            3,
        );
        let encoded_return =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Return)
                .expect("RETURN has an instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.threads[0].instruction_stack.push(encoded_return);

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(
            vm.snapshot().runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Return)
        );
        assert_eq!(vm.step().status, VmStatus::Running);
        let failed = vm.step();

        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.attempted_tick, 4);
        assert_eq!(failed.committed_ticks, 3);
        assert_eq!(failed.errors.len(), 1);
        assert_eq!(failed.errors[0].code(), "ReturnWithoutCall");
        assert!(matches!(
            failed.errors[0].kind(),
            crate::RuntimeErrorKind::ReturnWithoutCall {
                thread_id: 0,
                board: codegrid_ir::BoardId::Main,
                position: Coordinate { x: 1, y: 0 },
            }
        ));
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(snapshot.metrics.operation_count(), 3);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Return));
    }

    #[test]
    fn encode_decode_round_trip_preserves_an_instruction_stack_item() {
        use codegrid_model::{InstructionStackItem, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Encode, None),
                Cell::instruction(PrimaryInstruction::Decode, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let encoded_add = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code");
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.threads[0].instruction_stack.push(encoded_add);

        assert_eq!(vm.step().status, VmStatus::Running);
        let encoded = vm.step();
        assert_eq!(encoded.status, VmStatus::Running);
        assert!(encoded.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::RegisterChanged {
                scope: crate::ExecutionScope::Outer,
                register: 0,
                old: 0,
                new: 43,
            }
        )));
        assert_eq!(vm.step().status, VmStatus::Running);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], encoded_add.code());
        assert_eq!(snapshot.threads[0].instruction_stack, vec![encoded_add]);
        assert_eq!(snapshot.committed_ticks, 3);
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(snapshot.metrics.used_cell_count(), 3);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 1);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Encode));
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Decode));
    }

    #[test]
    fn decode_empty_then_encode_preserves_instruction_code_32() {
        use codegrid_model::{InstructionStackItem, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Decode, None),
                Cell::instruction(PrimaryInstruction::Encode, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 32;

        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(vm.step().status, VmStatus::Running);
        assert_eq!(
            vm.snapshot().threads[0].instruction_stack,
            vec![InstructionStackItem::Empty]
        );

        assert_eq!(vm.step().status, VmStatus::Running);
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 32);
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.committed_ticks, 3);
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(snapshot.metrics.used_cell_count(), 3);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 1);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Decode));
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Encode));
    }

    #[test]
    fn decode_of_an_invalid_byte_is_a_counted_no_op() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Decode, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 35;

        assert_eq!(vm.step().status, VmStatus::Running);
        let decoded = vm.step();
        assert_eq!(decoded.status, VmStatus::Running);
        assert!(!decoded
            .events
            .iter()
            .any(|event| matches!(event, crate::VmEvent::RegisterChanged { .. })));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 35);
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.metrics.operation_count(), 1);
        assert_eq!(snapshot.metrics.used_cell_count(), 2);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 0);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Decode));
    }

    #[test]
    fn encode_with_an_empty_instruction_stack_preserves_the_register() {
        use codegrid_model::PrimaryInstruction;

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Encode, None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");
        vm.registers[0] = 0xC7;

        assert_eq!(vm.step().status, VmStatus::Running);
        let encoded = vm.step();
        assert_eq!(encoded.status, VmStatus::Running);
        assert!(!encoded
            .events
            .iter()
            .any(|event| matches!(event, crate::VmEvent::RegisterChanged { .. })));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 0xC7);
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.metrics.operation_count(), 1);
        assert_eq!(snapshot.metrics.used_cell_count(), 2);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 0);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Encode));
    }

    #[test]
    fn read_code_pushes_the_tick_start_primary_onto_an_empty_stack() {
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::ReadCode),
                ),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        assert!(vm.snapshot().threads[0].instruction_stack.is_empty());
        let read = vm.step();
        assert_eq!(read.status, VmStatus::Running);
        assert!(read.events.iter().any(|event| matches!(
            event,
            crate::VmEvent::RegisterChanged {
                scope: crate::ExecutionScope::Outer,
                register: 0,
                old: 0,
                new: 1,
            }
        )));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(
            snapshot.threads[0].instruction_stack,
            vec![InstructionStackItem::from_primary(PrimaryInstruction::Add)
                .expect("ADD has a valid instruction code")]
        );
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(snapshot.metrics.used_cell_count(), 2);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 1);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::ReadCode));
    }

    #[test]
    fn write_code_with_an_empty_instruction_stack_leaves_the_cell_unchanged() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::WriteCode),
                ),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            3,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("one initial thread fits in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let write = vm.step();
        assert_eq!(write.status, VmStatus::Running);
        assert!(!write
            .events
            .iter()
            .any(|event| matches!(event, crate::VmEvent::CodeChanged { .. })));

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0], 1);
        assert_eq!(
            snapshot.runtime_program.main.cells[1].primary,
            Some(PrimaryInstruction::Add)
        );
        assert!(snapshot.threads[0].instruction_stack.is_empty());
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(snapshot.metrics.used_cell_count(), 2);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 0);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::WriteCode));
    }

    #[test]
    fn failed_repeat_tick_rolls_back_progress_and_shared_output() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let program = verified_cells(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Output,
                    Some(AttachmentInstruction::Repeat(2)),
                ),
                Cell::instruction(PrimaryInstruction::Direction(Direction::Left), None),
                Cell::entry(Direction::Left),
            ],
            4,
        );
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let first_repeat = vm.step();
        assert_eq!(first_repeat.status, VmStatus::Running);
        assert_eq!(first_repeat.newly_emitted_output, vec![0]);
        assert_eq!(
            vm.snapshot().threads[0].phase,
            super::ThreadPhaseSnapshot::Repeat {
                total: 2,
                completed: 1,
            }
        );

        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.errors.len(), 1);
        assert_eq!(failed.errors[0].code(), "ConcurrentOutputConflict");
        assert!(failed.events.is_empty());
        assert!(failed.newly_emitted_output.is_empty());

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.output, vec![0]);
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 1, y: 0 });
        assert_eq!(
            snapshot.threads[0].phase,
            super::ThreadPhaseSnapshot::Repeat {
                total: 2,
                completed: 1,
            }
        );
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 1, y: 0 });
        assert_eq!(snapshot.metrics.operation_count(), 3);
        assert_eq!(snapshot.metrics.used_cell_count(), 4);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Output));
    }

    #[test]
    fn concurrent_outer_folded_threads_keep_control_and_register_state_isolated() {
        use codegrid_ir::FoldedBlock;
        use codegrid_model::PrimaryInstruction;

        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let mut cells = vec![Cell::empty(); 15];
        cells[0] = Cell::entry(Direction::Right);
        cells[1] = Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None);
        cells[13] = Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None);
        cells[14] = Cell::entry(Direction::Left);
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 5,
                    height: 3,
                    cells,
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Add),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                                None,
                                None,
                                None,
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");
        vm.threads[1].register_pointer = 1;

        assert_eq!(vm.step().status, VmStatus::Running);
        let entered = vm.step();
        assert_eq!(entered.status, VmStatus::Running);
        let folded_visits = entered
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    crate::VmEvent::CellReached { cell, .. }
                        if cell.folded_block == Some(fold_id) && cell.position.x == 0
                )
            })
            .count();
        assert_eq!(folded_visits, 2);
        let snapshot = vm.snapshot();
        assert_eq!(snapshot.registers[0..2], [1, 1]);
        assert!(matches!(
            snapshot.threads[0].phase,
            super::ThreadPhaseSnapshot::Fold {
                fold_id: id,
                internal_position: Coordinate { x: 1, y: 0 },
                saved_outer_direction: Direction::Right,
                ..
            } if id == fold_id
        ));
        assert!(matches!(
            snapshot.threads[1].phase,
            super::ThreadPhaseSnapshot::Fold {
                fold_id: id,
                internal_position: Coordinate { x: 1, y: 0 },
                saved_outer_direction: Direction::Left,
                ..
            } if id == fold_id
        ));

        assert_eq!(vm.step().status, VmStatus::Running);
        assert!(matches!(
            vm.snapshot().threads[0].phase,
            super::ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction: Direction::Right,
            }
        ));
        assert!(matches!(
            vm.snapshot().threads[1].phase,
            super::ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction: Direction::Left,
            }
        ));
        assert_eq!(vm.step().status, VmStatus::Running);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(snapshot.threads[0].direction, Direction::Right);
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 2, y: 2 });
        assert_eq!(snapshot.threads[1].direction, Direction::Left);
        assert_eq!(snapshot.committed_ticks, 4);
        assert_eq!(snapshot.metrics.operation_count(), 2);
        assert_eq!(snapshot.metrics.used_cell_count(), 6);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Add));
    }

    #[test]
    fn concurrent_custom_folded_invocations_isolate_state_and_caller_stacks() {
        use codegrid_ir::{CustomDefinition, FoldedBlock};
        use codegrid_model::PrimaryInstruction;

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let fold_id = codegrid_model::Slot::new(0).expect("zero is a valid Folded Block ID");
        let outer = Board {
            width: 6,
            height: 1,
            cells: vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Custom(custom_id), None),
                Cell::instruction(PrimaryInstruction::Halt, None),
            ],
            folded_blocks: BTreeMap::new(),
        };
        let custom = CustomDefinition {
            program: ScopedProgram {
                main: Board {
                    width: 3,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(PrimaryInstruction::FoldedBlock(fold_id), None),
                        Cell::instruction(PrimaryInstruction::CustomReturn, None),
                    ],
                    folded_blocks: BTreeMap::from([(
                        fold_id,
                        FoldedBlock {
                            prefixes: Default::default(), cells: vec![
                                Some(PrimaryInstruction::Add),
                                Some(PrimaryInstruction::Output),
                                Some(PrimaryInstruction::Direction(Direction::Down)),
                            ],
                        },
                    )]),
                },
                functions: BTreeMap::new(),
            },
        };
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: outer,
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(custom_id, custom)]),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [], config())
            .expect("two initial threads fit in u64");

        assert_eq!(vm.step().status, VmStatus::Running);
        let invoked = vm.step();
        assert_eq!(invoked.status, VmStatus::Running);
        assert!(invoked.errors.is_empty());
        let folded_visits_by_caller = [0, 1].map(|caller_thread_id| {
            invoked
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        crate::VmEvent::CellReached {
                            scope: crate::ExecutionScope::Custom {
                                caller_thread_id: caller,
                                custom_id: id,
                                ..
                            },
                            cell,
                            ..
                        } if *caller == caller_thread_id
                            && *id == custom_id
                            && cell.code_grid == codegrid_ir::CodeGridId::Custom(custom_id)
                            && cell.folded_block == Some(fold_id)
                    )
                })
                .count()
        });
        assert_eq!(folded_visits_by_caller, [3, 3]);

        let snapshot = vm.snapshot();
        assert_eq!(snapshot.threads[0].data_stack, vec![1]);
        assert_eq!(snapshot.threads[1].data_stack, vec![1]);
        assert_eq!(snapshot.registers, [0; 10]);
        assert_eq!(snapshot.threads[0].position, Coordinate { x: 2, y: 0 });
        assert_eq!(snapshot.threads[1].position, Coordinate { x: 5, y: 0 });
        assert_eq!(snapshot.committed_ticks, 2);
        assert_eq!(snapshot.metrics.operation_count(), 6);
        assert_eq!(snapshot.metrics.used_cell_count(), 10);
        assert_eq!(snapshot.metrics.peak_data_stack_usage(), 2);
        assert_eq!(snapshot.metrics.peak_instruction_stack_usage(), 0);
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Add));
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::Output));
        assert!(snapshot
            .metrics
            .instruction_variety()
            .contains(&crate::InstructionKind::CustomReturn));
    }

    #[test]
    fn deferred_call_attachments_do_not_run_when_the_callee_halts_or_keeps_wrapping() {
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");
        let sentinel = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code");

        for attachment in [
            AttachmentInstruction::ReadCode,
            AttachmentInstruction::WriteCode,
        ] {
            for (callee_primary, expected_status) in [
                (PrimaryInstruction::Halt, VmStatus::Halted),
                (
                    PrimaryInstruction::Direction(Direction::Right),
                    VmStatus::Running,
                ),
            ] {
                let program = VerifiedProgram::new(Program {
                    format_version: IR_FORMAT_VERSION,
                    outer: ScopedProgram {
                        main: Board {
                            width: 2,
                            height: 1,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(
                                    PrimaryInstruction::Call(function_id),
                                    Some(attachment),
                                ),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::from([(
                            function_id,
                            Board {
                                width: 2,
                                height: 1,
                                cells: vec![
                                    Cell::entry(Direction::Right),
                                    Cell::instruction(callee_primary, None),
                                ],
                                folded_blocks: BTreeMap::new(),
                            },
                        )]),
                    },
                    customs: BTreeMap::new(),
                })
                .expect("the verifier accepts deferred CALL attachments");
                let mut vm = Vm::new(program, [], config())
                    .expect("one initial thread fits in u64");
                vm.threads[0].instruction_stack.push(sentinel);

                assert_eq!(vm.step().status, VmStatus::Running); // Main Entry
                assert_eq!(vm.step().status, VmStatus::Running); // CALL enters Function
                assert_eq!(vm.step().status, VmStatus::Running); // Function Entry
                let outcome = vm.step();

                assert_eq!(outcome.status, expected_status);
                if expected_status == VmStatus::Error {
                    assert!(outcome
                        .errors
                        .iter()
                        .any(|error| error.code() == "ReturnWithoutCall"));
                    assert!(outcome.events.is_empty());
                } else {
                    assert!(outcome.errors.is_empty());
                }

                let snapshot = vm.snapshot();
                assert_eq!(snapshot.threads[0].instruction_stack, vec![sentinel]);
                assert_eq!(
                    snapshot.runtime_program.main.cells[1].primary,
                    Some(PrimaryInstruction::Call(function_id))
                );
                assert!(!snapshot
                    .metrics
                    .instruction_variety()
                    .contains(&crate::InstructionKind::ReadCode));
                assert!(!snapshot
                    .metrics
                    .instruction_variety()
                    .contains(&crate::InstructionKind::WriteCode));
                assert!(!outcome
                    .events
                    .iter()
                    .any(|event| matches!(event, crate::VmEvent::CodeChanged { .. })));
            }
        }
    }

    #[test]
    fn successful_tick_emits_a_complete_ordered_outer_and_custom_event_golden() {
        use codegrid_model::{AttachmentInstruction, PrimaryInstruction};

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let mut outer_cells = vec![Cell::empty(); 11];
        outer_cells[0] = Cell::entry(Direction::Right);
        outer_cells[1] = Cell::instruction(PrimaryInstruction::Custom(custom_id), None);
        outer_cells[3] = Cell::entry(Direction::Right);
        outer_cells[4] = Cell::instruction(PrimaryInstruction::Read, None);
        outer_cells[9] = Cell::instruction(PrimaryInstruction::MemoryStore, None);
        outer_cells[10] = Cell::entry(Direction::Left);

        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: outer_cells.len(),
                    height: 1,
                    cells: outer_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(
                custom_id,
                CustomDefinition {
                    program: ScopedProgram {
                        main: Board {
                            width: 5,
                            height: 1,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(
                                    PrimaryInstruction::Read,
                                    None,
                                ),
                                Cell::instruction(PrimaryInstruction::Decode, None),
                                Cell::instruction(
                                    PrimaryInstruction::Add,
                                    Some(AttachmentInstruction::WriteCode),
                                ),
                                Cell::instruction(PrimaryInstruction::CustomReturn, None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::new(),
                    },
                },
            )]),
        })
        .expect("test program must satisfy IR validation");
        let mut vm = Vm::new(program, [17], config())
            .expect("three initial threads fit in u64");
        let encoded_direction =
            codegrid_model::InstructionStackItem::from_primary(PrimaryInstruction::Direction(
                Direction::Down,
            ))
            .expect("DOWN has a valid instruction code")
            .code();
        vm.threads[0].data_stack.push(encoded_direction);
        vm.threads[2].data_stack.push(42);

        assert_eq!(vm.step().status, VmStatus::Running); // Initialize all outer Entries.
        let step = vm.step();
        assert_eq!(step.status, VmStatus::Running);
        assert!(step.errors.is_empty());

        let actual = step
            .events
            .iter()
            .map(|event| match event {
                crate::VmEvent::CellReached {
                    scope,
                    thread_id,
                    cell,
                } => format!(
                    "cell:{scope:?}:thread{thread_id}:{:?}:{:?}:{}:{}",
                    cell.code_grid, cell.board, cell.position.x, cell.position.y
                ),
                crate::VmEvent::InputConsumed {
                    scope,
                    thread_id,
                    value,
                } => format!("input:{scope:?}:thread{thread_id}:{value}"),
                crate::VmEvent::RegisterChanged {
                    scope,
                    register,
                    old,
                    new,
                } => format!("register:{scope:?}:{register}:{old}->{new}"),
                crate::VmEvent::MemoryChanged {
                    scope,
                    location,
                    old,
                    new,
                } => format!(
                    "memory:{scope:?}:{:?}:{}:{old}->{new}",
                    location.space, location.address
                ),
                crate::VmEvent::CodeChanged {
                    scope,
                    cell,
                    old,
                    new,
                } => format!(
                    "code:{scope:?}:{:?}:{:?}:{}:{}:{old:?}->{new:?}",
                    cell.code_grid, cell.board, cell.position.x, cell.position.y
                ),
                crate::VmEvent::ThreadChanged {
                    scope,
                    before,
                    after,
                } => format!(
                    "thread:{scope:?}:{}:{:?}@{}:{}:{:?}:{:?}->{:?}@{}:{}:{:?}:{:?}",
                    before.id,
                    before.board,
                    before.position.x,
                    before.position.y,
                    before.phase,
                    before.data_stack,
                    after.board,
                    after.position.x,
                    after.position.y,
                    after.phase,
                    after.data_stack
                ),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                "cell:Outer:thread0:Outer:Main:1:0".to_owned(),
                "cell:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 1 }:thread0:Custom(Slot(0)):Main:0:0".to_owned(),
                "cell:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 2 }:thread0:Custom(Slot(0)):Main:1:0".to_owned(),
                "cell:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 3 }:thread0:Custom(Slot(0)):Main:2:0".to_owned(),
                "cell:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 4 }:thread0:Custom(Slot(0)):Main:3:0".to_owned(),
                "cell:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 5 }:thread0:Custom(Slot(0)):Main:4:0".to_owned(),
                "cell:Outer:thread1:Outer:Main:4:0".to_owned(),
                "cell:Outer:thread2:Outer:Main:9:0".to_owned(),
                "thread:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 1 }:0:Main@0:0:Normal:[]->Main@1:0:Normal:[]".to_owned(),
                format!(
                    "register:Custom {{ caller_thread_id: 0, custom_id: Slot(0), internal_tick: 2 }}:0:0->{}",
                    encoded_direction
                ),
                "thread:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 2 }:0:Main@1:0:Normal:[]->Main@2:0:Normal:[]".to_owned(),
                "thread:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 3 }:0:Main@2:0:Normal:[]->Main@3:0:Normal:[]".to_owned(),
                format!(
                    "register:Custom {{ caller_thread_id: 0, custom_id: Slot(0), internal_tick: 4 }}:0:{}->{}",
                    encoded_direction,
                    encoded_direction.wrapping_add(1)
                ),
                "code:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 4 }:Custom(Slot(0)):Main:3:0:Some(Add)->Some(Direction(Down))".to_owned(),
                "thread:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 4 }:0:Main@3:0:Normal:[]->Main@4:0:Normal:[]".to_owned(),
                "thread:Custom { caller_thread_id: 0, custom_id: Slot(0), internal_tick: 5 }:0:Main@4:0:Normal:[]->Main@4:0:Terminated:[]".to_owned(),
                "input:Outer:thread1:17".to_owned(),
                "register:Outer:0:0->17".to_owned(),
                "memory:Outer:Outer:0:0->42".to_owned(),
                format!(
                    "thread:Outer:0:Main@1:0:Normal:[{}]->Main@2:0:Normal:[]",
                    encoded_direction
                ),
                "thread:Outer:1:Main@4:0:Normal:[]->Main@5:0:Normal:[]".to_owned(),
                "thread:Outer:2:Main@9:0:Normal:[42]->Main@8:0:Normal:[]".to_owned(),
            ]
        );
    }

    #[test]
    fn failed_aligned_custom_tick_preserves_integrated_stack_high_water_formula() {
        use codegrid_ir::{CustomDefinition, Program, ScopedProgram};
        use codegrid_model::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction};

        let custom_id = codegrid_model::Slot::new(0).expect("zero is a valid Custom ID");
        let function_id = codegrid_model::Slot::new(0).expect("zero is a valid Function ID");

        let mut outer_cells = vec![Cell::empty(); 13];
        outer_cells[0] = Cell::entry(Direction::Right);
        outer_cells[1] = Cell::instruction(
            PrimaryInstruction::Push,
            Some(AttachmentInstruction::ReadCode),
        );
        outer_cells[3] = Cell::entry(Direction::Right);
        outer_cells[4] = Cell::instruction(PrimaryInstruction::PopAdd, None);
        outer_cells[6] = Cell::entry(Direction::Right);
        outer_cells[7] = Cell::instruction(PrimaryInstruction::Custom(custom_id), None);
        outer_cells[9] = Cell::entry(Direction::Right);
        outer_cells[10] = Cell::instruction(PrimaryInstruction::Clear, None);
        outer_cells[11] = Cell::instruction(PrimaryInstruction::Call(function_id), None);
        outer_cells[12] = Cell::entry(Direction::Left);

        let mut custom_cells = vec![Cell::empty(); 16];
        custom_cells[0] = Cell::entry(Direction::Right);
        custom_cells[1] = Cell::instruction(PrimaryInstruction::Push, None);
        custom_cells[2] = Cell::instruction(PrimaryInstruction::Read, None);
        custom_cells[3] = Cell::instruction(PrimaryInstruction::Decode, None);
        custom_cells[5] = Cell::instruction(PrimaryInstruction::Call(function_id), None);
        custom_cells[8] = Cell::entry(Direction::Right);
        custom_cells[9] = Cell::instruction(
            PrimaryInstruction::Push,
            Some(AttachmentInstruction::ReadCode),
        );
        custom_cells[13] = Cell::instruction(PrimaryInstruction::Call(function_id), None);

        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: outer_cells.len(),
                    height: 1,
                    cells: outer_cells,
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(
                    function_id,
                    Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::Return, None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                )]),
            },
            customs: BTreeMap::from([(
                custom_id,
                CustomDefinition {
                    program: ScopedProgram {
                        main: Board {
                            width: 8,
                            height: 2,
                            cells: custom_cells,
                            folded_blocks: BTreeMap::new(),
                        },
                        functions: BTreeMap::from([(
                            function_id,
                            Board {
                                width: 3,
                                height: 1,
                                cells: vec![
                                    Cell::entry(Direction::Right),
                                    Cell::instruction(PrimaryInstruction::Output, None),
                                    Cell::instruction(PrimaryInstruction::Return, None),
                                ],
                                folded_blocks: BTreeMap::new(),
                            },
                        )]),
                    },
                },
            )]),
        })
        .expect("the nested Outer and Custom program must verify");
        let mut vm = Vm::new(program, [], config())
            .expect("five outer initial threads fit in u64");
        let encoded_add = InstructionStackItem::from_primary(PrimaryInstruction::Add)
            .expect("ADD has a valid instruction code")
            .code();
        vm.threads[0].data_stack.push(5);
        vm.threads[1].data_stack.push(7);
        vm.threads[2].data_stack.push(encoded_add);

        assert_eq!(vm.step().status, VmStatus::Running);
        let before_failure = vm.snapshot();
        assert_eq!(before_failure.metrics.peak_data_stack_usage(), 3);
        assert_eq!(before_failure.metrics.peak_instruction_stack_usage(), 0);
        assert_eq!(before_failure.metrics.peak_call_stack_usage(), 0);

        let failed = vm.step();
        assert_eq!(failed.status, VmStatus::Error);
        assert_eq!(failed.committed_ticks, 1);
        assert!(failed.events.is_empty());
        assert!(failed.errors.iter().any(|error| {
            error.code() == "ConcurrentCallerStackWriteConflict"
                && error.scope()
                    == crate::ExecutionScope::Custom {
                        caller_thread_id: 2,
                        custom_id,
                        internal_tick: 8,
                    }
                && matches!(
                    error.kind(),
                    crate::RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                        internal_thread_ids
                    } if internal_thread_ids == &[0, 1]
                )
        }), "unexpected error set: {:?}", failed.errors);
        assert!(failed.errors.iter().any(|error| {
            error.code() == "ConcurrentWriteConflict"
                && error.scope() == crate::ExecutionScope::Outer
                && matches!(
                    error.kind(),
                    crate::RuntimeErrorKind::ConcurrentWriteConflict {
                        register: 0,
                        thread_ids,
                    } if thread_ids == &[1, 3]
                )
        }));

        let after_failure = vm.snapshot();
        assert_eq!(after_failure.committed_ticks, before_failure.committed_ticks);
        assert_eq!(after_failure.registers, before_failure.registers);
        assert_eq!(after_failure.memory, before_failure.memory);
        assert_eq!(after_failure.input, before_failure.input);
        assert_eq!(after_failure.output, before_failure.output);
        assert_eq!(after_failure.runtime_program, before_failure.runtime_program);
        assert_eq!(after_failure.threads, before_failure.threads);

        // Outer sample: baseline data 3 keeps the PopAdd's staged pop from
        // lowering the peak; the staged Outer PUSH raises it to 4. Instruction
        // and call growth are each 1. At Custom internal tick 8,
        // replace the caller's baseline depth 1 with its two staged output
        // pushes, then add both Custom threads' local stack depths (2 each).
        assert_eq!(after_failure.metrics.peak_data_stack_usage(), 7);
        assert_eq!(after_failure.metrics.peak_instruction_stack_usage(), 3);
        assert_eq!(after_failure.metrics.peak_call_stack_usage(), 3);
        assert_eq!(after_failure.metrics.global_tick(), 1);
    }
}
