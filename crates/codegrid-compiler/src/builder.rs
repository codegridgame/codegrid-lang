use std::collections::{BTreeMap, BTreeSet};

use codegrid_hir as hir;
use codegrid_model::{PrimaryInstruction, Slot, MAX_BOARD_CELLS, MAX_BOARD_DIMENSION};
use codegrid_syntax::{
    parse_cell_token, parse_size, BoardPath, CellToken, CodeGridPath, DefinitionTarget, Diagnostic,
    DirectiveKind, ParsedDirective, ParsedSource, Severity, Span, SyntaxItem,
};

use crate::CompletionContext;

#[derive(Clone, Debug, Default)]
struct RawRow {
    cells: Vec<RawCell>,
    span: Span,
}

#[derive(Clone, Copy, Debug)]
struct RawCell {
    value: CellToken,
    span: Span,
}

#[derive(Clone, Debug, Default)]
struct RawFold {
    span: Span,
    row: Option<RawRow>,
}

#[derive(Clone, Debug, Default)]
struct RawBoard {
    span: Span,
    grid_closed: bool,
    size: Option<hir::Spanned<hir::BoardSize>>,
    rows: Vec<RawRow>,
    folded_blocks: BTreeMap<Slot, RawFold>,
}

#[derive(Clone, Debug, Default)]
struct RawCodeGrid {
    declared: bool,
    span: Span,
    size: Option<hir::Spanned<hir::BoardSize>>,
    main: RawBoard,
    functions: BTreeMap<Slot, RawBoard>,
}

#[derive(Clone, Copy, Debug)]
enum FrameKind {
    CodeGrid(CodeGridPath),
    Board(BoardPath),
    FoldedBlock { board: BoardPath, slot: Slot },
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    kind: FrameKind,
    span: Span,
}

struct Builder<'a> {
    source: &'a str,
    outer: RawCodeGrid,
    customs: BTreeMap<Slot, RawCodeGrid>,
    defined_boards: BTreeSet<BoardPath>,
    explicit_main: bool,
    global_size: Option<hir::Spanned<hir::BoardSize>>,
    frames: Vec<Frame>,
    diagnostics: Vec<Diagnostic>,
}

pub(super) fn build_hir(
    source: &str,
    parsed: ParsedSource,
) -> (Option<hir::Program>, Vec<Diagnostic>) {
    let mut builder = Builder::new(source, parsed.diagnostics);

    for item in parsed.items {
        match item {
            SyntaxItem::Directive(directive) => builder.directive(directive),
            SyntaxItem::GridRow { cells, span } => builder.grid_row(
                cells
                    .into_iter()
                    .map(|cell| RawCell {
                        value: cell.value,
                        span: cell.span,
                    })
                    .collect(),
                span,
            ),
        }
    }

    builder.close_unterminated_frames();
    builder.validate_program_structure();
    if has_errors(&builder.diagnostics) {
        return (None, builder.diagnostics);
    }

    let mut diagnostics = builder.diagnostics;
    let outer = builder.outer;
    let raw_customs = builder.customs;
    let global_size = builder.global_size;
    let Some(main) = finalize_codegrid(outer, CodeGridPath::Main, global_size, &mut diagnostics)
    else {
        return (None, diagnostics);
    };

    let mut customs = BTreeMap::new();
    for (slot, raw) in raw_customs {
        if !raw.declared {
            continue;
        }
        let span = raw.span;
        if let Some(codegrid) = finalize_codegrid(
            raw,
            CodeGridPath::Custom(slot),
            global_size,
            &mut diagnostics,
        ) {
            customs.insert(
                slot,
                hir::Spanned {
                    value: codegrid,
                    span,
                },
            );
        }
    }

    if has_errors(&diagnostics) {
        return (None, diagnostics);
    }
    (Some(hir::Program { main, customs }), diagnostics)
}

pub(super) fn completion_context(
    source: &str,
    parsed: ParsedSource,
    byte_offset: usize,
) -> CompletionContext {
    let mut declarations = Builder::new(source, parsed.diagnostics.clone());
    for item in parsed.items.iter().cloned() {
        declarations.apply_item(item);
    }

    let mut builder = Builder::new(source, parsed.diagnostics);
    for item in parsed.items {
        let span = match &item {
            SyntaxItem::Directive(directive) => directive.span,
            SyntaxItem::GridRow { span, .. } => *span,
        };
        if span.end > byte_offset {
            break;
        }
        builder.apply_item(item);
    }
    builder.completion_context(&declarations)
}

impl<'a> Builder<'a> {
    fn new(source: &'a str, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            source,
            outer: RawCodeGrid::default(),
            customs: BTreeMap::new(),
            defined_boards: BTreeSet::new(),
            explicit_main: false,
            global_size: None,
            frames: Vec::new(),
            diagnostics,
        }
    }

    fn apply_item(&mut self, item: SyntaxItem) {
        match item {
            SyntaxItem::Directive(directive) => self.directive(directive),
            SyntaxItem::GridRow { cells, span } => self.grid_row(
                cells
                    .into_iter()
                    .map(|cell| RawCell {
                        value: cell.value,
                        span: cell.span,
                    })
                    .collect(),
                span,
            ),
        }
    }

    fn completion_context(&self, declarations: &Self) -> CompletionContext {
        let board = self.current_board();
        let codegrid = board_codegrid(board);
        let function_slots = declarations
            .defined_boards
            .iter()
            .filter_map(|path| match path {
                BoardPath::Function {
                    codegrid: owner,
                    slot,
                } if *owner == codegrid => Some(*slot),
                _ => None,
            })
            .collect();
        let custom_slots = declarations
            .customs
            .iter()
            .filter_map(|(slot, custom)| custom.declared.then_some(*slot))
            .collect();
        let folded_block_slots = declarations
            .raw_board(board)
            .map(|raw| raw.folded_blocks.keys().copied().collect())
            .unwrap_or_default();
        let has_grid = self
            .raw_board(board)
            .is_some_and(|raw| !raw.rows.is_empty());
        let inside_folded_block = self
            .frames
            .last()
            .is_some_and(|frame| matches!(frame.kind, FrameKind::FoldedBlock { .. }));
        let inside_codegrid = self
            .frames
            .last()
            .is_some_and(|frame| matches!(frame.kind, FrameKind::CodeGrid(_)));

        CompletionContext {
            codegrid,
            board,
            custom_slots,
            function_slots,
            folded_block_slots,
            has_grid,
            inside_folded_block,
            inside_codegrid,
            can_close_block: !self.frames.is_empty(),
        }
    }

    fn raw_board(&self, path: BoardPath) -> Option<&RawBoard> {
        match path {
            BoardPath::Main(CodeGridPath::Main) => Some(&self.outer.main),
            BoardPath::Main(CodeGridPath::Custom(slot)) => {
                self.customs.get(&slot).map(|custom| &custom.main)
            }
            BoardPath::Function { codegrid, slot } => match codegrid {
                CodeGridPath::Main => self.outer.functions.get(&slot),
                CodeGridPath::Custom(custom) => self.customs.get(&custom)?.functions.get(&slot),
            },
        }
    }
}

impl Builder<'_> {
    fn directive(&mut self, directive: ParsedDirective) {
        if self.inside_folded_block() && directive.kind != DirectiveKind::End {
            self.error(
                "source.folded_structure", "Only the Folded Block row and its @end are allowed inside a Folded Block definition.",
                directive.head_span,
            );
            return;
        }

        let active_board = self.current_board();
        let close_active_grid =
            directive.kind != DirectiveKind::Size && !self.board_mut(active_board).rows.is_empty();
        if close_active_grid {
            self.board_mut(active_board).grid_closed = true;
        }

        match directive.kind {
            DirectiveKind::Size => self.size_directive(&directive),
            DirectiveKind::End => self.end_directive(&directive),
            DirectiveKind::Definition(target) => self.definition(target, &directive),
        }
    }

    fn definition(&mut self, target: DefinitionTarget, directive: &ParsedDirective) {
        match target {
            DefinitionTarget::Main => self.open_codegrid(CodeGridPath::Main, directive),
            DefinitionTarget::Custom(slot) => {
                self.open_codegrid(CodeGridPath::Custom(slot), directive)
            }
            DefinitionTarget::RelativeFunction(slot) => {
                let Some(Frame {
                    kind: FrameKind::CodeGrid(codegrid),
                    ..
                }) = self.frames.last().copied()
                else {
                    self.error(
                        "source.definition_scope",
                        "A relative Function definition requires an open Main or Custom CodeGrid.",
                        directive.head_span,
                    );
                    return;
                };
                if self.require_current_grid(BoardPath::Main(codegrid), directive.head_span) {
                    self.open_board(BoardPath::Function { codegrid, slot }, directive);
                }
            }
            DefinitionTarget::RelativeFoldedBlock(slot) => {
                let board = match self.frames.last().map(|frame| frame.kind) {
                    Some(FrameKind::CodeGrid(codegrid)) => BoardPath::Main(codegrid),
                    Some(FrameKind::Board(board)) => board,
                    Some(FrameKind::FoldedBlock { .. }) => return,
                    None => BoardPath::Main(CodeGridPath::Main),
                };
                if self.require_current_grid(board, directive.head_span) {
                    self.open_folded_block(board, slot, directive);
                }
            }
            DefinitionTarget::Function { codegrid, slot } => {
                let scope_matches = match self.frames.last().map(|frame| frame.kind) {
                    None => true,
                    Some(FrameKind::CodeGrid(open_codegrid)) => open_codegrid == codegrid,
                    Some(FrameKind::Board(_)) | Some(FrameKind::FoldedBlock { .. }) => false,
                };
                if !scope_matches {
                    self.error(
                        "source.definition_scope", "A qualified Function definition must be declared at top level or in its owning CodeGrid.",
                        directive.head_span,
                    );
                    return;
                }
                let main_board = BoardPath::Main(codegrid);
                self.record_codegrid_span(codegrid, directive.head_span);
                self.board_mut(BoardPath::Function { codegrid, slot }).span = directive.head_span;
                if self.require_current_grid(main_board, directive.head_span) {
                    self.open_board(BoardPath::Function { codegrid, slot }, directive);
                }
            }
            DefinitionTarget::FoldedBlock { board, slot } => {
                let scope_matches = match self.frames.last().map(|frame| frame.kind) {
                    None => true,
                    Some(FrameKind::CodeGrid(open_codegrid)) => {
                        open_codegrid == board_codegrid(board)
                    }
                    Some(FrameKind::Board(open_board)) => open_board == board,
                    Some(FrameKind::FoldedBlock { .. }) => false,
                };
                if !scope_matches {
                    self.error(
                        "source.definition_scope", "A qualified Folded Block definition must be declared at top level, in its owning CodeGrid, or in its owning Board.",
                        directive.head_span,
                    );
                    return;
                }
                self.record_codegrid_span(board_codegrid(board), directive.head_span);
                if self.require_current_grid(board, directive.head_span) {
                    self.open_folded_block(board, slot, directive);
                } else if let CodeGridPath::Custom(custom) = board_codegrid(board) {
                    let declared = self
                        .customs
                        .get(&custom)
                        .is_some_and(|codegrid| codegrid.declared);
                    if !declared {
                        self.board_mut(board)
                            .folded_blocks
                            .entry(slot)
                            .or_insert_with(|| RawFold {
                                span: directive.head_span,
                                row: None,
                            });
                    }
                }
            }
            DefinitionTarget::RelativeFunctionFoldedBlock { function, slot } => {
                let Some(Frame {
                    kind: FrameKind::CodeGrid(codegrid),
                    ..
                }) = self.frames.last().copied()
                else {
                    self.error(
                        "source.definition_scope", "A relative Function Folded Block requires an open Main or Custom CodeGrid.",
                        directive.head_span,
                    );
                    return;
                };
                let board = BoardPath::Function {
                    codegrid,
                    slot: function,
                };
                if self.require_current_grid(board, directive.head_span) {
                    self.open_folded_block(board, slot, directive);
                }
            }
        }
    }

    fn open_codegrid(&mut self, codegrid: CodeGridPath, directive: &ParsedDirective) {
        if !self.frames.is_empty() {
            self.error(
                "source.nested_codegrid",
                "A Main or Custom CodeGrid definition cannot be nested inside another definition.",
                directive.head_span,
            );
            return;
        }

        match codegrid {
            CodeGridPath::Main => {
                if self.explicit_main {
                    self.error(
                        "source.duplicate_definition",
                        "@main is defined more than once.",
                        directive.head_span,
                    );
                    return;
                }
                if !self.outer.main.rows.is_empty() {
                    self.error(
                        "source.mixed_main",
                        "Implicit Main rows cannot be combined with an explicit @main block.",
                        directive.head_span,
                    );
                }
                self.explicit_main = true;
                self.outer.declared = true;
                self.outer.span = directive.head_span;
                if self.outer.main.span == Span::default() {
                    self.outer.main.span = directive.head_span;
                }
            }
            CodeGridPath::Custom(slot) => {
                let duplicate = self
                    .customs
                    .get(&slot)
                    .is_some_and(|custom| custom.declared);
                if duplicate {
                    self.error(
                        "source.duplicate_definition",
                        "A Custom CodeGrid is defined more than once.",
                        directive.head_span,
                    );
                    return;
                }
                let custom = self.customs.entry(slot).or_default();
                custom.declared = true;
                custom.span = directive.head_span;
                if custom.main.span == Span::default() {
                    custom.main.span = directive.head_span;
                }
            }
        }

        self.frames.push(Frame {
            kind: FrameKind::CodeGrid(codegrid),
            span: directive.head_span,
        });
    }

    fn open_board(&mut self, path: BoardPath, directive: &ParsedDirective) {
        if !self.defined_boards.insert(path) {
            self.error(
                "source.duplicate_definition",
                "A structural board path is defined more than once.",
                directive.head_span,
            );
            return;
        }
        self.record_codegrid_span(board_codegrid(path), directive.head_span);
        let board = self.board_mut(path);
        board.span = directive.head_span;
        self.frames.push(Frame {
            kind: FrameKind::Board(path),
            span: directive.head_span,
        });
    }

    fn open_folded_block(
        &mut self,
        board_path: BoardPath,
        slot: Slot,
        directive: &ParsedDirective,
    ) {
        if self.inside_folded_block() {
            self.error(
                "source.nested_fold",
                "Folded Blocks cannot be nested.",
                directive.head_span,
            );
            return;
        }
        self.record_codegrid_span(board_codegrid(board_path), directive.head_span);
        let duplicate = self.board_mut(board_path).folded_blocks.contains_key(&slot);
        if duplicate {
            self.error(
                "source.duplicate_definition",
                "A Folded Block path is defined more than once.",
                directive.head_span,
            );
            return;
        }
        self.board_mut(board_path).folded_blocks.insert(
            slot,
            RawFold {
                span: directive.head_span,
                row: None,
            },
        );

        if directive.arguments.is_empty() {
            self.frames.push(Frame {
                kind: FrameKind::FoldedBlock {
                    board: board_path,
                    slot,
                },
                span: directive.head_span,
            });
            return;
        }

        if let Some(row) = self.row_from_argument_spans(&directive.arguments, directive.span) {
            if let Some(fold) = self.board_mut(board_path).folded_blocks.get_mut(&slot) {
                fold.row = Some(row);
            }
        }
    }

    fn grid_row(&mut self, cells: Vec<RawCell>, span: Span) {
        let row = RawRow { cells, span };

        if let Some(Frame {
            kind: FrameKind::FoldedBlock { board, slot },
            ..
        }) = self.frames.last().copied()
        {
            let already_has_row = self
                .board_mut(board)
                .folded_blocks
                .get(&slot)
                .is_some_and(|fold| fold.row.is_some());
            if already_has_row {
                self.error(
                    "source.fold_row_count",
                    "A Folded Block contains exactly one row.",
                    span,
                );
            } else if let Some(fold) = self.board_mut(board).folded_blocks.get_mut(&slot) {
                fold.row = Some(row);
            }
            return;
        }

        if self.frames.is_empty() && self.explicit_main {
            self.error(
                "source.grid_scope",
                "Grid rows outside the explicit @main block are not allowed.",
                span,
            );
            return;
        }
        let board_path = self.current_board();
        let grid_closed = self.board_mut(board_path).grid_closed;
        if grid_closed {
            self.error(
                "source.grid_order",
                "Grid rows for a board must be contiguous and precede its nested definitions.",
                span,
            );
            return;
        }
        let board = self.board_mut(board_path);
        if board.span == Span::default() {
            board.span = span;
        }
        board.rows.push(row);
    }

    fn size_directive(&mut self, directive: &ParsedDirective) {
        if self.inside_folded_block() {
            self.error(
                "source.size_scope",
                "@size is not allowed inside a Folded Block.",
                directive.head_span,
            );
            return;
        }
        let Some(argument) = directive.arguments.first().copied() else {
            self.error(
                "source.size_argument_count",
                "@size requires one WIDTHxHEIGHT value.",
                directive.span,
            );
            return;
        };
        let Some(value) = self.source.get(argument.start..argument.end) else {
            self.error(
                "source.invalid_span",
                "Invalid source span for @size value.",
                argument,
            );
            return;
        };
        let size = match parse_size(value) {
            Ok(size) => hir::Spanned {
                value: hir::BoardSize {
                    width: size.width,
                    height: size.height,
                },
                span: argument,
            },
            Err(_) => return,
        };
        if !dimensions_within_portable_limits(size.value.width, size.value.height) {
            self.error(
                "source.geometry_limit",
                "@size dimensions or total cell count exceed the portable Full limit.",
                argument,
            );
            return;
        }

        let duplicate = match self.frames.last().map(|frame| frame.kind) {
            None => self.global_size.is_some(),
            Some(FrameKind::CodeGrid(codegrid)) => self.codegrid_mut(codegrid).size.is_some(),
            Some(FrameKind::Board(board)) => self.board_mut(board).size.is_some(),
            Some(FrameKind::FoldedBlock { .. }) => {
                self.error(
                    "source.size_scope",
                    "@size is not allowed inside a Folded Block.",
                    directive.head_span,
                );
                return;
            }
        };
        if duplicate {
            let scope = match self.frames.last().map(|frame| frame.kind) {
                None => "program scope",
                Some(FrameKind::CodeGrid(_)) => "CodeGrid scope",
                Some(FrameKind::Board(_)) => "Function Board scope",
                Some(FrameKind::FoldedBlock { .. }) => "Folded Block scope",
            };
            self.error(
                "source.duplicate_size",
                format!("@size is repeated in the {scope}."),
                directive.head_span,
            );
            return;
        }

        match self.frames.last().map(|frame| frame.kind) {
            None => self.global_size = Some(size),
            Some(FrameKind::CodeGrid(codegrid)) => self.codegrid_mut(codegrid).size = Some(size),
            Some(FrameKind::Board(board)) => self.board_mut(board).size = Some(size),
            Some(FrameKind::FoldedBlock { .. }) => return,
        }
    }

    fn end_directive(&mut self, directive: &ParsedDirective) {
        let Some(frame) = self.frames.pop() else {
            self.error(
                "source.unexpected_end",
                "@end has no open definition to close.",
                directive.head_span,
            );
            return;
        };
        if let Some(name_span) = directive.arguments.first().copied() {
            let Some(name) = self.source.get(name_span.start..name_span.end) else {
                self.error(
                    "source.invalid_span",
                    "Invalid source span for @end name.",
                    name_span,
                );
                return;
            };
            if !end_name_matches(frame.kind, name) {
                self.error(
                    "source.end_mismatch",
                    "@end name does not match the open definition.",
                    name_span,
                );
            }
        }
        if let FrameKind::FoldedBlock { board, slot } = frame.kind {
            let missing_row = self
                .board_mut(board)
                .folded_blocks
                .get(&slot)
                .is_some_and(|fold| fold.row.is_none());
            if missing_row {
                self.error(
                    "source.fold_row_count",
                    "A Folded Block definition requires exactly one row.",
                    frame.span,
                );
            }
        }
    }

    fn close_unterminated_frames(&mut self) {
        let unterminated: Vec<Span> = self.frames.iter().rev().map(|frame| frame.span).collect();
        for span in unterminated {
            self.error(
                "source.missing_end",
                "Definition is missing its closing @end.",
                span,
            );
        }
        self.frames.clear();
    }

    fn validate_program_structure(&mut self) {
        let has_functions = !self.outer.functions.is_empty();
        let has_customs = self.customs.values().any(|custom| custom.declared);
        if (has_functions || has_customs) && !self.explicit_main {
            let span = self.outer.span;
            self.error(
                "source.explicit_main_required",
                "An explicit @main block is required when the source defines F or C boards.",
                span,
            );
        }

        let mut orphaned_customs = Vec::new();
        let mut invalid_customs = Vec::new();
        let mut empty_custom_functions = Vec::new();
        for (slot, custom) in &self.customs {
            let has_orphaned_definitions = !custom.functions.is_empty()
                || !custom.main.folded_blocks.is_empty()
                || custom
                    .functions
                    .values()
                    .any(|board| !board.folded_blocks.is_empty());
            if !custom.declared && has_orphaned_definitions {
                orphaned_customs.push((slot.get(), custom.span));
            }
            if custom.declared && custom.main.rows.is_empty() {
                invalid_customs.push(custom.span);
            }
            for board in custom.functions.values() {
                if board.rows.is_empty() {
                    empty_custom_functions.push(board.span);
                }
            }
        }
        for (slot, span) in orphaned_customs {
            self.error(
                "source.undefined_custom",
                format!("A qualified definition refers to undefined Custom C{slot}."),
                span,
            );
        }
        for span in invalid_customs {
            self.error(
                "source.missing_grid",
                "A declared Custom CodeGrid requires a Main board grid.",
                span,
            );
        }
        for span in empty_custom_functions {
            self.error(
                "source.missing_grid",
                "A declared function requires a board grid.",
                span,
            );
        }

        let empty_outer_functions: Vec<Span> = self
            .outer
            .functions
            .values()
            .filter(|board| board.rows.is_empty())
            .map(|board| board.span)
            .collect();
        for span in empty_outer_functions {
            self.error(
                "source.missing_grid",
                "A declared function requires a board grid.",
                span,
            );
        }
        if self.outer.main.rows.is_empty() {
            let span = self.outer.span;
            self.error(
                "source.missing_grid",
                "The program requires a Main board grid.",
                span,
            );
        }
    }

    fn current_board(&self) -> BoardPath {
        if let Some(frame) = self.frames.last() {
            match frame.kind {
                FrameKind::Board(board) | FrameKind::FoldedBlock { board, .. } => return board,
                FrameKind::CodeGrid(codegrid) => return BoardPath::Main(codegrid),
            }
        }
        BoardPath::Main(CodeGridPath::Main)
    }

    fn require_current_grid(&mut self, board_path: BoardPath, span: Span) -> bool {
        if !self.board_mut(board_path).rows.is_empty() {
            true
        } else {
            self.error(
                "source.grid_order",
                "A board grid must begin before its nested definitions.",
                span,
            );
            false
        }
    }

    fn inside_folded_block(&self) -> bool {
        matches!(
            self.frames.last().map(|frame| frame.kind),
            Some(FrameKind::FoldedBlock { .. })
        )
    }

    fn scope_main_mut(&mut self, path: CodeGridPath) -> &mut RawBoard {
        match path {
            CodeGridPath::Main => &mut self.outer.main,
            CodeGridPath::Custom(slot) => &mut self.customs.entry(slot).or_default().main,
        }
    }

    fn codegrid_mut(&mut self, path: CodeGridPath) -> &mut RawCodeGrid {
        match path {
            CodeGridPath::Main => &mut self.outer,
            CodeGridPath::Custom(slot) => self.customs.entry(slot).or_default(),
        }
    }

    fn record_codegrid_span(&mut self, codegrid: CodeGridPath, span: Span) {
        if let CodeGridPath::Custom(slot) = codegrid {
            let custom = self.customs.entry(slot).or_default();
            if custom.span == Span::default() {
                custom.span = span;
            }
        }
    }

    fn board_mut(&mut self, path: BoardPath) -> &mut RawBoard {
        match path {
            BoardPath::Main(codegrid) => self.scope_main_mut(codegrid),
            BoardPath::Function { codegrid, slot } => match codegrid {
                CodeGridPath::Main => self.outer.functions.entry(slot).or_default(),
                CodeGridPath::Custom(custom) => self
                    .customs
                    .entry(custom)
                    .or_default()
                    .functions
                    .entry(slot)
                    .or_default(),
            },
        }
    }

    fn row_from_argument_spans(&mut self, spans: &[Span], row_span: Span) -> Option<RawRow> {
        let mut cells = Vec::with_capacity(spans.len());
        for span in spans {
            let Some(text) = self.source.get(span.start..span.end) else {
                self.error("source.invalid_span", "Invalid cell source span.", *span);
                return None;
            };
            match parse_cell_token(text) {
                Ok(value) => cells.push(RawCell { value, span: *span }),
                Err(error) => {
                    self.error(error.code, error.message, *span);
                    return None;
                }
            }
        }
        Some(RawRow {
            cells,
            span: row_span,
        })
    }

    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
}

fn finalize_codegrid(
    mut raw: RawCodeGrid,
    codegrid_path: CodeGridPath,
    global_size: Option<hir::Spanned<hir::BoardSize>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<hir::CodeGrid> {
    let custom_owned = matches!(codegrid_path, CodeGridPath::Custom(_));
    let inherited_size = raw.size.or(global_size);
    raw.main.size = raw.main.size.or(inherited_size);
    for board in raw.functions.values_mut() {
        board.size = board.size.or(inherited_size);
    }
    let main_span = raw.main.span;
    let main = finalize_board(raw.main, true, custom_owned, diagnostics)?;

    let mut functions = BTreeMap::new();
    for (slot, board) in raw.functions {
        let span = board.span;
        if let Some(board) = finalize_board(board, false, custom_owned, diagnostics) {
            functions.insert(slot, hir::Spanned { value: board, span });
        }
    }
    Some(hir::CodeGrid {
        main: hir::Spanned {
            value: main,
            span: main_span,
        },
        functions,
    })
}

fn finalize_board(
    raw: RawBoard,
    is_main: bool,
    custom_owned: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<hir::Board> {
    if raw.rows.is_empty() {
        diagnostics.push(Diagnostic::error(
            "source.missing_grid",
            if is_main {
                "A Main board requires a grid."
            } else {
                "A declared function requires a board grid."
            },
            raw.span,
        ));
        return None;
    }

    let inferred_width = raw.rows.first().map_or(0, |row| row.cells.len());
    let width = raw.size.map_or(inferred_width, |size| size.value.width);
    let height = raw.size.map_or(raw.rows.len(), |size| size.value.height);
    let dimensions_valid = width > 0 && height > 0;
    if !dimensions_valid {
        diagnostics.push(Diagnostic::error(
            "source.zero_size",
            "Board width and height must be greater than zero.",
            raw.size.map_or(raw.span, |size| size.span),
        ));
    }
    if !dimensions_within_portable_limits(width, height) {
        diagnostics.push(Diagnostic::error(
            "source.geometry_limit",
            "Board dimensions or total cell count exceed the portable Full limit.",
            raw.size.map_or(raw.span, |size| size.span),
        ));
    }
    if raw.size.is_some() && raw.rows.len() != height {
        diagnostics.push(Diagnostic::error(
            "source.height_mismatch",
            format!(
                "Board has {} grid rows but its effective height is {height}.",
                raw.rows.len()
            ),
            raw.size.map_or(raw.span, |size| size.span),
        ));
    }

    let mut hir_rows = Vec::with_capacity(raw.rows.len());
    let mut entry_count = 0usize;
    for row in &raw.rows {
        if row.cells.len() != width {
            diagnostics.push(Diagnostic::error(
                "source.width_mismatch",
                format!(
                    "Grid row has {} cells but its effective width is {width}.",
                    row.cells.len()
                ),
                row.span,
            ));
        }
        let mut cells = Vec::with_capacity(row.cells.len());
        for cell in &row.cells {
            let value = match cell.value {
                CellToken::Empty => hir::Cell::Empty,
                CellToken::Entry(direction) => {
                    entry_count += 1;
                    hir::Cell::Entry(direction)
                }
                CellToken::Instruction {
                    primary,
                    attachment,
                } => {
                    validate_normal_placement(
                        primary,
                        is_main,
                        custom_owned,
                        cell.span,
                        diagnostics,
                    );
                    hir::Cell::Instruction {
                        primary,
                        attachment,
                    }
                }
            };
            cells.push(hir::Spanned {
                value,
                span: cell.span,
            });
        }
        hir_rows.push(cells);
    }

    if is_main && entry_count == 0 {
        diagnostics.push(Diagnostic::error(
            "source.main_entry_count",
            "The Main board must contain at least one Entry marker.",
            raw.span,
        ));
    } else if !is_main && entry_count != 1 {
        diagnostics.push(Diagnostic::error(
            "source.function_entry_count",
            format!("A function board must contain exactly one Entry marker; found {entry_count}."),
            raw.span,
        ));
    }

    let mut folded_blocks = BTreeMap::new();
    for (slot, raw_fold) in raw.folded_blocks {
        let Some(row) = raw_fold.row else {
            diagnostics.push(Diagnostic::error(
                "source.fold_row_count",
                "A Folded Block definition requires exactly one row.",
                raw_fold.span,
            ));
            continue;
        };
        if row.cells.len() != width {
            diagnostics.push(Diagnostic::error(
                "source.fold_width",
                format!(
                    "Folded Block has {} cells but must match owner width {width}.",
                    row.cells.len()
                ),
                row.span,
            ));
        }
        let mut cells = Vec::with_capacity(row.cells.len());
        for cell in row.cells {
            let primary = match cell.value {
                CellToken::Empty => None,
                CellToken::Entry(_) => {
                    diagnostics.push(Diagnostic::error(
                        "source.fold_entry",
                        "Entry markers are not allowed inside a Folded Block.",
                        cell.span,
                    ));
                    None
                }
                CellToken::Instruction {
                    primary,
                    attachment,
                } => {
                    if attachment.is_some() {
                        diagnostics.push(Diagnostic::error(
                            "source.fold_attachment",
                            "Attachments are not allowed inside a Folded Block.",
                            cell.span,
                        ));
                    }
                    if !folded_primary_allowed(primary, custom_owned) {
                        diagnostics.push(Diagnostic::error(
                            "source.fold_primary",
                            "This Primary instruction is not allowed inside this Folded Block.",
                            cell.span,
                        ));
                    }
                    Some(primary)
                }
            };
            cells.push(hir::Spanned {
                value: primary,
                span: cell.span,
            });
        }
        folded_blocks.insert(
            slot,
            hir::Spanned {
                value: hir::FoldedBlock { cells },
                span: raw_fold.span,
            },
        );
    }

    Some(hir::Board {
        size: raw.size,
        rows: hir_rows,
        folded_blocks,
    })
}

fn validate_normal_placement(
    primary: PrimaryInstruction,
    is_main: bool,
    custom_owned: bool,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match primary {
        PrimaryInstruction::Return if is_main => diagnostics.push(Diagnostic::error(
            "source.return_scope",
            "RETURN is allowed only on a function board.",
            span,
        )),
        PrimaryInstruction::CustomReturn if !custom_owned || !is_main => {
            diagnostics.push(Diagnostic::error(
                "source.custom_return_scope",
                "CUSTOM_RETURN is allowed only directly on a Custom Main board.",
                span,
            ));
        }
        PrimaryInstruction::Custom(_) if custom_owned => diagnostics.push(Diagnostic::error(
            "source.custom_call_scope",
            "Custom Instructions cannot be called from inside a Custom definition.",
            span,
        )),
        _ => {}
    }
}

fn folded_primary_allowed(primary: PrimaryInstruction, custom_owned: bool) -> bool {
    match primary {
        PrimaryInstruction::FoldedBlock(_)
        | PrimaryInstruction::Call(_)
        | PrimaryInstruction::Return
        | PrimaryInstruction::CustomReturn => false,
        PrimaryInstruction::Custom(_) => !custom_owned,
        _ => true,
    }
}

fn board_codegrid(board: BoardPath) -> CodeGridPath {
    match board {
        BoardPath::Main(codegrid) | BoardPath::Function { codegrid, .. } => codegrid,
    }
}

fn dimensions_within_portable_limits(width: usize, height: usize) -> bool {
    let (Ok(width), Ok(height)) = (u64::try_from(width), u64::try_from(height)) else {
        return false;
    };
    width > 0
        && height > 0
        && width <= MAX_BOARD_DIMENSION
        && height <= MAX_BOARD_DIMENSION
        && width
            .checked_mul(height)
            .is_some_and(|cells| cells <= MAX_BOARD_CELLS)
}

fn end_name_matches(kind: FrameKind, name: &str) -> bool {
    let (local, qualified) = match kind {
        FrameKind::CodeGrid(CodeGridPath::Main) => ("main".to_owned(), "main".to_owned()),
        FrameKind::CodeGrid(CodeGridPath::Custom(slot)) => {
            let local = format!("C{}", slot.get());
            (local.clone(), local)
        }
        FrameKind::Board(BoardPath::Main(codegrid)) => {
            let local = "main".to_owned();
            let qualified = codegrid_name(codegrid);
            (local, qualified)
        }
        FrameKind::Board(BoardPath::Function { codegrid, slot }) => {
            let local = format!("F{}", slot.get());
            let qualified = format!("{}.{}", codegrid_name(codegrid), local);
            (local, qualified)
        }
        FrameKind::FoldedBlock { board, slot } => {
            let local = format!("M{}", slot.get());
            let qualified = format!("{}.{}", board_name(board), local);
            (local, qualified)
        }
    };
    name.eq_ignore_ascii_case(&local) || name.eq_ignore_ascii_case(&qualified)
}

fn codegrid_name(codegrid: CodeGridPath) -> String {
    match codegrid {
        CodeGridPath::Main => "main".to_owned(),
        CodeGridPath::Custom(slot) => format!("C{}", slot.get()),
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

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
