//! Source compilation and static validation shared by CLI, LSP, and WASM.

mod builder;
mod lower;
mod symbols;
mod view;

use codegrid_ir::VerifiedProgram;
use codegrid_model::Slot;
use codegrid_syntax::{parse_syntax, BoardPath, CodeGridPath};

pub use codegrid_syntax::{Diagnostic, Severity, Span};
pub use symbols::{SymbolDefinition, SymbolIndex, SymbolKey, SymbolKind, SymbolReference};
pub use view::{BoardView, CellView, CodeGridView, ProgramView};

/// A validated program paired with compiler-resolved source symbols.
#[derive(Clone, Debug)]
pub struct Compilation {
    pub program: VerifiedProgram,
    pub symbols: SymbolIndex,
    /// Compiler-resolved IR paths mapped to UTF-8 source byte ranges.
    pub source_map: std::collections::BTreeMap<String, Span>,
}

/// Resolved lexical scope and declarations visible at a source byte offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionContext {
    pub codegrid: CodeGridPath,
    pub board: BoardPath,
    pub custom_slots: Vec<Slot>,
    pub function_slots: Vec<Slot>,
    pub folded_block_slots: Vec<Slot>,
    pub has_grid: bool,
    pub inside_folded_block: bool,
    pub inside_codegrid: bool,
    pub can_close_block: bool,
}

/// Returns the compiler-resolved declaration scope at a source byte offset.
/// This uses the same structural parser and definition builder as compilation.
pub fn completion_context(source: &str, byte_offset: usize) -> CompletionContext {
    let parsed = parse_syntax(source);
    builder::completion_context(source, parsed, byte_offset)
}

/// Parses, resolves, validates, and lowers source into executable IR.
pub fn compile(source: &str) -> Result<VerifiedProgram, Vec<Diagnostic>> {
    compile_with_symbols(source).map(|compilation| compilation.program)
}

/// Compiles source and returns the same resolved symbol identities used by
/// validation.
pub fn compile_with_symbols(source: &str) -> Result<Compilation, Vec<Diagnostic>> {
    let parsed = parse_syntax(source);
    if contains_errors(&parsed.diagnostics) {
        return Err(parsed.diagnostics);
    }

    let (hir, mut diagnostics) = builder::build_hir(source, parsed);
    if contains_errors(&diagnostics) {
        return Err(diagnostics);
    }
    let hir = match hir {
        Some(hir) => hir,
        None => {
            diagnostics.push(Diagnostic::error(
                "source.missing_program",
                "Source did not produce a program.",
                codegrid_syntax::Span::empty(0),
            ));
            return Err(diagnostics);
        }
    };

    let symbols = SymbolIndex::from_hir(&hir);
    let (program, locations) = lower::lower_program(hir);
    match VerifiedProgram::new(program) {
        Ok(program) => Ok(Compilation {
            program,
            symbols,
            source_map: locations,
        }),
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| {
                let span = locations
                    .get(&error.path)
                    .copied()
                    .unwrap_or_else(|| codegrid_syntax::Span::empty(0));
                Diagnostic::error(error.code, error.message, span)
            })
            .collect()),
    }
}

/// Performs the exact source and IR validation used by `compile` without
/// retaining the executable program.
pub fn check(source: &str) -> Vec<Diagnostic> {
    match compile(source) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn contains_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
