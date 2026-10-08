use crate::{
    parse_cell_token, parse_directive_head, parse_size, source_lines, CellToken, Diagnostic,
    DirectiveKind, DirectiveNameError, Span, SpannedText,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedDirective {
    pub kind: DirectiveKind,
    pub head_span: Span,
    pub arguments: Vec<Span>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxItem {
    Directive(ParsedDirective),
    GridRow { cells: Vec<SpannedCell>, span: Span },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SpannedCell {
    pub value: CellToken,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedSource {
    pub items: Vec<SyntaxItem>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses lexical lines, directive heads, and complete cell tokens.
/// Structural nesting and cross-reference validation belong to the compiler.
pub fn parse_syntax(source: &str) -> ParsedSource {
    let document = source_lines(source);
    let mut parsed = ParsedSource {
        items: Vec::new(),
        diagnostics: document.diagnostics,
    };

    for line in document.lines {
        let atoms: Vec<SpannedText<'_>> = line
            .atoms(source)
            .map(|(text, span)| SpannedText { text, span })
            .collect();
        let Some(first) = atoms.first().copied() else {
            continue;
        };

        if first.text.starts_with('@') {
            let kind = match parse_directive_head(first.text) {
                Ok(kind) => kind,
                Err(error) => {
                    parsed.diagnostics.push(Diagnostic::error(
                        error.code(),
                        directive_error_message(error),
                        first.span,
                    ));
                    continue;
                }
            };
            let arguments = atoms
                .iter()
                .skip(1)
                .map(|atom| atom.span)
                .collect::<Vec<_>>();
            validate_directive_arguments(kind, &atoms[1..], line.span, &mut parsed.diagnostics);
            if let DirectiveKind::Definition(target) = kind {
                if target_is_folded_block(target) {
                    for atom in atoms.iter().skip(1) {
                        if let Err(error) = parse_cell_token(atom.text) {
                            parsed.diagnostics.push(Diagnostic::error(
                                error.code,
                                format!("Invalid cell '{}': {}", atom.text, error.message),
                                atom.span,
                            ));
                        }
                    }
                }
            }
            parsed.items.push(SyntaxItem::Directive(ParsedDirective {
                kind,
                head_span: first.span,
                arguments,
                span: line.span,
            }));
            continue;
        }

        let mut cells = Vec::with_capacity(atoms.len());
        for atom in atoms {
            match parse_cell_token(atom.text) {
                Ok(value) => cells.push(SpannedCell {
                    value,
                    span: atom.span,
                }),
                Err(error) => parsed.diagnostics.push(Diagnostic::error(
                    error.code,
                    format!("Invalid cell '{}': {}", atom.text, error.message),
                    atom.span,
                )),
            }
        }
        parsed.items.push(SyntaxItem::GridRow {
            cells,
            span: line.span,
        });
    }

    parsed
}

fn validate_directive_arguments(
    kind: DirectiveKind,
    arguments: &[SpannedText<'_>],
    line_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match kind {
        DirectiveKind::Size => {
            if arguments.len() != 1 {
                diagnostics.push(Diagnostic::error(
                    "source.size_argument_count",
                    "@size requires exactly one WIDTHxHEIGHT value.",
                    line_span,
                ));
                return;
            }
            if let Err(error) = parse_size(arguments[0].text) {
                let message = match error {
                    crate::SizeError::Malformed => {
                        "@size must use positive decimal dimensions in WIDTHxHEIGHT form."
                    }
                    crate::SizeError::Zero => "@size dimensions must be greater than zero.",
                    crate::SizeError::Overflow => {
                        "@size dimensions or board cell count exceed the portable source range."
                    }
                };
                diagnostics.push(Diagnostic::error(error.code(), message, arguments[0].span));
            }
        }
        DirectiveKind::End if arguments.len() > 1 => {
            diagnostics.push(Diagnostic::error(
                "source.end_argument_count",
                "@end accepts at most one closing name.",
                arguments[1].span,
            ));
        }
        DirectiveKind::End if !arguments.is_empty() => {
            let closing_name = format!("@{}", arguments[0].text);
            if !matches!(
                parse_directive_head(&closing_name),
                Ok(DirectiveKind::Definition(_))
            ) {
                diagnostics.push(Diagnostic::error(
                    "source.invalid_end_name",
                    "@end closing name must be a valid structural path.",
                    arguments[0].span,
                ));
            }
        }
        DirectiveKind::Definition(target)
            if !target_is_folded_block(target) && !arguments.is_empty() =>
        {
            diagnostics.push(Diagnostic::error(
                "source.definition_arguments",
                "Only a Folded Block definition may include cells on its directive line.",
                arguments[0].span,
            ));
        }
        _ => {}
    }
}

fn target_is_folded_block(target: crate::DefinitionTarget) -> bool {
    matches!(
        target,
        crate::DefinitionTarget::RelativeFoldedBlock(_)
            | crate::DefinitionTarget::FoldedBlock { .. }
            | crate::DefinitionTarget::RelativeFunctionFoldedBlock { .. }
    )
}

fn directive_error_message(error: DirectiveNameError) -> &'static str {
    match error {
        DirectiveNameError::MissingAtSign => "A directive must begin with @.",
        DirectiveNameError::InvalidPath => "Invalid CodeGrid structural path.",
        DirectiveNameError::UnknownDirective => "Unknown CodeGrid directive.",
    }
}

#[cfg(test)]
mod tests {
    use crate::{parse_syntax, Span, SyntaxItem};

    #[test]
    fn reports_invalid_cells_at_utf8_byte_spans_and_keeps_later_cells() {
        let source = "// café\r\n~v #0x3 _\r\n";
        let parsed = parse_syntax(source);

        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(parsed.diagnostics[0].span, Span::new(13, 17));
        let [SyntaxItem::GridRow { cells, .. }] = parsed.items.as_slice() else {
            panic!("the malformed cell should not prevent row recovery");
        };
        assert_eq!(cells.len(), 2);
    }

    #[test]
    fn block_comment_newlines_do_not_create_rows_or_cells() {
        let parsed = parse_syntax("~v /* ignored\ninside */ _\r\n");

        assert!(parsed.diagnostics.is_empty());
        let [SyntaxItem::GridRow { cells, .. }] = parsed.items.as_slice() else {
            panic!("comment contents must not create source rows");
        };
        assert_eq!(cells.len(), 2);
    }

    #[test]
    fn validates_directive_arguments_and_full_cell_tokens() {
        let parsed = parse_syntax("@size 0x5\n@main extra\n~> #\n");

        assert_eq!(parsed.diagnostics.len(), 3);
        assert!(parsed.diagnostics[0].message.contains("greater than zero"));
        assert!(parsed.diagnostics[1]
            .message
            .contains("may include cells on its directive line"));
        assert!(parsed.diagnostics[2]
            .message
            .contains("malformed Primary reference"));
    }

    #[test]
    fn parses_optional_end_names_and_rejects_invalid_names_at_their_spans() {
        let source = "@end\n@end main\n@end F0\n@end main.F0\n@end C0.F0.M1\n@end main extra\n@end invalid\n";
        let parsed = parse_syntax(source);

        assert_eq!(parsed.items.len(), 7);
        assert_eq!(parsed.diagnostics.len(), 2);
        assert!(parsed.diagnostics[0]
            .message
            .contains("at most one closing name"));
        assert!(parsed.diagnostics[1]
            .message
            .contains("valid structural path"));
        assert_eq!(
            parsed.diagnostics[0].span,
            Span::new(
                source.rfind("extra").unwrap(),
                source.rfind("extra").unwrap() + "extra".len()
            )
        );
        assert_eq!(
            parsed.diagnostics[1].span,
            Span::new(
                source.rfind("invalid").unwrap(),
                source.rfind("invalid").unwrap() + "invalid".len()
            )
        );

        let utf8 = parse_syntax("// café\n@end C10\n");
        assert_eq!(utf8.diagnostics[0].span, Span::new(14, 17));
    }

    #[test]
    fn parses_folded_block_directive_cells_and_recovers_after_an_invalid_cell() {
        let source = "@main.M0 +x3 ,*\n@C0.M9 #] ;\n@main\n~> ;\n";
        let parsed = parse_syntax(source);

        assert_eq!(parsed.diagnostics.len(), 0);
        assert_eq!(parsed.items.len(), 4);
        let SyntaxItem::Directive(folded) = &parsed.items[0] else {
            panic!("the first item should be a Folded Block directive");
        };
        assert_eq!(folded.arguments.len(), 2);

        let invalid_source = "@M0 +x3 malformed _\n@main\n~> ;\n";
        let invalid = parse_syntax(invalid_source);
        assert_eq!(invalid.diagnostics.len(), 1);
        assert!(invalid.diagnostics[0]
            .message
            .contains("malformed Full cell token"));
        assert!(matches!(
            invalid.items.last(),
            Some(SyntaxItem::GridRow { .. })
        ));
    }

    #[test]
    fn size_directives_remain_distinct_between_grid_rows() {
        let source = "~> _\n@size 2x2\n_ ;\n";
        let parsed = parse_syntax(source);

        assert!(parsed.diagnostics.is_empty());
        assert!(matches!(
            parsed.items.as_slice(),
            [
                SyntaxItem::GridRow { .. },
                SyntaxItem::Directive(_),
                SyntaxItem::GridRow { .. }
            ]
        ));
    }

    #[test]
    fn non_ascii_whitespace_does_not_split_cell_atoms() {
        let source = "~>\u{00a0}+\n";
        let parsed = parse_syntax(source);

        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(
            parsed.diagnostics[0].span,
            Span::new(0, source.trim_end().len())
        );
    }
}
