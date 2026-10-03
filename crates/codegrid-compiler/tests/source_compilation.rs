use codegrid_compiler::{compile, compile_with_symbols, completion_context, SymbolKey};
use codegrid_ir::CodeGridId;
use codegrid_model::{AttachmentInstruction, BoundaryMode, Direction, PrimaryInstruction};
use codegrid_syntax::{BoardPath, CodeGridPath};

#[test]
fn conditional_prefixes_preserve_references_and_fold_restrictions() {
    let program = compile("@main\n~> ?0$0 ;\n@M0 ?0+ ?1?= ?2^\n@end main").unwrap();
    let folded = &program.program().outer.main.folded_blocks[&slot(0)];
    assert_eq!(folded.prefixes.len(), 3);
    assert_eq!(
        codegrid_compiler::ProgramView::from_verified(&program)
            .outer
            .main
            .folded_blocks[&0][1]
            .as_deref(),
        Some("?1?=")
    );
    for source in [
        "~> ?1[0 ;",
        "~> ?1#0 ;",
        "~> ?1] ;",
        "@main\n~> $0 ;\n@M0 ?0+* ^ _\n@end main",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}

#[test]
fn immediate_outputs_are_complete_atoms_without_attachments() {
    compile("~> .0 .1 .2 .3 .4 .5 .6 .7 .8 .9 . ;\n").unwrap();
    for atom in [".10", ".00", ".-1", ".９", ".3*", ".3=", ".3x2"] {
        let errors = compile(&format!("~> {atom} ;\n")).unwrap_err();
        assert!(
            errors.iter().any(|e| e.code == "source.invalid_cell"),
            "{atom}: {errors:?}"
        );
    }
}

#[test]
fn stable_diagnostic_codes_are_assigned_at_validation_origins() {
    let cases = [
        ("~x\n", "source.invalid_entry"),
        ("~> _x\n", "source.invalid_cell"),
        ("/* open", "source.unterminated_comment"),
        ("~>\r_\n", "source.unsupported_line_ending"),
        ("@unknown\n", "source.unknown_directive"),
        ("@size 0x1\n~>\n", "source.zero_size"),
        ("@size 4294967296x1\n~>\n", "source.geometry_limit"),
        ("@main\n~>\n@end F0\n", "source.end_mismatch"),
        ("@main\n~>\n", "source.missing_end"),
        ("~> [0\n", "ir.undefined_function"),
        ("~> $0\n", "ir.undefined_fold"),
        ("~> #0\n", "ir.custom_reference"),
        ("~> ]\n", "source.return_scope"),
        ("~> #]\n", "source.custom_return_scope"),
    ];
    let registry = include_str!("../../../spec/codegrid-error-codes.md");
    for (source, code) in cases {
        let diagnostics = compile(source).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code == code),
            "{source:?}: {diagnostics:?}"
        );
        for diagnostic in diagnostics {
            assert!(registry.contains(&format!("`{}`", diagnostic.code)));
            assert!(source.is_char_boundary(diagnostic.span.start));
            assert!(source.is_char_boundary(diagnostic.span.end));
        }
    }
}

#[test]
fn compiles_implicit_main_with_multiple_initial_threads() {
    let program = compile("~> _\n~< _\n").expect("valid source should compile");
    let board = &program.program().outer.main;

    assert_eq!(board.width, 2);
    assert_eq!(board.height, 2);
    assert_eq!(board.cells[0].entry, Some(Direction::Right));
    assert_eq!(board.cells[2].entry, Some(Direction::Left));
}

#[test]
fn accepts_a_complete_program_without_a_final_newline() {
    compile("~> ;").expect("source specification makes the final newline optional");
}

#[test]
fn ignores_blank_lines_and_surrounding_whitespace_while_tabs_separate_cells() {
    let source = "\t \n  @main \t\n\t~>\t_\t // trailing comment\n \n @end main \t\n";
    let program = compile(source).expect("blank lines and surrounding whitespace are ignored");
    let board = &program.program().outer.main;

    assert_eq!((board.width, board.height), (2, 1));
    assert_eq!(board.cells[0].entry, Some(Direction::Right));
    assert!(board.cells[1].primary.is_none());
    assert!(board.cells[1].entry.is_none());
}

#[test]
fn compiles_explicit_board_geometry_without_padding_cells_or_rows() {
    let source = "@size 3x2\n@main\n~> + -\n_ ^ ;\n@end main\n";
    let program = compile(source).expect("an exact explicit board geometry should compile");
    let board = &program.program().outer.main;

    assert_eq!((board.width, board.height), (3, 2));
    assert_eq!(board.cells.len(), 6);
    assert_eq!(
        board.cell(0, 0).and_then(|cell| cell.entry),
        Some(Direction::Right)
    );
    assert_eq!(
        board
            .cell(1, 1)
            .and_then(|cell| cell.primary)
            .map(|primary| primary.token()),
        Some("^".to_owned())
    );
    assert_eq!(
        board
            .cell(2, 1)
            .and_then(|cell| cell.primary)
            .map(|primary| primary.token()),
        Some(";".to_owned())
    );
}

#[test]
fn rejects_missing_required_outer_and_custom_main_grids() {
    let cases = [
        (
            "outer Main without a grid",
            "@main\n@end main\n",
            "The program requires a Main board grid",
        ),
        (
            "declared Custom without a Main grid",
            "@main\n~>\n@end main\n@C0\n@end C0\n",
            "A declared Custom CodeGrid requires a Main board grid",
        ),
    ];

    for (rule, source, expected_message) in cases {
        let diagnostics = compile(source).expect_err("declared CodeGrids require Main grids");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected_message)),
            "{rule}: expected {expected_message:?}, got {diagnostics:?}"
        );
    }
}

#[test]
fn rejects_explicit_sizes_that_would_require_grid_padding() {
    let cases = [
        (
            "short row",
            "@size 3x1\n~> _\n",
            "Grid row has 2 cells but its effective width is 3.",
        ),
        (
            "too few rows",
            "@size 1x2\n~>\n",
            "Board has 1 grid rows but its effective height is 2.",
        ),
        (
            "too many rows",
            "@size 1x2\n~>\n_\n_\n",
            "Board has 3 grid rows but its effective height is 2.",
        ),
    ];

    for (rule, source, expected_message) in cases {
        let diagnostics = compile(source).expect_err("explicit dimensions must match the grid");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected_message)),
            "{rule}: expected {expected_message:?}, got {diagnostics:?}"
        );
    }
}

#[test]
fn compiles_implicit_main_with_only_board_local_folded_blocks() {
    let source = "~v _ _ _ _\n\
_  + > . _\n\
_  _ _ _ _\n\
_  _ _ _ _\n\
_  _ _ _ _\n\n\
@M0 + > _ ^ _\n";
    let program = compile(source).expect("an implicit Main may define board-local M blocks");

    assert!(program.program().outer.functions.is_empty());
    assert!(program.program().customs.is_empty());
    assert_eq!(
        program.program().outer.main.folded_blocks[&slot(0)]
            .cells
            .len(),
        5
    );
}

#[test]
fn implicit_main_accepts_qualified_folded_blocks_after_its_grid() {
    let source = "~> $9 _\n@main.M9 + _ _\n";
    let program = compile(source).expect("implicit Main allows the equivalent qualified M path");

    assert!(program
        .program()
        .outer
        .main
        .folded_blocks
        .contains_key(&slot(9)));

    let late_row = "~> $0 _\n@M0 + _ _\n_ _ _\n";
    let diagnostics = compile(late_row)
        .expect_err("the first top-level structural definition ends implicit Main rows");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("must be contiguous")));
}

#[test]
fn compiles_the_source_spec_simple_program_example_verbatim() {
    let source = "~v _ _ _ _\n\
_  + > . _\n\
_  _ _ _ _\n\
_  _ _ _ _\n\
_  _ _ _ _\n";
    let program = compile(source).expect("the normative 5x5 simple program must compile");
    let main = &program.program().outer.main;

    assert_eq!((main.width, main.height), (5, 5));
    assert_eq!(main.cells[0].entry, Some(Direction::Down));
    assert_eq!(
        main.cells[6].primary.map(|primary| primary.token()),
        Some("+".to_owned())
    );
    assert_eq!(
        main.cells[7].primary.map(|primary| primary.token()),
        Some(">".to_owned())
    );
    assert_eq!(
        main.cells[8].primary.map(|primary| primary.token()),
        Some(".".to_owned())
    );
}

#[test]
fn implicit_main_requires_an_explicit_block_when_function_or_custom_is_defined() {
    let cases = [
        ("function definition", "~> _\n@main.F0\n~v ]\n@end F0\n"),
        ("Custom definition", "~> _\n@C0\n~> ;\n@end C0\n"),
    ];

    for (definition, source) in cases {
        let diagnostics = compile(source).expect_err("F and C definitions require explicit Main");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("explicit @main block is required")),
            "{definition}: expected the explicit Main diagnostic, got {diagnostics:?}"
        );
    }
}

#[test]
fn resolves_forward_function_references_and_qualified_blocks() {
    let source = "@main\n~> [0\n@end main\n\
@main.F0\n~v ]\n@end main.F0\n";
    let program = compile(source).expect("qualified function source should compile");

    assert!(program.program().outer.functions.contains_key(&slot(0)));
}

#[test]
fn accepts_indirect_recursion_between_local_function_boards() {
    let source = "@main\n~> [0\n@end main\n\
@main.F0\n~> [1 ]\n@end F0\n\
@main.F1\n~v [0 ]\n@end F1\n";
    let program = compile(source).expect("local functions may recurse indirectly");
    let functions = &program.program().outer.functions;

    assert_eq!(
        functions[&slot(0)].cells[1]
            .primary
            .map(|primary| primary.token()),
        Some("[1".to_owned())
    );
    assert_eq!(
        functions[&slot(1)].cells[1]
            .primary
            .map(|primary| primary.token()),
        Some("[0".to_owned())
    );
}

#[test]
fn compiles_custom_return_on_custom_main() {
    let source = "@main\n~> #0\n@end\n@C0\n~> #]\n@end C0\n";
    let program = compile(source).expect("valid Custom source should compile");

    assert!(program.program().customs.contains_key(&slot(0)));
}

#[test]
fn custom_main_accepts_multiple_initial_threads() {
    let source = "@main\n~> #0\n@end\n@C0\n~> #]\n~< #]\n@end\n";
    let program = compile(source).expect("a Custom Main may define multiple Entries");

    assert_eq!(program.program().customs[&slot(0)].program.main.height, 2);
    assert_eq!(
        program.program().customs[&slot(0)].program.main.cells[0].entry,
        Some(Direction::Right)
    );
    assert_eq!(
        program.program().customs[&slot(0)].program.main.cells[2].entry,
        Some(Direction::Left)
    );
}

#[test]
fn directive_names_are_case_insensitive_but_instruction_tokens_are_not() {
    let source = "@mAiN\n~> [0\n@END MAIN\n@Main.F0\n~v ]\n@END main.f0\n";
    compile(source).expect("directive names and structural paths are case-insensitive");

    let upper_case_instruction = "~V\n";
    let diagnostics = compile(upper_case_instruction)
        .expect_err("instruction spellings must retain their specified case");
    assert!(diagnostics.iter().any(|diagnostic| {
        &upper_case_instruction[diagnostic.span.start..diagnostic.span.end] == "~V"
    }));
}

#[test]
fn accepts_case_insensitive_local_and_qualified_end_names() {
    let cases = [
        "@main\n~> #0\n@end main\n@C0\n~> #]\n@END c0\n",
        "@main\n~> [0\n@end main\n@main.F0\n~v ]\n@END MAIN.f0\n",
        "@main\n~> #0\n@end main\n@C0\n~> #]\n@C0.F0\n~v ]\n@END c0.f0\n@end C0\n",
        "@main\n~> $0\n@M0\n+ >\n@END m0\n@end main\n",
        "@main\n~> $0\n@end main\n@main.M0\n+ >\n@END MAIN.m0\n",
    ];

    for source in cases {
        compile(source).unwrap_or_else(|diagnostics| {
            panic!("Full local or qualified @end alias should compile: {diagnostics:?}")
        });
    }
}

#[test]
fn accepts_named_end_aliases_for_custom_and_function_folded_blocks() {
    let cases = [
        // Custom Main Folded Block: local final segment.
        "@main\n~> #0\n@end main\n@C0\n~> $0 #]\n@M0\n+ > _\n@END m0\n@end C0\n",
        // Custom Main Folded Block: complete qualified path.
        "@main\n~> #0\n@end main\n@C0\n~> $0 #]\n@C0.M0\n+ > _\n@END C0.m0\n@end C0\n",
        // Outer Function Folded Block: local final segment.
        "@main\n~> [0\n@end main\n@main.F0\n~> $0 ]\n@M0\n+ > _\n@END m0\n@end F0\n",
        // Outer Function Folded Block: complete qualified path.
        "@main\n~> [0\n@end main\n@main.F0\n~> $0 ]\n@end main.F0\n@main.F0.M0\n+ > _\n@END MAIN.f0.m0\n",
        // Custom Function Folded Block: local final segment.
        "@main\n~> #0\n@end main\n@C0\n~> [0 #]\n@F0\n~> $0 ]\n@M0\n+ > _\n@END m0\n@end F0\n@end C0\n",
        // Custom Function Folded Block: complete qualified path.
        "@main\n~> #0\n@end main\n@C0\n~> [0 #]\n@C0.F0\n~> $0 ]\n@end C0.F0\n@C0.F0.M0\n+ > _\n@END C0.F0.M0\n@end C0\n",
    ];

    for source in cases {
        compile(source).unwrap_or_else(|diagnostics| {
            panic!("a matching Full Folded Block @end alias should compile: {diagnostics:?}")
        });
    }
}

#[test]
fn rejects_mismatched_custom_and_function_folded_block_end_aliases() {
    let cases = [
        // Custom Main Folded Block: wrong local alias.
        "@main\n~> #0\n@end main\n@C0\n~> $0 #]\n@M0\n+ > _\n@end M1\n@end C0\n",
        // Custom Main Folded Block: wrong qualified alias.
        "@main\n~> #0\n@end main\n@C0\n~> $0 #]\n@C0.M0\n+ > _\n@end C0.M1\n@end C0\n",
        // Outer Function Folded Block: wrong local alias.
        "@main\n~> [0\n@end main\n@main.F0\n~> $0 ]\n@M0\n+ > _\n@end M1\n@end F0\n",
        // Outer Function Folded Block: wrong qualified alias.
        "@main\n~> [0\n@end main\n@main.F0\n~> $0 ]\n@end main.F0\n@main.F0.M0\n+ > _\n@end main.F0.M1\n",
        // Custom Function Folded Block: wrong local alias.
        "@main\n~> #0\n@end main\n@C0\n~> [0 #]\n@F0\n~> $0 ]\n@M0\n+ > _\n@end M1\n@end F0\n@end C0\n",
        // Custom Function Folded Block: wrong qualified alias.
        "@main\n~> #0\n@end main\n@C0\n~> [0 #]\n@C0.F0\n~> $0 ]\n@end C0.F0\n@C0.F0.M0\n+ > _\n@end C0.F0.M1\n@end C0\n",
    ];

    for source in cases {
        let diagnostics =
            compile(source).expect_err("a Folded Block close name must match its owner path");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("@end name does not match") }),
            "expected a focused mismatched close diagnostic, got {diagnostics:?}"
        );
    }
}

#[test]
fn comments_and_crlf_do_not_create_rows_and_end_names_must_match() {
    let source =
        "@MAIN\r\n~>\r\n// ignored row\r\n/* ignored\r\ncomment rows */\r\n;\r\n@END main\r\n";
    let program = compile(source).expect("comments and CRLF must preserve the two-cell grid");
    assert_eq!(program.program().outer.main.width, 1);
    assert_eq!(program.program().outer.main.height, 2);

    let mismatched_end = "@main\n~>\n@end F0\n";
    let diagnostics = compile(mismatched_end).expect_err("named @end must match its open block");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("@end name does not match")));
}

#[test]
fn compiles_board_local_folded_block_with_owner_width() {
    let source = "@main\n~> $0\n@end\n@M0 + >\n";
    let program = compile(source).expect("valid Folded Block should compile");

    assert_eq!(
        program.program().outer.main.folded_blocks[&slot(0)]
            .cells
            .len(),
        2
    );
}

#[test]
fn single_line_and_multiline_fold_definitions_compile_equivalently() {
    let single_line = "~> _ _ _ _\n@M0 + > _ ^ _\n";
    let multiline = "~> _ _ _ _\n@M0\n+ > _ ^ _\n@end\n";
    let single_line_program = compile(single_line).expect("single-line Fold should compile");
    let multiline_program = compile(multiline).expect("multiline Fold should compile");

    assert_eq!(
        single_line_program.program(),
        multiline_program.program(),
        "the two source forms must lower to the same executable program"
    );

    let main = &single_line_program.program().outer.main;
    let folded_block = &main.folded_blocks[&slot(0)];
    assert_eq!(main.width, 5);
    assert_eq!(folded_block.cells.len(), 5);
    assert_eq!(
        folded_block
            .cells
            .iter()
            .map(|cell| cell.map(|primary| primary.token()))
            .collect::<Vec<_>>(),
        vec![
            Some("+".to_owned()),
            Some(">".to_owned()),
            None,
            Some("^".to_owned()),
            None,
        ]
    );
}

#[test]
fn resolves_folded_blocks_on_their_owning_function_board() {
    let source = "@main\n~> $0\n@end main\n\
@main.M0 + >\n\
@main.F0\n~v $0\n@end F0\n\
@main.F0.M0 - <\n";
    let program = compile(source).expect("Main and F boards may each own a local M0");
    let main_m = &program.program().outer.main.folded_blocks[&slot(0)].cells;
    let function_m = &program.program().outer.functions[&slot(0)].folded_blocks[&slot(0)].cells;

    assert_eq!(
        main_m[0].map(|primary| primary.token()),
        Some("+".to_owned())
    );
    assert_eq!(
        function_m[0].map(|primary| primary.token()),
        Some("-".to_owned())
    );
}

#[test]
fn nested_and_qualified_definitions_lower_to_equivalent_programs() {
    let nested = r#"@size 5x5

@main
~v _ [0 _ _
_  + _  _ _
_  _ #0 _ _
_  _ _  _ _
_  _ _  _ ;

@M0 + > _ ^ _

@F0
~> _ _ _ _
_  + _ _ _
_  _ _ _ _
_  _ _ _ _
_  _ _ ] _
@end F0
@end main

@C0
~> _ _ _ _
_  + _ _ _
_  _ [0 _ _
_  _ _ _ _
_  _ _ #] _

@F0
~v _ _ _ _
_  - _ _ _
_  _ _ _ _
_  _ _ _ _
_  ] _ _ _
@end F0
@end C0
"#;
    let qualified = r#"@size 5x5

@main
~v _ [0 _ _
_  + _  _ _
_  _ #0 _ _
_  _ _  _ _
_  _ _  _ ;
@end main

@main.M0 + > _ ^ _

@main.F0
~> _ _ _ _
_  + _ _ _
_  _ _ _ _
_  _ _ _ _
_  _ _ ] _
@end main.F0

@C0
~> _ _ _ _
_  + _ _ _
_  _ [0 _ _
_  _ _ _ _
_  _ _ #] _
@end C0

@C0.F0
~v _ _ _ _
_  - _ _ _
_  _ _ _ _
_  _ _ _ _
_  ] _ _ _
@end C0.F0
"#;
    let nested_program = compile(nested).expect("the normative nested example should compile");
    let qualified_program =
        compile(qualified).expect("the normative qualified example should compile");

    assert_eq!(nested_program, qualified_program);
}

#[test]
fn identical_numbered_definitions_in_distinct_scopes_remain_distinct() {
    let source = "@size 2x1\n\
@main\n~> _\n@end main\n\
@main.F0\n~v ]\n@end main.F0\n\
@main.M0 + >\n\
@main.F0.M0 - <\n\
@C0\n~> _\n@end C0\n\
@C0.M0 - >\n\
@C0.F0\n~v ]\n@end C0.F0\n\
@C0.F0.M0 + <\n\
@C1\n~> _\n@end C1\n\
@C1.F0\n~v ]\n@end C1.F0\n\
@C1.F0.M0 > -\n";
    let program = compile(source).expect("same IDs in separate scopes should compile");
    let outer = &program.program().outer;
    let custom_zero = &program.program().customs[&slot(0)].program;
    let custom_one = &program.program().customs[&slot(1)].program;

    assert!(outer.functions.contains_key(&slot(0)));
    assert!(outer.main.folded_blocks.contains_key(&slot(0)));
    assert!(outer.functions[&slot(0)]
        .folded_blocks
        .contains_key(&slot(0)));
    assert!(custom_zero.main.folded_blocks.contains_key(&slot(0)));
    assert!(custom_zero.functions.contains_key(&slot(0)));
    assert!(custom_zero.functions[&slot(0)]
        .folded_blocks
        .contains_key(&slot(0)));
    assert!(custom_one.functions.contains_key(&slot(0)));
    assert!(custom_one.functions[&slot(0)]
        .folded_blocks
        .contains_key(&slot(0)));
}

#[test]
fn rejects_duplicate_custom_and_mixed_nested_qualified_fold_definitions() {
    let duplicate_custom = "@main\n~> _\n@end main\n\
@C0\n~> _\n@end C0\n\
@C0\n~> _\n@end C0\n";
    let diagnostics = compile(duplicate_custom).expect_err("a Custom path can be defined once");
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("defined more than once"))
        .expect("expected a duplicate Custom diagnostic");
    let second_custom = duplicate_custom
        .rfind("@C0")
        .expect("second declaration exists");
    assert_eq!(
        &duplicate_custom[duplicate.span.start..duplicate.span.end],
        &duplicate_custom[second_custom..second_custom + "@C0".len()]
    );
    assert_eq!(duplicate.span.start, second_custom);
    assert_eq!(duplicate.span.end, second_custom + "@C0".len());

    let duplicate_fold = "@main\n~> _\n@M0 + _\n@end main\n\
@main.M0 - _\n";
    let diagnostics = compile(duplicate_fold).expect_err("a Folded Block path can be defined once");
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("Folded Block path is defined more than once")
        })
        .expect("expected a duplicate Folded Block diagnostic");
    assert_eq!(
        &duplicate_fold[duplicate.span.start..duplicate.span.end],
        "@main.M0"
    );
    let second_fold = duplicate_fold
        .rfind("@main.M0")
        .expect("second Folded Block declaration exists");
    assert_eq!(duplicate.span.start, second_fold);
    assert_eq!(duplicate.span.end, second_fold + "@main.M0".len());
}

#[test]
fn rejects_duplicate_main_definition_at_the_second_declaration() {
    let source = "@main\n~>\n@end main\n@main\n~>\n@end main\n";
    let diagnostics = compile(source).expect_err("the program can define Main only once");
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("@main is defined more than once")
        })
        .expect("expected a duplicate Main diagnostic");
    let second_main = source
        .rfind("@main")
        .expect("second Main declaration exists");

    assert_eq!(
        &source[duplicate.span.start..duplicate.span.end],
        &source[second_main..second_main + "@main".len()]
    );
    assert_eq!(duplicate.span.start, second_main);
    assert_eq!(duplicate.span.end, second_main + "@main".len());
}

#[test]
fn rejects_custom_definitions_nested_inside_main_or_another_custom() {
    let cases = [
        ("Main", "@main\n~>\n@C0\n~> #]\n@end C0\n@end main\n", "@C0"),
        (
            "Custom",
            "@main\n~>\n@end main\n@C0\n~>\n@C1\n~> #]\n@end C1\n@end C0\n",
            "@C1",
        ),
    ];

    for (owner, source, expected_span) in cases {
        let diagnostics =
            compile(source).expect_err("a Custom definition cannot be nested in another block");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .message
                    .contains("A Main or Custom CodeGrid definition cannot be nested")
            })
            .unwrap_or_else(|| {
                panic!(
                    "inside {owner}: expected a nested-definition diagnostic, got {diagnostics:?}"
                )
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "inside {owner}: diagnostic should identify the nested definition"
        );
    }
}

#[test]
fn compiles_highest_numbered_custom_function_and_folded_block_paths() {
    let source = "@main\n~> [9 $9 #9\n@end main\n\
@main.F9\n~v ]\n@end main.F9\n\
@main.M9 + _ _ _\n\
@C9\n~> [9 $9 #]\n\
@F9\n~v ]\n@end F9\n\
@M9 - _ _ _\n\
@F9.M9 + _\n\
@end C9\n";
    let program = compile(source).expect("the full structural namespace includes slot 9");
    let outer = &program.program().outer;
    let custom = &program.program().customs[&slot(9)].program;

    assert!(outer.functions.contains_key(&slot(9)));
    assert!(outer.main.folded_blocks.contains_key(&slot(9)));
    assert!(custom.functions.contains_key(&slot(9)));
    assert!(custom.main.folded_blocks.contains_key(&slot(9)));
    assert!(custom.functions[&slot(9)]
        .folded_blocks
        .contains_key(&slot(9)));
}

#[test]
fn compiles_specified_complete_cell_tokens_without_splitting_invalid_tokens() {
    let source = "~> $<x2 ,<* +=\n";
    let program = compile(source).expect("the specified complete cell tokens are valid");
    let cells = &program.program().outer.main.cells;

    assert_eq!(
        cells[1].primary.map(|primary| primary.token()),
        Some("$<".to_owned())
    );
    assert_eq!(cells[1].attachment, Some(AttachmentInstruction::Repeat(2)));
    assert_eq!(
        cells[2].primary.map(|primary| primary.token()),
        Some(",<".to_owned())
    );
    assert_eq!(cells[2].attachment, Some(AttachmentInstruction::ReadCode));
    assert_eq!(
        cells[3].primary.map(|primary| primary.token()),
        Some("+".to_owned())
    );
    assert_eq!(cells[3].attachment, Some(AttachmentInstruction::WriteCode));

    for token in ["#00", "++"] {
        let invalid_source = format!("~> {token}\n");
        let diagnostics =
            compile(&invalid_source).expect_err("invalid whole cell tokens must not be split");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                &invalid_source[diagnostic.span.start..diagnostic.span.end] == token
            }),
            "{token}: expected a diagnostic spanning the whole invalid token, got {diagnostics:?}"
        );
    }
}

#[test]
fn compiles_call_read_and_write_code_attachments_from_source() {
    let source = "@main\n~> [0* [0=\n@end main\n@main.F0\n~> ]\n@end main.F0\n";
    let program = compile(source).expect("CALL may carry either non-Repeat code attachment");
    let cells = &program.program().outer.main.cells;

    assert_eq!(cells[1].primary, Some(PrimaryInstruction::Call(slot(0))));
    assert_eq!(cells[1].attachment, Some(AttachmentInstruction::ReadCode));
    assert_eq!(cells[2].primary, Some(PrimaryInstruction::Call(slot(0))));
    assert_eq!(cells[2].attachment, Some(AttachmentInstruction::WriteCode));
}

#[test]
fn compiles_every_resolved_attachment_pair_for_each_encodable_primary() {
    // ReadCode and WriteCode apply to every encodable Primary; Repeat excludes Call and Return.
    for primary in PrimaryInstruction::source_forms() {
        if !primary.is_encodable() {
            continue;
        }
        for attachment in AttachmentInstruction::ALL {
            if matches!(attachment, AttachmentInstruction::Repeat(_))
                && matches!(
                    primary,
                    PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                )
            {
                continue;
            }

            let token = format!("{}{}", primary.token(), attachment.token());
            let source = source_for_primary_cell(primary, &token);
            compile(&source).unwrap_or_else(|diagnostics| {
                panic!("{token:?} is a resolved Full Primary-Attachment pair: {diagnostics:?}")
            });
        }
    }
}

#[test]
fn compiles_every_primary_in_the_canonical_full_inventory() {
    for primary in PrimaryInstruction::source_forms() {
        let token = primary.token();
        let source = source_for_primary_cell(primary, &token);
        compile(&source).unwrap_or_else(|diagnostics| {
            panic!("Full Primary {token:?} must compile in its valid scope: {diagnostics:?}")
        });
    }
}

fn source_for_primary_cell(primary: PrimaryInstruction, cell: &str) -> String {
    match primary {
        PrimaryInstruction::Return => {
            format!("@main\n~> _\n@end main\n@main.F0\n~> {cell}\n@end main.F0\n")
        }
        PrimaryInstruction::Call(slot) => format!(
            "@main\n~> {cell}\n@end main\n@main.F{}\n~> ]\n@end main.F{}\n",
            slot.get(),
            slot.get()
        ),
        PrimaryInstruction::FoldedBlock(slot) => {
            format!("@main\n~> {cell}\n@end main\n@M{} + _\n", slot.get())
        }
        PrimaryInstruction::Custom(slot) => format!(
            "@main\n~> {cell}\n@end main\n@C{}\n~> #]\n@end C{}\n",
            slot.get(),
            slot.get()
        ),
        PrimaryInstruction::CustomReturn => {
            format!("@main\n~> _\n@end main\n@C0\n~> {cell}\n@end C0\n")
        }
        _ => format!("@main\n~> {cell}\n@end main\n"),
    }
}

#[test]
fn validates_function_and_custom_main_entry_count_boundaries() {
    let invalid_cases = [
        (
            "function without an Entry",
            "@main\n~> [0\n@end\n@main.F0\n_ _\n@end F0\n",
            "exactly one Entry marker; found 0",
            "@main.F0",
        ),
        (
            "function with multiple Entries",
            "@main\n~> [0\n@end\n@main.F0\n~> ~v\n@end F0\n",
            "exactly one Entry marker; found 2",
            "@main.F0",
        ),
        (
            "Custom Main without an Entry",
            "@main\n~> #0\n@end\n@C0\n_\n@end C0\n",
            "at least one Entry marker",
            "@C0",
        ),
    ];

    for (rule, source, expected_message, expected_span) in invalid_cases {
        let diagnostics = compile(source).expect_err("invalid Entry counts must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(expected_message))
            .unwrap_or_else(|| {
                panic!("{rule}: expected {expected_message:?}, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "{rule}: diagnostic should identify the affected board"
        );
    }
}

#[test]
fn rejects_attachments_on_entry_markers() {
    for token in ["~>x2", "~>*", "~>="] {
        let source = format!("{token}\n");
        let diagnostics = compile(&source).expect_err("Entry markers cannot have Attachments");
        let diagnostic = diagnostics
            .first()
            .expect("expected an Entry Attachment diagnostic");
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            token,
            "the complete invalid Entry token should be highlighted"
        );
    }
}

#[test]
fn validates_missing_extra_and_mismatched_end_closures() {
    let cases = [
        (
            "missing closing directive",
            "@main\n~>\n",
            "missing its closing @end",
            "@main",
        ),
        (
            "extra closing name",
            "@main\n~>\n@end main F0\n",
            "@end accepts at most one closing name",
            "F0",
        ),
        (
            "mismatched closing name",
            "@main\n~>\n@end F0\n",
            "@end name does not match",
            "F0",
        ),
        (
            "closing directive without an open block",
            "@end\n~>\n",
            "@end has no open definition",
            "@end",
        ),
    ];

    for (rule, source, expected_message, expected_span) in cases {
        let diagnostics = compile(source).expect_err("invalid @end usage must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(expected_message))
            .unwrap_or_else(|| {
                panic!("{rule}: expected {expected_message:?}, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "{rule}: diagnostic should point at the relevant @end text"
        );
    }
}

#[test]
fn accepts_every_primary_category_allowed_in_an_outer_folded_block() {
    let allowed = [
        "_", "^", "v", "<", ">", "??", "?=", ",<", ",>", ",^", ",v", "!", "+", "-", "{", "}", ".",
        "(", ")", "&", "%", "$&", "$(", "$)", "$+", "$-", "$<", "$>", ";", "#0",
    ];
    let mut main_cells = vec!["~>".to_owned(), "$0".to_owned()];
    main_cells.resize(allowed.len(), "_".to_owned());
    let source = format!(
        "@main\n{}\n@end main\n@M0 {}\n@C0\n~> ;\n@end C0\n",
        main_cells.join(" "),
        allowed.join(" ")
    );
    let program = compile(&source).expect("all normative outer M Primary categories are allowed");
    let cells = &program.program().outer.main.folded_blocks[&slot(0)].cells;

    assert_eq!(cells.len(), allowed.len());
    for (index, (cell, token)) in cells.iter().zip(allowed).enumerate() {
        let actual = cell.map(|primary| primary.token());
        let expected = (token != "_").then(|| token.to_owned());
        assert_eq!(actual, expected, "incorrect M cell at index {index}");
    }
}

#[test]
fn rejects_every_forbidden_folded_block_form_and_custom_owned_calls() {
    let invalid_outer_forms = ["$0", "[0", "]", "#]", "~>", "+*"];
    for token in invalid_outer_forms {
        let source = format!("@main\n~> $0\n@end main\n@M0 {token} _\n");
        let diagnostics = compile(&source).expect_err("forbidden M forms must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .message
                    .contains("not allowed inside this Folded Block")
                    || diagnostic.message.contains("Attachments are not allowed")
                    || diagnostic.message.contains("Entry markers are not allowed")
            })
            .unwrap_or_else(|| {
                panic!("{token}: expected an M placement diagnostic, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            token,
            "{token}: diagnostic should highlight the forbidden form"
        );
    }

    let custom_owned_m = "@main\n~> #0\n@end main\n\
@C0\n~> $0\n@end C0\n\
@C0.M0 #0 _\n";
    let diagnostics =
        compile(custom_owned_m).expect_err("a Custom-owned M cannot call a Custom definition");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("not allowed inside this Folded Block")
        })
        .expect("a Custom call in Custom-owned M must be diagnosed");
    assert_eq!(
        &custom_owned_m[diagnostic.span.start..diagnostic.span.end],
        "#0"
    );
}

#[test]
fn accepts_halt_returns_custom_returns_and_custom_calls_in_their_valid_scopes() {
    let source = "@main\n~> #0 $0 ;\n\
@F0\n~v #0 ; ]\n@end F0\n\
@end main\n\
@main.M0 #0 ; _ _\n\
@C0\n~> $0 #] ;\n\
@M0 ; _ _ _\n\
@F0\n~v ; ]\n@end F0\n\
@end C0\n";
    compile(source).expect("exit and Custom-call forms are legal in these scopes");
}

#[test]
fn rejects_exit_and_custom_call_forms_in_invalid_scopes() {
    let cases = [
        (
            "CUSTOM_RETURN on outer Main",
            "~> #]\n",
            "CUSTOM_RETURN is allowed only directly on a Custom Main board",
            "#]",
        ),
        (
            "CUSTOM_RETURN on outer F",
            "@main\n~> [0\n@F0\n~> #] ]\n@end F0\n@end main\n",
            "CUSTOM_RETURN is allowed only directly on a Custom Main board",
            "#]",
        ),
        (
            "CUSTOM_RETURN on Custom F",
            "@main\n~> #0\n@end main\n@C0\n~> [0\n@F0\n~v #] ]\n@end F0\n@end C0\n",
            "CUSTOM_RETURN is allowed only directly on a Custom Main board",
            "#]",
        ),
        (
            "RETURN on Custom Main",
            "@main\n~> #0\n@end main\n@C0\n~> ]\n@end C0\n",
            "RETURN is allowed only on a function board",
            "]",
        ),
        (
            "RETURN in outer M",
            "@main\n~> $0\n@end main\n@M0 ] _\n",
            "not allowed inside this Folded Block",
            "]",
        ),
        (
            "Custom call on Custom F",
            "@main\n~> #0\n@end main\n@C0\n~> [0\n@F0\n~v #0 ]\n@end F0\n@end C0\n",
            "cannot be called from inside a Custom",
            "#0",
        ),
    ];

    for (rule, source, expected_message, expected_span) in cases {
        let diagnostics =
            compile(source).expect_err("the placement is forbidden by the source spec");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(expected_message))
            .unwrap_or_else(|| {
                panic!("{rule}: expected {expected_message:?}, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "{rule}: diagnostic should identify the misplaced token"
        );
    }
}

#[test]
fn marks_a_source_level_self_call_with_a_navigation_only_return_path() {
    let source = "@main\n~> [0\n@end main\n\
@main.F0\n~> [0 _ ]\n@end main.F0\n";
    let program = compile(source).expect("valid tail-recursive source should compile");

    assert!(program.is_tail_call(CodeGridId::Outer, slot(0), 1, BoundaryMode::Exit,));
    assert!(program.is_tail_call(CodeGridId::Outer, slot(0), 1, BoundaryMode::Wrap,));
}

#[test]
fn conservatively_disables_tail_calls_in_functions_with_conditional_navigation() {
    let source = "@size 3x3\n\
@main\n~> [0 _\n_ _ _\n_ _ _\n@end main\n\
@main.F0\n~> ?0v v\nv [0 <\n] < _\n@end main.F0\n";
    let program = compile(source).expect("branched navigation-only returns should compile");

    assert!(!program.is_tail_call(CodeGridId::Outer, slot(0), 4, BoundaryMode::Exit));
    assert!(!program.is_tail_call(CodeGridId::Outer, slot(0), 4, BoundaryMode::Wrap));
}

#[test]
fn marks_tail_recursive_calls_in_custom_function_codegrids() {
    let source = "@main\n~> #0\n@end main\n\
@C0\n~>\n@F0\n~> [0 _ ]\n@end F0\n@end C0\n";
    let program = compile(source).expect("valid Custom tail-recursive source should compile");
    let custom = CodeGridId::Custom(slot(0));

    assert!(program.is_tail_call(custom, slot(0), 1, BoundaryMode::Exit));
    assert!(program.is_tail_call(custom, slot(0), 1, BoundaryMode::Wrap));
}

#[test]
fn tail_call_navigation_proof_observes_exit_and_wrap_boundaries() {
    let source = "@size 4x1\n\
@main\n~> [0 _ _\n@end main\n\
@main.F0\n] ~> _ [0\n@end main.F0\n";
    let program = compile(source).expect("boundary-sensitive navigation should compile");

    assert!(!program.is_tail_call(CodeGridId::Outer, slot(0), 3, BoundaryMode::Exit));
    assert!(program.is_tail_call(CodeGridId::Outer, slot(0), 3, BoundaryMode::Wrap));
}

#[test]
fn compiles_repeat_attachments_x3_x4_and_x5_as_single_cells() {
    let program = compile("~> +x3 -x4 .x5\n").expect("valid Repeat attachments should compile");
    let cells = &program.program().outer.main.cells;

    assert_eq!(cells[1].attachment, Some(AttachmentInstruction::Repeat(3)));
    assert_eq!(cells[2].attachment, Some(AttachmentInstruction::Repeat(4)));
    assert_eq!(cells[3].attachment, Some(AttachmentInstruction::Repeat(5)));
    assert_eq!(
        cells[1].primary.map(|primary| primary.token()),
        Some("+".to_owned())
    );
    assert_eq!(
        cells[2].primary.map(|primary| primary.token()),
        Some("-".to_owned())
    );
    assert_eq!(
        cells[3].primary.map(|primary| primary.token()),
        Some(".".to_owned())
    );
}

#[test]
fn does_not_mark_a_source_level_self_call_with_a_side_effecting_return_path() {
    let source = "@main\n~> [0\n@end main\n\
@main.F0\n~> [0 . ]\n@end main.F0\n";
    let program = compile(source).expect("valid recursive source should compile");

    assert!(!program.is_tail_call(CodeGridId::Outer, slot(0), 1, BoundaryMode::Exit,));
}

#[test]
fn applies_inherited_and_inner_size_attributes() {
    let source = "@size 2x1\n\
@main\n~> _\n@end\n\
@C0\n@size 3x1\n~> _ _\n@end\n";
    let program = compile(source).expect("valid inherited sizes should compile");

    assert_eq!(program.program().outer.main.width, 2);
    assert_eq!(program.program().customs[&slot(0)].program.main.width, 3);
}

#[test]
fn size_between_rows_applies_to_the_complete_lexical_scope() {
    let source = "@main\n~> _\n@size 2x2\n_ _\n@end main\n";
    let program = compile(source).expect("@size does not split a board's row sequence");
    let main = &program.program().outer.main;

    assert_eq!((main.width, main.height), (2, 2));
    assert_eq!(main.cells.len(), 4);
}

#[test]
fn rejects_dimensions_and_total_cells_above_the_portable_source_limit() {
    for source in ["@size 4294967296x1\n~>\n", "@size 65536x65536\n~>\n"] {
        assert!(
            compile(source).is_err(),
            "portable Full geometry must reject {source:?}"
        );
    }
}

#[test]
fn function_board_size_overrides_the_inherited_codegrid_size() {
    let source = "@main\n@size 4x1\n~> [0 _ _\n\
@F0\n@size 2x1\n~v ]\n@end F0\n@end main\n";
    let program = compile(source).expect("a function board may override inherited dimensions");

    assert_eq!(program.program().outer.main.width, 4);
    assert_eq!(program.program().outer.functions[&slot(0)].width, 2);
}

#[test]
fn rejects_repeated_and_invalid_size_attributes_in_each_scope() {
    let repeated_scopes = [
        (
            "CodeGrid scope",
            "@main\n@size 1x1\n@size 1x1\n~>\n@end main\n",
        ),
        (
            "board scope",
            "@main\n~> [0\n@F0\n@size 1x1\n@size 1x1\n~v\n@end F0\n@end main\n",
        ),
    ];

    for (scope, source) in repeated_scopes {
        let diagnostics = compile(source).expect_err("same-scope @size declarations are invalid");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("@size is repeated"))
            .unwrap_or_else(|| {
                panic!("{scope}: expected a repeated-scope diagnostic, got {diagnostics:?}")
            });
        let second_size = source.rfind("@size").expect("second @size exists");
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "@size");
        assert_eq!(
            diagnostic.span.start, second_size,
            "{scope}: wrong occurrence"
        );
        assert_eq!(diagnostic.span.end, second_size + "@size".len());
    }

    let folded_scope = "@main\n~> $0\n@end main\n@M0\n@size 2x1\n+ _\n@end M0\n";
    let diagnostics = compile(folded_scope).expect_err("@size is not valid in a Folded Block");
    let invalid_scope = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("Only the Folded Block row and its @end are allowed")
        })
        .expect("expected a directive-scope diagnostic");
    assert_eq!(
        &folded_scope[invalid_scope.span.start..invalid_scope.span.end],
        "@size"
    );

    let path_attribute = "@main.F0.size 1x1\n";
    let diagnostics = compile(path_attribute).expect_err("attributes cannot be part of a path");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("Invalid CodeGrid structural path")
                || diagnostic.message.contains("Invalid structural path")
        })
        .expect("expected an invalid structural path diagnostic");
    assert_eq!(
        &path_attribute[diagnostic.span.start..diagnostic.span.end],
        "@main.F0.size"
    );
}

#[test]
fn program_size_applies_when_declared_after_explicit_main() {
    let source = "@main\n~>\n@end main\n@size 2x1\n";
    let diagnostics = compile(source).expect_err("program-scope size must apply to Main");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("effective width")));
}

#[test]
fn late_program_size_applies_to_customs_without_a_local_override() {
    let source = "@main\n@size 1x1\n~>\n@end main\n\
@C0\n~>\n@end C0\n@size 2x1\n";
    let diagnostics = compile(source).expect_err("program size must be inherited by Custom");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("effective width")));
}

#[test]
fn codegrid_size_applies_to_previously_declared_function_boards() {
    let source = "@main\n~> _ _\n@F0\n~v ]\n@end F0\n@size 3x1\n@end main\n";
    let diagnostics = compile(source).expect_err("CodeGrid size must apply to all its boards");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("effective width")));
}

#[test]
fn rejects_non_rectangular_boards() {
    let source = "~> _\n+\n";
    let diagnostics = compile(source).expect_err("ragged rows must be rejected");

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("effective width"))
        .expect("expected a non-rectangular row diagnostic");
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "+");
}

#[test]
fn rejects_board_rows_after_nested_definitions_start() {
    let source = "@main\n~> _\n@F0\n~v ]\n@end\n_ _\n@end\n";
    let diagnostics = compile(source).expect_err("board rows must precede nested definitions");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("must be contiguous")));
}

#[test]
fn rejects_undefined_function_references_with_cell_span() {
    let source = "~> [0\n";
    let diagnostics = compile(source).expect_err("undefined function must be rejected");

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("undefined function"))
        .expect("expected an undefined-function diagnostic");
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "[0");
}

#[test]
fn references_do_not_resolve_across_codegrid_or_board_scopes() {
    let cases = [
        (
            "outer F cannot resolve to a Custom F",
            "@main\n~> [0\n@end main\n\
@C0\n~> #]\n@end C0\n\
@C0.F0\n~v ]\n@end C0.F0\n",
            "undefined function",
            "[0",
        ),
        (
            "Custom F cannot resolve to an outer F",
            "@main\n~> #0\n@end main\n\
@main.F0\n~v ]\n@end main.F0\n\
@C0\n~> [0 #]\n@end C0\n",
            "undefined function",
            "[0",
        ),
        (
            "a Custom F cannot resolve to a same-numbered F in another Custom",
            "@main\n~> #0\n@end main\n\
@C0\n~> #]\n@F0\n~v ]\n@end F0\n@end C0\n\
@C1\n~> [0 #]\n@end C1\n",
            "undefined function",
            "[0",
        ),
        (
            "a Custom Main cannot resolve to another Custom Main's M",
            "@main\n~> #0\n@end main\n\
@C0\n~> $0 #]\n@end C0\n\
@C1\n~> _ #]\n@M0 + _ _\n@end C1\n",
            "Folded Block reference is undefined",
            "$0",
        ),
        (
            "a Custom F cannot resolve to a same-numbered M in another Custom F",
            "@main\n~> #0\n@end main\n\
@C0\n~> #]\n@F0\n~v $0 ]\n@end F0\n@end C0\n\
@C1\n~> #]\n@F0\n~v ] _\n@M0 + _ _\n@end F0\n@end C1\n",
            "Folded Block reference is undefined",
            "$0",
        ),
        (
            "Main M cannot resolve to a function-owned M",
            "@main\n~> $0\n@end main\n\
@main.F0\n~v ]\n@end main.F0\n\
@main.F0.M0 + _\n",
            "Folded Block reference is undefined",
            "$0",
        ),
        (
            "function-owned M cannot resolve to Main M",
            "@main\n~> _\n@end main\n\
@main.M0 + _\n\
@main.F0\n~v $0\n@end main.F0\n",
            "Folded Block reference is undefined",
            "$0",
        ),
    ];

    for (rule, source, expected_message, expected_token) in cases {
        let diagnostics = compile(source).expect_err("cross-scope references must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(expected_message))
            .unwrap_or_else(|| {
                panic!("{rule}: expected {expected_message:?}, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_token,
            "{rule}: diagnostic should identify the out-of-scope reference"
        );
    }
}

#[test]
fn duplicate_definition_diagnostic_points_at_the_second_definition() {
    let source = "@main\n~> _\n@end main\n\
@main.F0\n~v ]\n@end main.F0\n\
@main.F0\n~v ]\n@end main.F0\n";
    let diagnostics = compile(source).expect_err("duplicate structural definitions are invalid");
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("structural board path is defined more than once")
        })
        .expect("expected a duplicate board diagnostic");
    let second_definition = source.rfind("@main.F0").expect("second definition exists");

    assert_eq!(
        &source[duplicate.span.start..duplicate.span.end],
        &source[second_definition..second_definition + "@main.F0".len()]
    );
}

#[test]
fn entry_count_diagnostic_points_at_the_board_definition() {
    let source = "@main\n~> [0\n@end main\n@main.F0\n_ _\n@end main.F0\n";
    let diagnostics = compile(source).expect_err("a function must have one Entry marker");
    let entry_count = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("exactly one Entry marker; found 0")
        })
        .expect("expected a function Entry-count diagnostic");

    assert_eq!(
        &source[entry_count.span.start..entry_count.span.end],
        "@main.F0"
    );
}

#[test]
fn illegal_folded_block_diagnostic_points_at_the_primary_token() {
    let source = "@main\n~> $0\n@end main\n@M0 [0 _\n";
    let diagnostics = compile(source).expect_err("CALL is forbidden inside a Folded Block");
    let illegal_primary = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("not allowed inside this Folded Block")
        })
        .expect("expected an illegal Folded Block content diagnostic");

    assert_eq!(
        &source[illegal_primary.span.start..illegal_primary.span.end],
        "[0"
    );
}

#[test]
fn rejects_custom_calls_inside_custom_code() {
    let source = "@main\n~> #0\n@end\n@C0\n~> #0 #]\n@end\n";
    let diagnostics = compile(source).expect_err("nested Custom calls must be rejected");

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("cannot be called from inside a Custom")
        })
        .expect("expected a nested Custom call diagnostic");
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "#0");
}

#[test]
fn undefined_custom_call_inside_folded_block_has_exact_source_span() {
    let source = "@main\n~> $0\n@end main\n@main.M0 + #0\n";
    let diagnostics = compile(source).expect_err("Folded Block Custom calls must be resolved");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("undefined Custom"))
        .expect("expected an undefined Custom diagnostic");

    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "#0");
}

#[test]
fn rejects_qualified_custom_definitions_without_a_declared_custom() {
    let cases = [
        (
            "orphan Custom function",
            "@C0.F0\n~>\n@end C0.F0\n",
            "@C0.F0",
        ),
        ("orphan Custom Main Folded Block", "@C0.M0 +\n", "@C0.M0"),
        (
            "orphan Custom function Folded Block",
            "@C0.F0.M0 +\n",
            "@C0.F0.M0",
        ),
    ];

    for (rule, definition, expected_span) in cases {
        let source = format!("@main\n~>\n@end main\n{definition}");
        let diagnostics = compile(&source).expect_err("qualified definitions require a Custom");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .message
                    .contains("qualified definition refers to undefined Custom C0")
            })
            .unwrap_or_else(|| {
                panic!("{rule}: expected an orphaned Custom diagnostic, got {diagnostics:?}")
            });

        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "{rule}: diagnostic should identify the orphaned definition"
        );
    }
}

#[test]
fn rejects_the_required_source_validation_failures() {
    let cases = [
        (
            "malformed cell",
            "~x\n",
            "Entry markers must be exactly",
            "~x",
        ),
        (
            "mismatched board size",
            "@size 2x1\n~> _ _\n",
            "effective width",
            "~> _ _",
        ),
        (
            "mismatched board height",
            "@size 2x2\n~> _ _\n",
            "effective height",
            "2x2",
        ),
        (
            "Main without an Entry",
            "_\n",
            "at least one Entry marker",
            "_",
        ),
        (
            "duplicate function definition",
            "@main\n~> _\n@end\n@main.F0\n~> ]\n@end\n@main.F0\n~> ]\n@end\n",
            "defined more than once",
            "@main.F0",
        ),
        (
            "undefined Custom target",
            "~> #0\n",
            "reference an undefined Custom",
            "#0",
        ),
        (
            "undefined Folded Block target",
            "~> $0\n",
            "Folded Block reference is undefined",
            "$0",
        ),
        (
            "invalid structural path",
            "@main\n~> _\n@end\n@main.C0\n~> _\n@end\n",
            "Invalid CodeGrid structural path",
            "@main.C0",
        ),
        (
            "RETURN on Main",
            "~> ]\n",
            "RETURN is allowed only on a function board",
            "]",
        ),
        (
            "F board without exactly one Entry",
            "@main\n~> _\n@end\n@main.F0\n_\n@end\n",
            "exactly one Entry marker",
            "@main.F0",
        ),
        (
            "M width differs from its owner",
            "@main\n~> _ _\n@end\n@main.M0 +\n",
            "must match owner width",
            "@main.M0 +",
        ),
        (
            "CALL inside M",
            "@main\n~> _\n@end\n@main.M0 [0 _\n@main.F0\n~> ]\n@end\n",
            "not allowed inside this Folded Block",
            "[0",
        ),
        (
            "Attachment inside M",
            "@main\n~> _\n@end\n@main.M0 +* _\n",
            "Attachments are not allowed inside a Folded Block",
            "+*",
        ),
        (
            "multiple Attachments",
            "~> +*x2\n",
            "invalid Primary-Attachment combination",
            "+*x2",
        ),
        (
            "detached Attachment",
            "~> + x2\n",
            "Unknown or malformed Full cell token",
            "x2",
        ),
        (
            "Attachment on HALT",
            "~> ;x2\n",
            "invalid Primary-Attachment combination",
            ";x2",
        ),
        (
            "duplicate same-scope size attributes",
            "@size 1x1\n@size 1x1\n~>\n",
            "@size is repeated in the program scope",
            "@size",
        ),
    ];

    for (rule, source, expected_message, expected_span) in cases {
        let diagnostics =
            compile(source).expect_err("source violating a required static rule must be rejected");
        let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| diagnostic.message.contains(expected_message))
                .unwrap_or_else(|| {
                    panic!(
                        "{rule}: expected diagnostic containing {expected_message:?}, got {diagnostics:?}"
                    )
                });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_span,
            "{rule}: diagnostic should highlight the relevant source text"
        );
        if rule.starts_with("duplicate") {
            let expected_start = source
                .rfind(expected_span)
                .expect("the final duplicate declaration should be present");
            assert_eq!(
                diagnostic.span.start, expected_start,
                "{rule}: wrong occurrence"
            );
            assert_eq!(
                diagnostic.span.end,
                expected_start + expected_span.len(),
                "{rule}: wrong span end"
            );
        }
    }
}

#[test]
fn rejects_each_standalone_primary_prefix_at_its_source_span() {
    for token in ["#", "$", ",", "[", "x"] {
        let source = format!("~> {token}\n");
        let diagnostics = compile(&source)
            .expect_err("a Primary prefix without its required suffix must be rejected");

        assert_eq!(
            diagnostics.len(),
            1,
            "standalone token {token:?} should produce one focused diagnostic: {diagnostics:?}"
        );
        let diagnostic = &diagnostics[0];
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            token,
            "standalone token {token:?} should be highlighted exactly"
        );
    }
}

#[test]
fn invalid_comment_and_size_diagnostics_point_to_exact_source_spans() {
    let cases = [
        (
            "nested block comment",
            "~> /* outer /* nested */ */\n",
            "Block comments cannot nest.",
            "/*",
        ),
        (
            "unterminated block comment",
            "~> /* unfinished",
            "Unterminated block comment.",
            "/* unfinished",
        ),
        (
            "bare carriage return",
            "~>\r_\n",
            "Bare carriage return",
            "\r",
        ),
        ("malformed size", "@size 2X3\n~>\n", "WIDTHxHEIGHT", "2X3"),
        ("zero size", "@size 0x1\n~>\n", "greater than zero", "0x1"),
        (
            "overflowing size",
            "@size 999999999999999999999999999999999999x1\n~>\n",
            "portable source range",
            "999999999999999999999999999999999999x1",
        ),
        (
            "malformed instruction token",
            "~> _ ~V\n",
            "Entry markers must be exactly",
            "~V",
        ),
        (
            "undefined function reference",
            "~> [0\n",
            "undefined function",
            "[0",
        ),
        (
            "undefined Custom reference",
            "~> #0\n",
            "undefined Custom",
            "#0",
        ),
        ("misplaced RETURN", "~> ]\n", "RETURN is allowed only", "]"),
    ];

    for (rule, source, expected_message, expected_text) in cases {
        let diagnostics = compile(source).expect_err("invalid source must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(expected_message))
            .unwrap_or_else(|| {
                panic!("{rule}: expected {expected_message:?}, got {diagnostics:?}")
            });
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected_text,
            "{rule}: diagnostic must highlight the offending source"
        );
    }
}

#[test]
fn block_comments_accept_lf_and_crlf_but_reject_bare_cr_at_its_exact_span() {
    for line_ending in ["\n", "\r\n"] {
        let source =
            format!("@main\n~> /* comment starts{line_ending}comment continues */ ;\n@end main\n");
        compile(&source).expect("LF and CRLF inside block comments must be accepted");
    }

    let source = "@main\n~> /* comment starts\rcomment continues */ ;\n@end main\n";
    let carriage_return = source.find('\r').expect("fixture contains a bare CR");
    let diagnostics = compile(source).expect_err("bare CR inside a block comment is invalid");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Bare carriage return"))
        .expect("expected a bare-CR diagnostic");

    assert_eq!(
        diagnostic.span,
        codegrid_syntax::Span::new(carriage_return, carriage_return + 1)
    );
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "\r");
}

#[test]
fn exposes_resolved_definition_and_reference_spans_for_editor_features() {
    let source = "@main\n~> [0 $0 #0\n@end main\n\
@main.M0 + > #0 ^\n\
@main.F0\n~> ]\n@end main.F0\n\
@C0\n~> [0 $0\n@F0\n~> ]\n@end F0\n\
@M0 + > _\n@end C0\n";
    let compilation = compile_with_symbols(source).expect("valid symbols should compile");
    let function = SymbolKey::Function {
        codegrid: CodeGridPath::Main,
        slot: slot(0),
    };
    let folded_block = SymbolKey::FoldedBlock {
        board: BoardPath::Main(CodeGridPath::Main),
        slot: slot(0),
    };
    let custom = SymbolKey::Custom(slot(0));
    let custom_function = SymbolKey::Function {
        codegrid: CodeGridPath::Custom(slot(0)),
        slot: slot(0),
    };
    let custom_folded_block = SymbolKey::FoldedBlock {
        board: BoardPath::Main(CodeGridPath::Custom(slot(0))),
        slot: slot(0),
    };

    assert_eq!(
        compilation
            .symbols
            .definition(function)
            .map(|item| item.key),
        Some(function)
    );
    assert_eq!(
        compilation
            .symbols
            .definition(folded_block)
            .map(|item| item.key),
        Some(folded_block)
    );
    assert_eq!(
        compilation.symbols.definition(custom).map(|item| item.key),
        Some(custom)
    );
    assert!(compilation.symbols.definition(custom_function).is_some());
    assert!(compilation
        .symbols
        .definition(custom_folded_block)
        .is_some());
    assert_eq!(custom_function.qualified_name(), "@C0.F0");
    assert_eq!(custom_folded_block.qualified_name(), "@C0.M0");
    let function_reference_offset = source.find("[0").expect("source includes the call");
    assert_eq!(
        compilation.symbols.symbol_at(function_reference_offset),
        Some(function)
    );

    let references = compilation.symbols.references();
    assert_eq!(references.len(), 6);
    assert_eq!(
        &source[references[0].span.start..references[0].span.end],
        "[0"
    );
    assert_eq!(references[0].key, function);
    assert_eq!(
        &source[references[1].span.start..references[1].span.end],
        "$0"
    );
    assert_eq!(references[1].key, folded_block);
    assert_eq!(
        &source[references[2].span.start..references[2].span.end],
        "#0"
    );
    assert_eq!(references[2].key, custom);
    assert_eq!(
        &source[references[3].span.start..references[3].span.end],
        "#0"
    );
    assert_eq!(references[3].key, custom);
    assert_eq!(
        compilation.symbols.symbol_at(references[3].span.start),
        Some(custom)
    );
    assert_eq!(references[4].key, custom_function);
    assert_eq!(references[5].key, custom_folded_block);
}

#[test]
fn exposes_forward_declarations_and_current_structural_scope_for_completion() {
    let source = "@main\n~> [0 $0 #0\n@end main\n\
@main.F0\n~> ]\n@end main.F0\n\
@main.M0\n+ > _ ^\n@end main.M0\n\
@C0\n~> _\n@end C0\n";
    let main_offset = source.find("[0").expect("Main contains a function call");
    let main_context = completion_context(source, main_offset);

    assert_eq!(main_context.codegrid, CodeGridPath::Main);
    assert_eq!(main_context.board, BoardPath::Main(CodeGridPath::Main));
    assert_eq!(main_context.custom_slots, vec![slot(0)]);
    assert_eq!(main_context.function_slots, vec![slot(0)]);
    assert_eq!(main_context.folded_block_slots, vec![slot(0)]);
    assert!(main_context.inside_codegrid);
    assert!(main_context.can_close_block);

    let folded_offset = source.find("+ > _ ^").expect("Folded Block has one row");
    let folded_context = completion_context(source, folded_offset);
    assert!(folded_context.inside_folded_block);

    let custom_offset = source.find("~> _").expect("Custom Main has a grid");
    let custom_context = completion_context(source, custom_offset);
    assert_eq!(custom_context.codegrid, CodeGridPath::Custom(slot(0)));
    assert!(custom_context.inside_codegrid);
}

fn slot(value: u8) -> codegrid_model::Slot {
    codegrid_model::Slot::new(value).expect("test uses an in-range structural ID")
}
