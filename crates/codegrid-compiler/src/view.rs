use std::collections::BTreeMap;

use codegrid_ir::{Board, ScopedProgram, VerifiedProgram};
use codegrid_model::{AttachmentInstruction, Direction, PrimaryInstruction};

/// Canonical, read-only projection of verified code for host rendering.
///
/// This contains structure and canonical source spellings only. It does not
/// expose mutable VM state or grant authority to execute a host-supplied view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramView {
    pub ir_format_version: u32,
    pub outer: CodeGridView,
    pub customs: BTreeMap<u8, CodeGridView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeGridView {
    pub main: BoardView,
    pub functions: BTreeMap<u8, BoardView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardView {
    pub width: u64,
    pub height: u64,
    /// Row-major cells; the length is `width * height`.
    pub cells: Vec<CellView>,
    pub folded_blocks: BTreeMap<u8, Vec<Option<String>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellView {
    pub prefix: Option<String>,
    pub entry: Option<String>,
    pub primary: Option<String>,
    pub attachment: Option<String>,
}

impl CodeGridView {
    /// Projects a mutable runtime scope into the canonical read-only view.
    ///
    /// Unlike `ProgramView`, this projection does not contain Custom
    /// definitions; it is intended for a snapshot's current Outer program.
    pub fn from_scoped(program: &ScopedProgram) -> Self {
        scoped_program_view(program)
    }
}

impl ProgramView {
    pub fn from_verified(program: &VerifiedProgram) -> Self {
        let data = program.program();
        Self {
            ir_format_version: data.format_version,
            outer: scoped_program_view(&data.outer),
            customs: data
                .customs
                .iter()
                .map(|(slot, definition)| (slot.get(), scoped_program_view(&definition.program)))
                .collect(),
        }
    }
}

fn scoped_program_view(program: &ScopedProgram) -> CodeGridView {
    CodeGridView {
        main: board_view(&program.main),
        functions: program
            .functions
            .iter()
            .map(|(slot, board)| (slot.get(), board_view(board)))
            .collect(),
    }
}

fn board_view(board: &Board) -> BoardView {
    BoardView {
        width: board.width as u64,
        height: board.height as u64,
        cells: board.cells.iter().map(cell_view).collect(),
        folded_blocks: board
            .folded_blocks
            .iter()
            .map(|(slot, fold)| {
                (
                    slot.get(),
                    fold.cells
                        .iter()
                        .enumerate()
                        .map(|(index, primary)| {
                            primary.as_ref().map(|instruction| {
                                format!(
                                    "{}{}",
                                    fold.prefixes
                                        .get(&index)
                                        .map_or("", |prefix| prefix.token()),
                                    instruction.token()
                                )
                            })
                        })
                        .collect(),
                )
            })
            .collect(),
    }
}

fn cell_view(cell: &codegrid_ir::Cell) -> CellView {
    CellView {
        prefix: cell.prefix.map(|prefix| prefix.token().to_owned()),
        entry: cell.entry.map(entry_token),
        primary: cell.primary.map(PrimaryInstruction::token),
        attachment: cell.attachment.map(AttachmentInstruction::token),
    }
}

fn entry_token(direction: Direction) -> String {
    let arrow = match direction {
        Direction::Up => '^',
        Direction::Down => 'v',
        Direction::Left => '<',
        Direction::Right => '>',
    };
    format!("~{arrow}")
}

#[cfg(test)]
mod tests {
    use super::ProgramView;
    use crate::compile;

    #[test]
    fn projects_full_scopes_using_canonical_source_spellings() {
        let source = "@size 4x1\n@main\n~> [0* $0 #0\n@end main\n\
@main.M0 + _ _ _\n\
@main.F0\n~> ] _ _\n@end main.F0\n\
@C0\n~> #] _ _\n@end C0\n";
        let program = compile(source).expect("Full view fixture must compile");
        let view = ProgramView::from_verified(&program);

        assert_eq!(view.ir_format_version, 2);
        assert_eq!(view.outer.main.width, 4);
        assert_eq!(view.outer.main.height, 1);
        assert_eq!(view.outer.main.cells[0].entry.as_deref(), Some("~>"));
        assert_eq!(view.outer.main.cells[0].primary, None);
        assert_eq!(view.outer.main.cells[1].primary.as_deref(), Some("[0"));
        assert_eq!(view.outer.main.cells[1].attachment.as_deref(), Some("*"));
        assert_eq!(view.outer.main.cells[2].primary.as_deref(), Some("$0"));
        assert_eq!(view.outer.main.folded_blocks[&0][0].as_deref(), Some("+"));
        assert_eq!(
            view.outer.functions[&0].cells[1].primary.as_deref(),
            Some("]")
        );
        assert_eq!(
            view.customs[&0].main.cells[1].primary.as_deref(),
            Some("#]")
        );
    }
}
