use std::collections::BTreeMap;

use codegrid_hir as hir;
use codegrid_ir as ir;
use codegrid_syntax::Span;

pub(super) fn lower_program(program: hir::Program) -> (ir::Program, BTreeMap<String, Span>) {
    let mut locations = BTreeMap::new();
    let outer = lower_codegrid(program.main, "@main", &mut locations);
    let mut customs = BTreeMap::new();
    for (slot, custom) in program.customs {
        let path = format!("@C{}", slot.get());
        let lowered = lower_codegrid(custom.value, &path, &mut locations);
        customs.insert(slot, ir::CustomDefinition { program: lowered });
    }
    (
        ir::Program {
            format_version: ir::IR_FORMAT_VERSION,
            outer,
            customs,
        },
        locations,
    )
}

fn lower_codegrid(
    codegrid: hir::CodeGrid,
    path: &str,
    locations: &mut BTreeMap<String, Span>,
) -> ir::ScopedProgram {
    let main = lower_board(codegrid.main, path, locations);
    let mut functions = BTreeMap::new();
    for (slot, function) in codegrid.functions {
        let function_path = format!("{path}.F{}", slot.get());
        functions.insert(slot, lower_board(function, &function_path, locations));
    }
    ir::ScopedProgram { main, functions }
}

fn lower_board(
    board: hir::Spanned<hir::Board>,
    path: &str,
    locations: &mut BTreeMap<String, Span>,
) -> ir::Board {
    let board_span = board.span;
    locations.insert(path.to_owned(), board_span);

    let inferred_width = board.value.rows.first().map_or(0, Vec::len);
    let width = board
        .value
        .size
        .map_or(inferred_width, |size| size.value.width);
    let height = board
        .value
        .size
        .map_or(board.value.rows.len(), |size| size.value.height);
    let mut cells = Vec::new();
    for row in board.value.rows {
        for cell in row {
            let index = cells.len();
            let cell_path = format!("{path}[{index}]");
            locations.insert(cell_path, cell.span);
            cells.push(match cell.value {
                hir::Cell::Empty => ir::Cell::empty(),
                hir::Cell::Entry(direction) => ir::Cell::entry(direction),
                hir::Cell::Instruction {
                    prefix,
                    primary,
                    attachment,
                } => {
                    let mut cell = ir::Cell::instruction(primary, attachment);
                    cell.prefix = prefix;
                    cell
                }
            });
        }
    }

    let mut folded_blocks = BTreeMap::new();
    for (slot, folded) in board.value.folded_blocks {
        let folded_path = format!("{path}.M{}", slot.get());
        locations.insert(folded_path.clone(), folded.span);
        let mut folded_cells = Vec::with_capacity(folded.value.cells.len());
        for (index, cell) in folded.value.cells.into_iter().enumerate() {
            locations.insert(format!("{folded_path}[{index}]"), cell.span);
            folded_cells.push(cell.value);
        }
        folded_blocks.insert(
            slot,
            ir::FoldedBlock {
                cells: folded_cells,
                prefixes: folded.value.prefixes,
            },
        );
    }

    ir::Board {
        width,
        height,
        cells,
        folded_blocks,
    }
}
