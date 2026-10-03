//! Source-aware high-level program representation.
//!
//! HIR normalizes nested and qualified declarations while retaining byte
//! spans for diagnostics. Lowering to executable IR removes source locations.

use std::collections::BTreeMap;

use codegrid_model::{AttachmentInstruction, ConditionPrefix, Direction, PrimaryInstruction, Slot};
use codegrid_syntax::Span;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoardSize {
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub main: CodeGrid,
    pub customs: BTreeMap<Slot, Spanned<CodeGrid>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeGrid {
    pub main: Spanned<Board>,
    pub functions: BTreeMap<Slot, Spanned<Board>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Board {
    /// An explicit or inherited size. `None` means dimensions are inferred.
    pub size: Option<Spanned<BoardSize>>,
    pub rows: Vec<Vec<Spanned<Cell>>>,
    pub folded_blocks: BTreeMap<Slot, Spanned<FoldedBlock>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldedBlock {
    pub prefixes: BTreeMap<usize, ConditionPrefix>,
    pub cells: Vec<Spanned<Option<PrimaryInstruction>>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cell {
    Empty,
    Entry(Direction),
    Instruction {
        prefix: Option<ConditionPrefix>,
        primary: PrimaryInstruction,
        attachment: Option<AttachmentInstruction>,
    },
}
