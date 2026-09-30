//! Verified, immutable CodeGrid programs accepted by the VM.
//!
//! Source spans remain in the compiler's source map. This crate contains only
//! executable program data and deterministic validation of that data.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use codegrid_model::{
    AttachmentInstruction, BoundaryMode, Direction, PrimaryInstruction, Slot, MAX_BOARD_CELLS,
    MAX_BOARD_DIMENSION,
};

pub const IR_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeGridId {
    Outer,
    Custom(Slot),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoardId {
    Main,
    Function(Slot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cell {
    pub entry: Option<Direction>,
    pub primary: Option<PrimaryInstruction>,
    pub attachment: Option<AttachmentInstruction>,
}

impl Cell {
    pub const fn empty() -> Self {
        Self {
            entry: None,
            primary: None,
            attachment: None,
        }
    }

    pub const fn entry(direction: Direction) -> Self {
        Self {
            entry: Some(direction),
            primary: None,
            attachment: None,
        }
    }

    pub const fn instruction(
        primary: PrimaryInstruction,
        attachment: Option<AttachmentInstruction>,
    ) -> Self {
        Self {
            entry: None,
            primary: Some(primary),
            attachment,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldedBlock {
    pub cells: Vec<Option<PrimaryInstruction>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Board {
    pub width: usize,
    pub height: usize,
    /// Row-major cells; the length must equal `width * height`.
    pub cells: Vec<Cell>,
    pub folded_blocks: BTreeMap<Slot, FoldedBlock>,
}

impl Board {
    pub fn cell(&self, x: usize, y: usize) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(y.checked_mul(self.width)?.checked_add(x)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedProgram {
    pub main: Board,
    pub functions: BTreeMap<Slot, Board>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomDefinition {
    pub program: ScopedProgram,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub format_version: u32,
    pub outer: ScopedProgram,
    pub customs: BTreeMap<Slot, CustomDefinition>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TailCallSite {
    pub code_grid: CodeGridId,
    pub function: Slot,
    pub cell_index: usize,
    pub boundary_mode: BoundaryMode,
}

/// A Program that has passed all structural IR invariants and static reference
/// checks. Its fields are private so callers cannot bypass verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedProgram {
    program: Program,
    tail_call_sites: BTreeSet<TailCallSite>,
}

impl VerifiedProgram {
    pub fn new(program: Program) -> Result<Self, Vec<IrError>> {
        validate_program(&program).map(|()| Self {
            tail_call_sites: analyze_tail_calls(&program),
            program,
        })
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Returns statically proven self-tail-call locations. This metadata is
    /// derived from verified program structure and is not part of serialized IR.
    pub fn tail_call_sites(&self) -> &BTreeSet<TailCallSite> {
        &self.tail_call_sites
    }

    pub fn is_tail_call(
        &self,
        code_grid: CodeGridId,
        function: Slot,
        cell_index: usize,
        boundary_mode: BoundaryMode,
    ) -> bool {
        self.tail_call_sites.contains(&TailCallSite {
            code_grid,
            function,
            cell_index,
            boundary_mode,
        })
    }
}

fn analyze_tail_calls(program: &Program) -> BTreeSet<TailCallSite> {
    let mut sites = BTreeSet::new();
    collect_tail_calls(&program.outer, CodeGridId::Outer, &mut sites);
    for (slot, custom) in &program.customs {
        collect_tail_calls(&custom.program, CodeGridId::Custom(*slot), &mut sites);
    }
    sites
}

fn collect_tail_calls(
    scoped: &ScopedProgram,
    code_grid: CodeGridId,
    sites: &mut BTreeSet<TailCallSite>,
) {
    for (function, board) in &scoped.functions {
        let reachable = reachable_directions(board);
        for (index, cell) in board.cells.iter().enumerate() {
            if !matches!(cell.primary, Some(PrimaryInstruction::Call(target)) if target == *function)
                || cell.attachment.is_some()
            {
                continue;
            }
            let Some(incoming) = reachable.get(&index) else {
                continue;
            };
            for boundary_mode in [BoundaryMode::Exit, BoundaryMode::Wrap] {
                let mut has_incoming_path = false;
                let mut all_paths_return = true;
                for &(direction, mode) in incoming {
                    if mode != boundary_mode {
                        continue;
                    }
                    has_incoming_path = true;
                    all_paths_return &= has_navigation_return_path(board, index, direction, mode);
                }
                if has_incoming_path && all_paths_return {
                    sites.insert(TailCallSite {
                        code_grid,
                        function: *function,
                        cell_index: index,
                        boundary_mode,
                    });
                }
            }
        }
    }
}

/// Computes a conservative set of directions that can reach each cell,
/// considering both legal boundary modes and runtime-dependent direction
/// instructions.
fn reachable_directions(board: &Board) -> BTreeMap<usize, BTreeSet<(Direction, BoundaryMode)>> {
    let Some((entry_index, entry_direction)) = board
        .cells
        .iter()
        .enumerate()
        .find_map(|(index, cell)| cell.entry.map(|direction| (index, direction)))
    else {
        return BTreeMap::new();
    };

    let mut queue = VecDeque::new();
    queue.push_back((entry_index, entry_direction, BoundaryMode::Exit));
    queue.push_back((entry_index, entry_direction, BoundaryMode::Wrap));
    let mut visited = BTreeSet::new();
    let mut by_cell: BTreeMap<usize, BTreeSet<(Direction, BoundaryMode)>> = BTreeMap::new();

    while let Some((index, direction, wraps)) = queue.pop_front() {
        if !visited.insert((index, direction, wraps)) {
            continue;
        }
        by_cell.entry(index).or_default().insert((direction, wraps));

        let Some(cell) = board.cells.get(index) else {
            continue;
        };
        let next_directions = match cell.primary {
            Some(
                PrimaryInstruction::Halt
                | PrimaryInstruction::Return
                | PrimaryInstruction::CustomReturn,
            ) => {
                continue;
            }
            Some(PrimaryInstruction::Direction(next)) => vec![next],
            Some(PrimaryInstruction::RandomDirection) => vec![
                Direction::Up,
                Direction::Down,
                Direction::Left,
                Direction::Right,
            ],
            Some(PrimaryInstruction::IfZero(target) | PrimaryInstruction::Read(target)) => {
                vec![direction, target]
            }
            _ => vec![direction],
        };

        for next_direction in next_directions {
            if let Some(next_index) = next_cell_index(board, index, next_direction, wraps) {
                queue.push_back((next_index, next_direction, wraps));
            }
        }
    }
    by_cell
}

fn has_navigation_return_path(
    board: &Board,
    call_index: usize,
    direction: Direction,
    boundary_mode: BoundaryMode,
) -> bool {
    let Some(mut index) = next_cell_index(board, call_index, direction, boundary_mode) else {
        return false;
    };
    let mut current_direction = direction;
    let mut visited = BTreeSet::new();

    loop {
        if !visited.insert((index, current_direction)) {
            return false;
        }
        let Some(cell) = board.cells.get(index) else {
            return false;
        };
        if cell.primary == Some(PrimaryInstruction::Return) {
            return true;
        }
        if cell.entry.is_some() || cell.attachment.is_some() {
            return false;
        }
        match cell.primary {
            None => {}
            Some(PrimaryInstruction::Direction(next)) => current_direction = next,
            _ => return false,
        }
        let Some(next) = next_cell_index(board, index, current_direction, boundary_mode) else {
            return false;
        };
        index = next;
    }
}

fn next_cell_index(
    board: &Board,
    index: usize,
    direction: Direction,
    boundary_mode: BoundaryMode,
) -> Option<usize> {
    if board.width == 0 || board.height == 0 || index >= board.cells.len() {
        return None;
    }
    let x = index % board.width;
    let y = index / board.width;
    let (next_x, next_y) = match direction {
        Direction::Up if y > 0 => (x, y - 1),
        Direction::Down if y + 1 < board.height => (x, y + 1),
        Direction::Left if x > 0 => (x - 1, y),
        Direction::Right if x + 1 < board.width => (x + 1, y),
        Direction::Up if boundary_mode == BoundaryMode::Wrap => (x, board.height - 1),
        Direction::Down if boundary_mode == BoundaryMode::Wrap => (x, 0),
        Direction::Left if boundary_mode == BoundaryMode::Wrap => (board.width - 1, y),
        Direction::Right if boundary_mode == BoundaryMode::Wrap => (0, y),
        _ => return None,
    };
    next_y.checked_mul(board.width)?.checked_add(next_x)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrError {
    pub code: &'static str,
    pub path: String,
    pub message: &'static str,
}

#[derive(Clone, Copy)]
enum BoardContext {
    OuterMain,
    OuterFunction,
    CustomMain,
    CustomFunction,
}

fn validate_program(program: &Program) -> Result<(), Vec<IrError>> {
    let mut errors = Vec::new();
    if program.format_version != IR_FORMAT_VERSION {
        errors.push(error(
            "program",
            "ir.unsupported_version",
            "The executable program uses an unsupported IR format version.",
        ));
    }
    validate_scoped(
        &program.outer,
        BoardContext::OuterMain,
        "@main",
        &program.customs,
        &mut errors,
    );

    for (slot, custom) in &program.customs {
        let context = BoardContext::CustomMain;
        validate_scoped(
            &custom.program,
            context,
            &format!("@C{}", slot.get()),
            &program.customs,
            &mut errors,
        );
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_scoped(
    scoped: &ScopedProgram,
    context: BoardContext,
    path: &str,
    customs: &BTreeMap<Slot, CustomDefinition>,
    errors: &mut Vec<IrError>,
) {
    validate_board(
        &scoped.main,
        context,
        path,
        customs,
        &scoped.functions,
        errors,
    );

    for (slot, board) in &scoped.functions {
        let function_context = match context {
            BoardContext::OuterMain | BoardContext::OuterFunction => BoardContext::OuterFunction,
            BoardContext::CustomMain | BoardContext::CustomFunction => BoardContext::CustomFunction,
        };
        validate_board(
            board,
            function_context,
            &format!("{path}.F{}", slot.get()),
            customs,
            &scoped.functions,
            errors,
        );
    }
}

fn validate_board(
    board: &Board,
    context: BoardContext,
    path: &str,
    customs: &BTreeMap<Slot, CustomDefinition>,
    functions: &BTreeMap<Slot, Board>,
    errors: &mut Vec<IrError>,
) {
    let Some(expected_cells) = board.width.checked_mul(board.height) else {
        errors.push(error(
            path,
            "ir.geometry_overflow",
            "Board dimensions overflow the host address space.",
        ));
        return;
    };
    let exceeds_portable_geometry = board.width > MAX_BOARD_DIMENSION as usize
        || board.height > MAX_BOARD_DIMENSION as usize
        || u64::try_from(expected_cells)
            .map(|cell_count| cell_count > MAX_BOARD_CELLS)
            .unwrap_or(true);
    if exceeds_portable_geometry {
        errors.push(error(
            path,
            "ir.geometry_limit",
            "Board dimensions exceed the portable Full geometry bounds.",
        ));
    }
    if board.width == 0 || board.height == 0 || board.cells.len() != expected_cells {
        errors.push(error(
            path,
            "ir.invalid_layout",
            "Board dimensions must be positive and match its cell count.",
        ));
    }

    let entry_count = board
        .cells
        .iter()
        .filter(|cell| cell.entry.is_some())
        .count();
    match context {
        BoardContext::OuterMain | BoardContext::CustomMain if entry_count == 0 => {
            errors.push(error(
                path,
                "ir.main_entry_count",
                "A Main board must contain at least one Entry.",
            ));
        }
        BoardContext::OuterFunction | BoardContext::CustomFunction if entry_count != 1 => {
            errors.push(error(
                path,
                "ir.function_entry_count",
                "A function board must contain exactly one Entry.",
            ));
        }
        _ => {}
    }

    for (index, cell) in board.cells.iter().enumerate() {
        let cell_path = format!("{path}[{}]", index);
        validate_cell(cell, context, &cell_path, customs, functions, board, errors);
    }

    for (slot, folded) in &board.folded_blocks {
        let fold_path = format!("{path}.M{}", slot.get());
        if folded.cells.len() != board.width {
            errors.push(error(
                &fold_path,
                "ir.fold_width",
                "Folded Block width must match its owner board.",
            ));
        }
        for (index, primary) in folded.cells.iter().enumerate() {
            if let Some(primary) = primary {
                let outer_context = matches!(
                    context,
                    BoardContext::OuterMain | BoardContext::OuterFunction
                );
                if !folded_primary_allowed(*primary, outer_context) {
                    errors.push(error(
                        &format!("{fold_path}[{index}]"),
                        "ir.fold_primary",
                        "This instruction is not allowed in a Folded Block.",
                    ));
                    continue;
                }
                validate_reference(
                    *primary,
                    context,
                    &format!("{fold_path}[{index}]"),
                    customs,
                    functions,
                    board,
                    errors,
                );
            }
        }
    }
}

fn validate_cell(
    cell: &Cell,
    context: BoardContext,
    path: &str,
    customs: &BTreeMap<Slot, CustomDefinition>,
    functions: &BTreeMap<Slot, Board>,
    board: &Board,
    errors: &mut Vec<IrError>,
) {
    if cell.entry.is_some() && (cell.primary.is_some() || cell.attachment.is_some()) {
        errors.push(error(
            path,
            "ir.entry_instruction",
            "An Entry cannot share a cell with an instruction or Attachment.",
        ));
    }
    if cell.attachment.is_some() && cell.primary.is_none() {
        errors.push(error(
            path,
            "ir.detached_attachment",
            "An Attachment requires a Primary instruction.",
        ));
    }
    if let Some(primary) = cell.primary {
        if let Some(attachment) = cell.attachment {
            if matches!(attachment, AttachmentInstruction::Repeat(count) if !(2..=5).contains(&count))
            {
                errors.push(error(
                    path,
                    "ir.repeat_count",
                    "Repeat count must be from 2 through 5.",
                ));
            }
            if !primary.is_encodable() {
                errors.push(error(
                    path,
                    "ir.attachment_primary",
                    "This Primary instruction cannot have an Attachment.",
                ));
            }
            if matches!(attachment, AttachmentInstruction::Repeat(_))
                && matches!(
                    primary,
                    PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                )
            {
                errors.push(error(
                    path,
                    "ir.repeat_call_return",
                    "Repeat is not allowed on CALL or RETURN.",
                ));
            }
        }
        if matches!(primary, PrimaryInstruction::Return)
            && !matches!(
                context,
                BoardContext::OuterFunction | BoardContext::CustomFunction
            )
        {
            errors.push(error(
                path,
                "ir.return_scope",
                "RETURN is allowed only on a function board.",
            ));
        }
        if matches!(primary, PrimaryInstruction::CustomReturn)
            && !matches!(context, BoardContext::CustomMain)
        {
            errors.push(error(
                path,
                "ir.custom_return_scope",
                "CUSTOM_RETURN is allowed only on a Custom Main board.",
            ));
        }
        validate_reference(primary, context, path, customs, functions, board, errors);
    }
}

fn validate_reference(
    primary: PrimaryInstruction,
    context: BoardContext,
    path: &str,
    customs: &BTreeMap<Slot, CustomDefinition>,
    functions: &BTreeMap<Slot, Board>,
    board: &Board,
    errors: &mut Vec<IrError>,
) {
    match primary {
        PrimaryInstruction::Call(slot) if !functions.contains_key(&slot) => {
            errors.push(error(
                path,
                "ir.undefined_function",
                "CALL references an undefined function.",
            ));
        }
        PrimaryInstruction::FoldedBlock(slot) if !board.folded_blocks.contains_key(&slot) => {
            errors.push(error(
                path,
                "ir.undefined_fold",
                "Folded Block reference is undefined on this board.",
            ));
        }
        PrimaryInstruction::Custom(slot)
            if !matches!(
                context,
                BoardContext::OuterMain | BoardContext::OuterFunction
            ) || !customs.contains_key(&slot) =>
        {
            errors.push(error(
                path,
                "ir.custom_reference",
                "Custom calls are unavailable here or reference an undefined Custom.",
            ));
        }
        _ => {}
    }
}

fn folded_primary_allowed(primary: PrimaryInstruction, outer: bool) -> bool {
    match primary {
        PrimaryInstruction::FoldedBlock(_)
        | PrimaryInstruction::Call(_)
        | PrimaryInstruction::Return
        | PrimaryInstruction::CustomReturn => false,
        PrimaryInstruction::Custom(_) => outer,
        _ => true,
    }
}

fn error(path: &str, code: &'static str, message: &'static str) -> IrError {
    IrError {
        code,
        path: path.to_owned(),
        message,
    }
}
