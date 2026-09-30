use codegrid_hir as hir;
use codegrid_model::{PrimaryInstruction, Slot};
use codegrid_syntax::{BoardPath, CodeGridPath, Span};

/// Stable identity for a source-level definition inside one program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SymbolKey {
    Custom(Slot),
    Function { codegrid: CodeGridPath, slot: Slot },
    FoldedBlock { board: BoardPath, slot: Slot },
}

/// The category of a source definition exposed to editor adapters.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SymbolKind {
    Custom,
    Function,
    FoldedBlock,
}

impl SymbolKey {
    pub const fn kind(self) -> SymbolKind {
        match self {
            Self::Custom(_) => SymbolKind::Custom,
            Self::Function { .. } => SymbolKind::Function,
            Self::FoldedBlock { .. } => SymbolKind::FoldedBlock,
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Custom(slot) => format!("C{}", slot.get()),
            Self::Function { slot, .. } => format!("F{}", slot.get()),
            Self::FoldedBlock { slot, .. } => format!("M{}", slot.get()),
        }
    }

    pub fn qualified_name(self) -> String {
        match self {
            Self::Custom(slot) => format!("@C{}", slot.get()),
            Self::Function { codegrid, slot } => {
                format!("{}.F{}", codegrid_name(codegrid), slot.get())
            }
            Self::FoldedBlock { board, slot } => {
                format!("{}.M{}", board_name(board), slot.get())
            }
        }
    }
}

fn codegrid_name(codegrid: CodeGridPath) -> String {
    match codegrid {
        CodeGridPath::Main => "@main".to_owned(),
        CodeGridPath::Custom(slot) => format!("@C{}", slot.get()),
    }
}

fn board_name(board: BoardPath) -> String {
    match board {
        BoardPath::Main(codegrid) => codegrid_name(codegrid),
        BoardPath::Function { codegrid, slot } => {
            format!("{}.F{}", codegrid_name(codegrid), slot.get())
        }
    }
}

/// A source definition and the byte span of its declaring directive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolDefinition {
    pub key: SymbolKey,
    pub span: Span,
}

/// A source reference and the byte span of its complete cell token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolReference {
    pub key: SymbolKey,
    pub span: Span,
}

/// Definitions and resolved references derived from the compiler's validated HIR.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolIndex {
    definitions: Vec<SymbolDefinition>,
    references: Vec<SymbolReference>,
}

impl SymbolIndex {
    pub fn definitions(&self) -> &[SymbolDefinition] {
        &self.definitions
    }

    pub fn references(&self) -> &[SymbolReference] {
        &self.references
    }

    pub fn definition(&self, key: SymbolKey) -> Option<&SymbolDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.key == key)
    }

    pub fn references_to(&self, key: SymbolKey) -> impl Iterator<Item = &SymbolReference> {
        self.references
            .iter()
            .filter(move |reference| reference.key == key)
    }

    pub fn symbol_at(&self, byte_offset: usize) -> Option<SymbolKey> {
        self.references
            .iter()
            .find(|reference| contains(reference.span, byte_offset))
            .map(|reference| reference.key)
            .or_else(|| {
                self.definitions
                    .iter()
                    .find(|definition| contains(definition.span, byte_offset))
                    .map(|definition| definition.key)
            })
    }

    pub(super) fn from_hir(program: &hir::Program) -> Self {
        let mut index = Self::default();
        collect_code_grid(&program.main, CodeGridPath::Main, &mut index);

        for (slot, custom) in &program.customs {
            index.definitions.push(SymbolDefinition {
                key: SymbolKey::Custom(*slot),
                span: custom.span,
            });
            collect_code_grid(&custom.value, CodeGridPath::Custom(*slot), &mut index);
        }

        index
            .definitions
            .sort_by_key(|definition| definition.span.start);
        index
            .references
            .sort_by_key(|reference| reference.span.start);
        index
    }
}

fn contains(span: Span, byte_offset: usize) -> bool {
    span.start <= byte_offset && byte_offset < span.end
}

fn collect_code_grid(code_grid: &hir::CodeGrid, path: CodeGridPath, index: &mut SymbolIndex) {
    collect_board(&code_grid.main.value, BoardPath::Main(path), path, index);
    collect_function_definitions_and_references(code_grid, path, index);
}

fn collect_function_definitions_and_references(
    code_grid: &hir::CodeGrid,
    path: CodeGridPath,
    index: &mut SymbolIndex,
) {
    for (slot, function) in &code_grid.functions {
        index.definitions.push(SymbolDefinition {
            key: SymbolKey::Function {
                codegrid: path,
                slot: *slot,
            },
            span: function.span,
        });
        collect_board(
            &function.value,
            BoardPath::Function {
                codegrid: path,
                slot: *slot,
            },
            path,
            index,
        );
    }
}

fn collect_board(
    board: &hir::Board,
    board_path: BoardPath,
    codegrid: CodeGridPath,
    index: &mut SymbolIndex,
) {
    for (slot, folded_block) in &board.folded_blocks {
        index.definitions.push(SymbolDefinition {
            key: SymbolKey::FoldedBlock {
                board: board_path,
                slot: *slot,
            },
            span: folded_block.span,
        });
    }

    for row in &board.rows {
        for cell in row {
            let hir::Cell::Instruction { primary, .. } = cell.value else {
                continue;
            };
            let key = match primary {
                PrimaryInstruction::Call(slot) => Some(SymbolKey::Function { codegrid, slot }),
                PrimaryInstruction::FoldedBlock(slot) => Some(SymbolKey::FoldedBlock {
                    board: board_path,
                    slot,
                }),
                PrimaryInstruction::Custom(slot) => Some(SymbolKey::Custom(slot)),
                _ => None,
            };
            if let Some(key) = key {
                index.references.push(SymbolReference {
                    key,
                    span: cell.span,
                });
            }
        }
    }

    for folded_block in board.folded_blocks.values() {
        for cell in &folded_block.value.cells {
            let Some(PrimaryInstruction::Custom(slot)) = cell.value else {
                continue;
            };
            index.references.push(SymbolReference {
                key: SymbolKey::Custom(slot),
                span: cell.span,
            });
        }
    }
}
