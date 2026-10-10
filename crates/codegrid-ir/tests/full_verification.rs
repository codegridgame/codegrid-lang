#[cfg(test)]
mod tests {
    use codegrid_ir::{
        Board, Cell, CustomDefinition, FoldedBlock, Program, ScopedProgram, TailCallSite,
        VerifiedProgram, IR_FORMAT_VERSION,
    };
    use codegrid_model::{AttachmentInstruction, Direction, PrimaryInstruction, Slot};
    use std::collections::BTreeMap;

    fn program(cells: Vec<Cell>, width: usize) -> Program {
        Program {
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
        }
    }

    #[test]
    fn immediate_output_accepts_only_repeat_at_ir_boundary() {
        let primary = PrimaryInstruction::from_token(".3").unwrap();
        for attachment in AttachmentInstruction::ALL {
            let result = VerifiedProgram::new(program(
                vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(primary, Some(attachment)),
                ],
                2,
            ));
            assert_eq!(
                result.is_ok(),
                matches!(attachment, AttachmentInstruction::Repeat(_)),
                "{attachment:?}"
            );
        }
    }

    #[test]
    fn rejects_detached_prefixes_entry_prefixes_and_old_ir_version() {
        use codegrid_model::ConditionPrefix;
        for cell in [
            Cell::empty().with_prefix(ConditionPrefix::Zero),
            Cell::entry(Direction::Right).with_prefix(ConditionPrefix::Zero),
        ] {
            assert!(
                VerifiedProgram::new(program(vec![Cell::entry(Direction::Right), cell], 2))
                    .is_err()
            );
        }
        let mut old = program(vec![Cell::entry(Direction::Right)], 1);
        old.format_version = 1;
        assert_eq!(
            VerifiedProgram::new(old).unwrap_err()[0].code,
            "ir.unsupported_version"
        );
        let mut malformed = program(vec![Cell::entry(Direction::Right)], 1);
        malformed.outer.main.folded_blocks.insert(
            Slot::new(0).unwrap(),
            FoldedBlock {
                cells: vec![None],
                prefixes: BTreeMap::from([(1, ConditionPrefix::Zero)]),
            },
        );
        assert!(VerifiedProgram::new(malformed)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "ir.detached_attachment"));
    }

    fn program_with_function(function: Board) -> Program {
        let function_id = Slot::new(0).expect("zero is a valid function ID");
        Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 1,
                    height: 1,
                    cells: vec![Cell::entry(Direction::Right)],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::from([(function_id, function)]),
            },
            customs: BTreeMap::new(),
        }
    }

    fn self_recursive_function(tail_path: Vec<Cell>) -> Board {
        let function_id = Slot::new(0).expect("zero is a valid function ID");
        let mut cells = vec![
            Cell::entry(Direction::Right),
            Cell::instruction(PrimaryInstruction::Call(function_id), None),
        ];
        cells.extend(tail_path);
        Board {
            width: cells.len(),
            height: 1,
            cells,
            folded_blocks: BTreeMap::new(),
        }
    }

    #[test]
    fn verified_program_accepts_multiple_main_entries() {
        let checked = VerifiedProgram::new(program(
            vec![Cell::entry(Direction::Right), Cell::entry(Direction::Down)],
            2,
        ));

        assert!(checked.is_ok());
    }

    #[test]
    fn verified_program_rejects_invalid_repeat_counts() {
        let checked = VerifiedProgram::new(program(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Add,
                    Some(AttachmentInstruction::Repeat(1)),
                ),
            ],
            2,
        ));

        assert!(checked
            .expect_err("Repeat count 1 is outside the v2 range")
            .iter()
            .any(|error| error.message.contains("Repeat count")));
    }

    #[test]
    fn verified_program_rejects_invalid_ir_boundary_cases() {
        struct InvalidCase {
            name: &'static str,
            program: Program,
            expected_path: &'static str,
            expected_message: &'static str,
        }

        let dimension_mismatch = program(vec![Cell::entry(Direction::Right)], 2);

        let mut entry_shares_instruction =
            program(vec![Cell::entry(Direction::Right), Cell::empty()], 2);
        entry_shares_instruction.outer.main.cells[1] = Cell {
            prefix: None,
            entry: Some(Direction::Left),
            primary: Some(PrimaryInstruction::Add),
            attachment: None,
        };

        let mut invalid_function_entry_count = program(vec![Cell::entry(Direction::Right)], 1);
        invalid_function_entry_count.outer.functions.insert(
            Slot::new(0).expect("zero is a valid function ID"),
            Board {
                width: 2,
                height: 1,
                cells: vec![Cell::entry(Direction::Right), Cell::entry(Direction::Down)],
                folded_blocks: BTreeMap::new(),
            },
        );

        let mut invalid_fold_width = program(vec![Cell::entry(Direction::Right), Cell::empty()], 2);
        invalid_fold_width.outer.main.folded_blocks.insert(
            Slot::new(0).expect("zero is a valid Fold ID"),
            FoldedBlock {
                prefixes: Default::default(),
                cells: vec![None],
            },
        );

        let mut invalid_fold_content =
            program(vec![Cell::entry(Direction::Right), Cell::empty()], 2);
        invalid_fold_content.outer.main.folded_blocks.insert(
            Slot::new(0).expect("zero is a valid Fold ID"),
            FoldedBlock {
                prefixes: Default::default(),
                cells: vec![Some(PrimaryInstruction::Return), None],
            },
        );

        let return_outside_function = program(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(PrimaryInstruction::Return, None),
            ],
            2,
        );
        let undefined_function_reference = program(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Call(Slot::new(0).expect("zero is a valid function ID")),
                    None,
                ),
            ],
            2,
        );

        let custom_zero = Slot::new(0).expect("zero is a valid Custom ID");
        let custom_one = Slot::new(1).expect("one is a valid Custom ID");
        let function_zero = Slot::new(0).expect("zero is a valid function ID");
        let mut custom_calls_custom = program(vec![Cell::entry(Direction::Right)], 1);
        custom_calls_custom.customs.insert(
            custom_zero,
            CustomDefinition {
                program: ScopedProgram {
                    main: Board {
                        width: 2,
                        height: 1,
                        cells: vec![
                            Cell::entry(Direction::Right),
                            Cell::instruction(PrimaryInstruction::Custom(custom_one), None),
                        ],
                        folded_blocks: BTreeMap::new(),
                    },
                    functions: BTreeMap::new(),
                },
            },
        );
        custom_calls_custom.customs.insert(
            custom_one,
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
        );

        let mut custom_return_in_function = program(vec![Cell::entry(Direction::Right)], 1);
        custom_return_in_function.customs.insert(
            custom_zero,
            CustomDefinition {
                program: ScopedProgram {
                    main: Board {
                        width: 1,
                        height: 1,
                        cells: vec![Cell::entry(Direction::Right)],
                        folded_blocks: BTreeMap::new(),
                    },
                    functions: BTreeMap::from([(
                        function_zero,
                        Board {
                            width: 2,
                            height: 1,
                            cells: vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(PrimaryInstruction::CustomReturn, None),
                            ],
                            folded_blocks: BTreeMap::new(),
                        },
                    )]),
                },
            },
        );

        let mut custom_fold_contains_custom_call = program(vec![Cell::entry(Direction::Right)], 1);
        let mut custom_main = Board {
            width: 1,
            height: 1,
            cells: vec![Cell::entry(Direction::Right)],
            folded_blocks: BTreeMap::new(),
        };
        custom_main.folded_blocks.insert(
            function_zero,
            FoldedBlock {
                prefixes: Default::default(),
                cells: vec![Some(PrimaryInstruction::Custom(custom_one))],
            },
        );
        custom_fold_contains_custom_call.customs.insert(
            custom_zero,
            CustomDefinition {
                program: ScopedProgram {
                    main: custom_main,
                    functions: BTreeMap::new(),
                },
            },
        );

        let mut unsupported_version = program(vec![Cell::entry(Direction::Right)], 1);
        unsupported_version.format_version = IR_FORMAT_VERSION + 1;

        let cases = [
            InvalidCase {
                name: "board cell count does not match dimensions",
                program: dimension_mismatch,
                expected_path: "@main",
                expected_message: "Board dimensions must be positive and match its cell count.",
            },
            InvalidCase {
                name: "Entry shares a cell with an instruction",
                program: entry_shares_instruction,
                expected_path: "@main[1]",
                expected_message: "An Entry cannot share a cell with an instruction or Attachment.",
            },
            InvalidCase {
                name: "function board has multiple Entries",
                program: invalid_function_entry_count,
                expected_path: "@main.F0",
                expected_message: "A function board must contain at most one Entry.",
            },
            InvalidCase {
                name: "Fold width differs from owner board",
                program: invalid_fold_width,
                expected_path: "@main.M0",
                expected_message: "Folded Block width must match its owner board.",
            },
            InvalidCase {
                name: "Fold contains a prohibited instruction",
                program: invalid_fold_content,
                expected_path: "@main.M0[0]",
                expected_message: "This instruction is not allowed in a Folded Block.",
            },
            InvalidCase {
                name: "RETURN appears outside a function board",
                program: return_outside_function,
                expected_path: "@main[1]",
                expected_message: "RETURN is allowed only on a function board.",
            },
            InvalidCase {
                name: "CALL references an undefined function",
                program: undefined_function_reference,
                expected_path: "@main[1]",
                expected_message: "CALL references an undefined function.",
            },
            InvalidCase {
                name: "Custom Main calls another Custom",
                program: custom_calls_custom,
                expected_path: "@C0[1]",
                expected_message:
                    "Custom calls are unavailable here or reference an undefined Custom.",
            },
            InvalidCase {
                name: "Custom function contains CUSTOM_RETURN",
                program: custom_return_in_function,
                expected_path: "@C0.F0[1]",
                expected_message: "CUSTOM_RETURN is allowed only on a Custom Main board.",
            },
            InvalidCase {
                name: "Custom-owned Folded Block contains a Custom call",
                program: custom_fold_contains_custom_call,
                expected_path: "@C0.M0[0]",
                expected_message: "This instruction is not allowed in a Folded Block.",
            },
            InvalidCase {
                name: "IR format version is unsupported",
                program: unsupported_version,
                expected_path: "program",
                expected_message: "The executable program uses an unsupported IR format version.",
            },
        ];

        for case in cases {
            let errors = VerifiedProgram::new(case.program)
                .err()
                .unwrap_or_else(|| panic!("{} should be rejected", case.name));

            assert!(
                errors.iter().any(|error| {
                    error.path == case.expected_path && error.message == case.expected_message
                }),
                "{} should report {} at {}; got {errors:?}",
                case.name,
                case.expected_message,
                case.expected_path,
            );
        }
    }

    #[test]
    fn verifies_self_call_with_only_navigation_return_path_as_tail_call() {
        let function = self_recursive_function(vec![
            Cell::empty(),
            Cell::instruction(PrimaryInstruction::Return, None),
        ]);
        let program = VerifiedProgram::new(program_with_function(function))
            .expect("tail-call fixture is a valid IR program");
        let function_id = Slot::new(0).expect("zero is valid");

        assert!(program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 1,));
        assert!(program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 1,));
        assert_eq!(
            program
                .tail_call_sites()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![TailCallSite {
                code_grid: codegrid_ir::CodeGridId::Outer,
                function: function_id,
                cell_index: 1,
            }]
        );
    }

    #[test]
    fn rejects_self_call_when_return_path_has_a_side_effect() {
        let function = self_recursive_function(vec![
            Cell::instruction(PrimaryInstruction::Output, None),
            Cell::instruction(PrimaryInstruction::Return, None),
        ]);
        let program = VerifiedProgram::new(program_with_function(function))
            .expect("non-tail recursive fixture is valid IR");
        let function_id = Slot::new(0).expect("zero is valid");

        assert!(!program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 1,));
    }

    #[test]
    fn rejects_tail_optimization_when_the_call_cell_has_an_attachment() {
        let function_id = Slot::new(0).expect("zero is valid");
        let function = Board {
            width: 4,
            height: 1,
            cells: vec![
                Cell::entry(Direction::Right),
                Cell::instruction(
                    PrimaryInstruction::Call(function_id),
                    Some(AttachmentInstruction::WriteCode),
                ),
                Cell::empty(),
                Cell::instruction(PrimaryInstruction::Return, None),
            ],
            folded_blocks: BTreeMap::new(),
        };
        let program = VerifiedProgram::new(program_with_function(function))
            .expect("CALL with a code Attachment is valid IR");

        assert!(!program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 1,));
    }

    #[test]
    fn rejects_self_call_when_navigation_path_exits_without_return() {
        let function_id = Slot::new(0).expect("zero is a valid function ID");
        let mut cells = vec![Cell::empty(); 8];
        cells[0] = Cell::entry(Direction::Right);
        cells[1] = Cell::instruction(PrimaryInstruction::Call(function_id), None);
        cells[7] = Cell::instruction(PrimaryInstruction::Return, None);
        let function = Board {
            width: 4,
            height: 2,
            cells,
            folded_blocks: BTreeMap::new(),
        };
        let program = VerifiedProgram::new(program_with_function(function))
            .expect("self-call fixture is valid IR");

        assert!(!program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 1,));
    }

    #[test]
    fn tail_call_proof_follows_toroidal_navigation() {
        let function_id = Slot::new(0).expect("zero is valid");
        let function = Board {
            width: 4,
            height: 1,
            cells: vec![
                Cell::instruction(PrimaryInstruction::Return, None),
                Cell::entry(Direction::Right),
                Cell::empty(),
                Cell::instruction(PrimaryInstruction::Call(function_id), None),
            ],
            folded_blocks: BTreeMap::new(),
        };
        let program = VerifiedProgram::new(program_with_function(function))
            .expect("toroidal tail-call fixture is valid IR");

        assert!(program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 3,));
        assert!(program.is_tail_call(codegrid_ir::CodeGridId::Outer, function_id, 3,));
    }
}
