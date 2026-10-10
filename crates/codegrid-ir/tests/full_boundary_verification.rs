use std::collections::BTreeMap;

use codegrid_ir::{Board, Cell, CustomDefinition, Program, ScopedProgram, VerifiedProgram};
use codegrid_model::{Direction, Slot};

fn board(cells: Vec<Cell>, width: usize, height: usize) -> Board {
    Board {
        width,
        height,
        cells,
        folded_blocks: BTreeMap::new(),
    }
}

fn program(main: Board) -> Program {
    Program {
        format_version: codegrid_ir::IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main,
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    }
}

#[test]
fn old_shared_function_register_ir_versions_are_rejected() {
    for version in [1, 2] {
        let mut old = program(board(vec![Cell::entry(Direction::Right)], 1, 1));
        old.format_version = version;
        assert!(VerifiedProgram::new(old).is_err());
    }
}

fn program_with_attachment_candidate(
    primary: codegrid_model::PrimaryInstruction,
    attachment: codegrid_model::AttachmentInstruction,
) -> Program {
    use codegrid_ir::{CustomDefinition, FoldedBlock};
    use codegrid_model::{PrimaryInstruction as P, Slot};

    let function_id = Slot::new(0).unwrap();
    let custom_id = Slot::new(0).unwrap();

    let candidate = Cell::instruction(primary, Some(attachment));
    let mut outer_cells = vec![Cell::entry(Direction::Right)];
    let mut custom_main_cells = vec![
        Cell::entry(Direction::Right),
        Cell::instruction(P::CustomReturn, None),
    ];
    let mut functions = BTreeMap::new();
    for id in 0..Slot::COUNT as u8 {
        let id = Slot::new(id).unwrap();
        let function_cells = vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Return, None),
        ];
        functions.insert(id, board(function_cells, 2, 1));
    }
    if matches!(primary, P::Return) {
        functions.get_mut(&function_id).unwrap().cells[1] = candidate;
    } else if matches!(primary, P::CustomReturn) {
        custom_main_cells[1] = candidate;
    } else {
        outer_cells.push(candidate);
    }

    let folded_blocks = (0..Slot::COUNT as u8)
        .map(|id| {
            (
                Slot::new(id).unwrap(),
                FoldedBlock {
                    prefixes: Default::default(),
                    cells: vec![Some(P::Add); outer_cells.len()],
                },
            )
        })
        .collect();
    let outer_main = Board {
        width: outer_cells.len(),
        height: 1,
        cells: outer_cells,
        folded_blocks,
    };

    let outer = ScopedProgram {
        main: outer_main,
        functions,
    };
    let mut customs = BTreeMap::new();
    for id in 0..Slot::COUNT as u8 {
        let id = Slot::new(id).unwrap();
        let mut custom_main_cells = custom_main_cells.clone();
        if id != custom_id {
            custom_main_cells[1] = Cell::instruction(P::CustomReturn, None);
        }
        customs.insert(
            id,
            CustomDefinition {
                program: ScopedProgram {
                    main: board(custom_main_cells, 2, 1),
                    functions: BTreeMap::from([(
                        function_id,
                        board(
                            vec![
                                Cell::entry(Direction::Right),
                                Cell::instruction(P::Return, None),
                            ],
                            2,
                            1,
                        ),
                    )]),
                },
            },
        );
    }

    Program {
        format_version: codegrid_ir::IR_FORMAT_VERSION,
        outer,
        customs,
    }
}

#[test]
fn accepts_multiple_main_entries_and_implicit_origin() {
    let valid = program(board(
        vec![Cell::entry(Direction::Right), Cell::entry(Direction::Down)],
        2,
        1,
    ));
    assert!(VerifiedProgram::new(valid).is_ok());

    let invalid = program(board(vec![Cell::empty()], 1, 1));
    assert!(VerifiedProgram::new(invalid).is_ok());

    let custom_id = Slot::new(0).expect("zero is a valid Custom ID");
    let mut with_custom = program(board(vec![Cell::entry(Direction::Right)], 1, 1));
    with_custom.customs.insert(
        custom_id,
        CustomDefinition {
            program: ScopedProgram {
                main: board(
                    vec![Cell::entry(Direction::Right), Cell::entry(Direction::Down)],
                    2,
                    1,
                ),
                functions: BTreeMap::new(),
            },
        },
    );
    assert!(VerifiedProgram::new(with_custom).is_ok());
}

#[test]
fn function_boards_require_exactly_one_entry() {
    let mut invalid = program(board(vec![Cell::entry(Direction::Right)], 1, 1));
    let function_id = Slot::new(0).expect("zero is a valid function ID");
    invalid.outer.functions.insert(
        function_id,
        board(
            vec![Cell::entry(Direction::Right), Cell::entry(Direction::Down)],
            2,
            1,
        ),
    );

    let errors = VerifiedProgram::new(invalid).expect_err("Function must have at most one Entry");
    assert!(errors.iter().any(|error| error
        .message
        .contains("function board must contain at most one Entry")));
}

#[test]
fn resolves_highest_structural_ids_in_outer_and_custom_scopes() {
    use codegrid_ir::FoldedBlock;
    use codegrid_model::PrimaryInstruction as P;

    let id = Slot::new(9).expect("slot 9 is the highest Full structural ID");

    let mut outer_main = board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Call(id), None),
            Cell::instruction(P::Custom(id), None),
            Cell::instruction(P::FoldedBlock(id), None),
        ],
        4,
        1,
    );
    outer_main.folded_blocks.insert(
        id,
        FoldedBlock {
            prefixes: Default::default(),
            cells: vec![Some(P::Add), None, None, None],
        },
    );
    let outer_function = board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Custom(id), None),
        ],
        2,
        1,
    );

    let mut custom_main = board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::FoldedBlock(id), None),
        ],
        2,
        1,
    );
    custom_main.folded_blocks.insert(
        id,
        FoldedBlock {
            prefixes: Default::default(),
            cells: vec![Some(P::Add), None],
        },
    );
    let custom_function = board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Call(id), None),
            Cell::instruction(P::Return, None),
        ],
        3,
        1,
    );

    let mut program = program(outer_main);
    program.outer.functions.insert(id, outer_function);
    program.customs.insert(
        id,
        CustomDefinition {
            program: ScopedProgram {
                main: custom_main,
                functions: BTreeMap::from([(id, custom_function)]),
            },
        },
    );

    assert!(VerifiedProgram::new(program).is_ok());
}

#[test]
fn rejects_malformed_dimensions_and_unsupported_ir_versions() {
    let malformed = program(board(vec![Cell::entry(Direction::Right)], 2, 1));
    assert!(VerifiedProgram::new(malformed)
        .expect_err("cell count must match dimensions")
        .iter()
        .any(|error| error.message.contains("match its cell count")));

    let overflow = program(board(vec![Cell::entry(Direction::Right)], usize::MAX, 2));
    assert!(VerifiedProgram::new(overflow)
        .expect_err("dimension products must be checked")
        .iter()
        .any(|error| error.message.contains("dimensions overflow")));

    let mut unsupported = program(board(vec![Cell::entry(Direction::Right)], 1, 1));
    unsupported.format_version = codegrid_ir::IR_FORMAT_VERSION + 1;
    assert!(VerifiedProgram::new(unsupported)
        .expect_err("unsupported IR versions must be rejected")
        .iter()
        .any(|error| error.message.contains("unsupported IR format version")));
}

#[test]
fn rejects_board_dimensions_and_cell_counts_above_portable_full_bounds() {
    use codegrid_model::MAX_BOARD_DIMENSION;

    if let Some(over_dimension) = usize::try_from(MAX_BOARD_DIMENSION)
        .ok()
        .and_then(|maximum| maximum.checked_add(1))
    {
        for (width, height) in [(over_dimension, 1), (1, over_dimension)] {
            let candidate = program(board(vec![Cell::entry(Direction::Right)], width, height));
            let errors = VerifiedProgram::new(candidate)
                .expect_err("each dimension must fit the portable Full geometry bound");
            assert!(errors
                .iter()
                .any(|error| { error.message.contains("portable Full geometry bounds") }));
        }
    }

    let too_many_cells = program(board(vec![Cell::entry(Direction::Right)], 65_536, 65_536));
    let errors = VerifiedProgram::new(too_many_cells)
        .expect_err("the total cell count must fit the portable Full geometry bound");
    #[cfg(target_pointer_width = "64")]
    assert!(errors
        .iter()
        .any(|error| { error.message.contains("portable Full geometry bounds") }));
    #[cfg(target_pointer_width = "32")]
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("dimensions overflow the host address space")
    }));
}

#[test]
fn rejects_undefined_full_structural_references() {
    use codegrid_model::PrimaryInstruction as P;

    let custom_id = Slot::new(0).expect("zero is a valid Custom ID");
    let fold_id = Slot::new(0).expect("zero is a valid Folded Block ID");

    for (primary, message) in [
        (P::Custom(custom_id), "undefined Custom"),
        (
            P::FoldedBlock(fold_id),
            "Folded Block reference is undefined",
        ),
    ] {
        let candidate = program(board(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(primary, None),
            ],
            2,
            1,
        ));
        let errors = VerifiedProgram::new(candidate)
            .expect_err("undefined structural references must be rejected");
        assert!(errors.iter().any(|error| error.message.contains(message)));
    }
}

#[test]
fn rejects_invalid_attachment_placement_and_repeat_counts() {
    use codegrid_model::{AttachmentInstruction as Attachment, PrimaryInstruction as P};

    let detached = program(board(
        vec![
            Cell::entry(Direction::Right),
            Cell {
                prefix: None,
                entry: None,
                primary: None,
                attachment: Some(Attachment::ReadCode),
            },
        ],
        2,
        1,
    ));
    assert!(VerifiedProgram::new(detached)
        .expect_err("an Attachment requires a Primary")
        .iter()
        .any(|error| error.message.contains("requires a Primary")));

    let invalid_repeat = program(board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Add, Some(Attachment::Repeat(1))),
        ],
        2,
        1,
    ));
    assert!(VerifiedProgram::new(invalid_repeat)
        .expect_err("Repeat count 1 is outside the Full range")
        .iter()
        .any(|error| error.message.contains("Repeat count")));

    let function_id = Slot::new(0).expect("zero is a valid function ID");
    let mut repeat_call = program(board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Call(function_id), Some(Attachment::Repeat(2))),
        ],
        2,
        1,
    ));
    repeat_call.outer.functions.insert(
        function_id,
        board(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(P::Return, None),
            ],
            2,
            1,
        ),
    );
    assert!(VerifiedProgram::new(repeat_call)
        .expect_err("Repeat is not allowed on CALL")
        .iter()
        .any(|error| error
            .message
            .contains("Repeat is not allowed on CALL or RETURN")));

    let mut repeat_return = program(board(vec![Cell::entry(Direction::Right)], 1, 1));
    repeat_return.outer.functions.insert(
        function_id,
        board(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(P::Return, Some(Attachment::Repeat(2))),
            ],
            2,
            1,
        ),
    );
    assert!(VerifiedProgram::new(repeat_return)
        .expect_err("Repeat is not allowed on RETURN")
        .iter()
        .any(|error| error
            .message
            .contains("Repeat is not allowed on CALL or RETURN")));

    let halt_attachment = program(board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::Halt, Some(Attachment::WriteCode)),
        ],
        2,
        1,
    ));
    assert!(VerifiedProgram::new(halt_attachment)
        .expect_err("HALT cannot have an Attachment")
        .iter()
        .any(|error| error.message.contains("cannot have an Attachment")));

    for invalid_count in [0, 1, 6, u8::MAX] {
        let invalid_repeat = program(board(
            vec![
                Cell::entry(Direction::Right),
                Cell::instruction(P::Add, Some(Attachment::Repeat(invalid_count))),
            ],
            2,
            1,
        ));
        assert!(VerifiedProgram::new(invalid_repeat)
            .expect_err("Repeat counts outside 2 through 5 must be rejected")
            .iter()
            .any(|error| error
                .message
                .contains("Repeat count must be from 2 through 5")));
    }

    let entry_attachment = program(board(
        vec![Cell {
            prefix: None,
            entry: Some(Direction::Right),
            primary: None,
            attachment: Some(Attachment::ReadCode),
        }],
        1,
        1,
    ));
    assert!(VerifiedProgram::new(entry_attachment)
        .expect_err("an Entry cannot carry an Attachment")
        .iter()
        .any(|error| error.message.contains("Entry cannot share a cell")));
}

#[test]
fn verifies_the_complete_full_primary_attachment_compatibility_matrix() {
    use codegrid_model::{AttachmentInstruction as Attachment, PrimaryInstruction};

    for primary in PrimaryInstruction::source_forms() {
        for attachment in Attachment::ALL {
            let repeat_on_call_or_return = matches!(attachment, Attachment::Repeat(_))
                && matches!(
                    primary,
                    PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                );
            let compatible = (primary.is_encodable()
                || (matches!(primary, PrimaryInstruction::OutputImmediate(_))
                    && matches!(attachment, Attachment::Repeat(_))))
                && !repeat_on_call_or_return;
            let result =
                VerifiedProgram::new(program_with_attachment_candidate(primary, attachment));
            if compatible {
                result.unwrap_or_else(|errors| {
                    panic!(
                        "{}{} must be accepted by the IR verifier: {errors:?}",
                        primary.token(),
                        attachment.token()
                    )
                });
            } else {
                let errors =
                    result.expect_err("incompatible Primary-Attachment pairs are rejected");
                assert!(
                    errors.iter().any(|error| {
                        error.message.contains("cannot have an Attachment")
                            || error
                                .message
                                .contains("Repeat is not allowed on CALL or RETURN")
                    }),
                    "{}{} must fail the IR attachment matrix: {errors:?}",
                    primary.token(),
                    attachment.token()
                );
            }
        }
    }
}

#[test]
fn accepts_full_primary_inventory_in_scoped_programs() {
    use codegrid_model::{
        AttachmentInstruction as Attachment, PageDirection, PointerDirection,
        PrimaryInstruction as P, ShiftDirection,
    };

    let function_id = Slot::new(0).expect("zero is a valid function ID");
    let custom_id = Slot::new(0).expect("zero is a valid Custom ID");
    let fold_id = Slot::new(0).expect("zero is a valid Folded Block ID");

    let mut main_cells = vec![Cell::entry(Direction::Right)];
    main_cells.extend(
        [
            P::Direction(Direction::Up),
            P::Direction(Direction::Down),
            P::Direction(Direction::Left),
            P::Direction(Direction::Right),
            P::RandomDirection,
            P::Compare,
            P::Read,
            P::Read,
            P::Read,
            P::Read,
            P::Clear,
            P::Add,
            P::Sub,
            P::MoveRegisterPointer(PointerDirection::Left),
            P::MoveRegisterPointer(PointerDirection::Right),
            P::Output,
            P::Push,
            P::PopAdd,
            P::Decode,
            P::Encode,
            P::Call(function_id),
            P::Nand,
            P::MemoryLoad,
            P::MemoryStore,
            P::MovePage(PageDirection::Increment),
            P::MovePage(PageDirection::Decrement),
            P::Shift(ShiftDirection::Left),
            P::Shift(ShiftDirection::Right),
            P::FoldedBlock(fold_id),
            P::Custom(custom_id),
            P::Halt,
        ]
        .into_iter()
        .map(|primary| Cell::instruction(primary, None)),
    );

    // Attachment eligibility follows the existing verifier contract: an
    // encodable Primary may carry ReadCode or WriteCode; Repeat is additionally
    // forbidden on CALL and RETURN. The Full boundary cases below pin that rule.
    main_cells.extend([
        Cell::instruction(P::Shift(ShiftDirection::Left), Some(Attachment::Repeat(2))),
        Cell::instruction(P::Read, Some(Attachment::ReadCode)),
        Cell::instruction(P::Add, Some(Attachment::WriteCode)),
    ]);
    let main_width = main_cells.len();
    let mut main = board(main_cells, main_width, 1);
    let mut folded_cells = vec![None; main_width];
    folded_cells[0] = Some(P::Custom(custom_id));
    main.folded_blocks.insert(
        fold_id,
        codegrid_ir::FoldedBlock {
            prefixes: Default::default(),
            cells: folded_cells,
        },
    );

    let mut custom_main = board(
        vec![
            Cell::entry(Direction::Right),
            Cell::instruction(P::CustomReturn, None),
        ],
        2,
        1,
    );
    custom_main.folded_blocks.insert(
        fold_id,
        codegrid_ir::FoldedBlock {
            prefixes: Default::default(),
            cells: vec![Some(P::Add), None],
        },
    );

    let scoped = ScopedProgram {
        main,
        functions: BTreeMap::from([(
            function_id,
            board(
                vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(P::Return, Some(Attachment::ReadCode)),
                    Cell::instruction(P::Return, Some(Attachment::WriteCode)),
                ],
                3,
                1,
            ),
        )]),
    };
    let custom_scoped = ScopedProgram {
        main: custom_main,
        functions: BTreeMap::from([(
            function_id,
            board(
                vec![
                    Cell::entry(Direction::Right),
                    Cell::instruction(P::Return, None),
                ],
                2,
                1,
            ),
        )]),
    };
    let program = Program {
        format_version: codegrid_ir::IR_FORMAT_VERSION,
        outer: scoped,
        customs: BTreeMap::from([(
            custom_id,
            CustomDefinition {
                program: custom_scoped,
            },
        )]),
    };

    assert!(VerifiedProgram::new(program).is_ok());
}
