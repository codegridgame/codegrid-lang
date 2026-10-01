//! Host-independent source text primitives for CodeGrid.
//!
//! This crate accepts in-memory UTF-8 source and never reads files or invokes
//! host services. Tokens retain byte spans so compiler and editor adapters can
//! map diagnostics back to the original text.

mod cell;
mod directive;
mod formatter;
mod lexer;
mod parser;
mod source;
mod span;

pub use cell::{parse_cell_token, CellToken, CellTokenError};
pub use directive::{
    parse_directive_head, parse_size, BoardPath, BoardSize, CodeGridPath, DefinitionTarget,
    DirectiveKind, DirectiveNameError, SizeError,
};
pub use formatter::format_source;
pub use lexer::{lex, Lexed, Token, TokenKind};
pub use parser::{parse_syntax, ParsedDirective, ParsedSource, SyntaxItem};
pub use source::{source_lines, DirectiveLine, SourceDocument, SourceLine, SpannedText};
pub use span::{LineIndex, Span};

/// Severity for a source diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

/// A source diagnostic whose span uses UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Stable machine-readable identifier; independent of message wording.
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    /// Stable four-digit presentation identity for source or forwarded IR errors.
    pub fn error_number(&self) -> Option<&'static str> {
        codegrid_model::error_number("source", self.code)
            .or_else(|| codegrid_model::error_number("ir", self.code))
    }

    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }
}
