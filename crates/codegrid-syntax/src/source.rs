use crate::{lex, Diagnostic, Span, Token, TokenKind};

/// One logical source line. Newlines inside block comments do not split a line
/// because comments contribute neither grid rows nor structural lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLine {
    pub tokens: Vec<Token>,
    pub span: Span,
    pub line_break: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SpannedText<'a> {
    pub text: &'a str,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectiveLine<'a> {
    /// The full directive atom, including its leading `@`.
    pub head: SpannedText<'a>,
    pub arguments: Vec<SpannedText<'a>>,
}

impl SourceLine {
    pub fn atoms<'a>(&'a self, source: &'a str) -> impl Iterator<Item = (&'a str, Span)> + 'a {
        self.tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Atom)
            .filter_map(|token| token.text(source).map(|text| (text, token.span)))
    }

    pub fn directive<'a>(&'a self, source: &'a str) -> Option<DirectiveLine<'a>> {
        let mut atoms = self.atoms(source);
        let (head_text, head_span) = atoms.next()?;
        if !head_text.starts_with('@') {
            return None;
        }
        Some(DirectiveLine {
            head: SpannedText {
                text: head_text,
                span: head_span,
            },
            arguments: atoms
                .map(|(text, span)| SpannedText { text, span })
                .collect(),
        })
    }
}

/// Lossless line grouping plus recoverable lexical diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceDocument {
    pub lines: Vec<SourceLine>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Groups lexical tokens into structural lines while retaining comments and
/// each original line ending's byte span.
pub fn source_lines(source: &str) -> SourceDocument {
    let lexed = lex(source);
    let mut document = SourceDocument {
        lines: Vec::new(),
        diagnostics: lexed.diagnostics,
    };
    let mut line_tokens = Vec::new();
    let mut line_start = 0;

    for token in lexed.tokens {
        if token.kind == TokenKind::Newline {
            document.lines.push(SourceLine {
                tokens: std::mem::take(&mut line_tokens),
                span: Span::new(line_start, token.span.start),
                line_break: Some(token.span),
            });
            line_start = token.span.end;
        } else {
            line_tokens.push(token);
        }
    }

    if line_start < source.len() || !line_tokens.is_empty() {
        document.lines.push(SourceLine {
            tokens: line_tokens,
            span: Span::new(line_start, source.len()),
            line_break: None,
        });
    }

    document
}
