use codegrid_model::PrimaryInstruction;

use crate::{
    parse_cell_token, parse_directive_head, parse_size, CellToken, DefinitionTarget, DirectiveKind,
};
use crate::{BoardPath, CodeGridPath};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Line {
    Blank,
    Comment(String),
    Raw(String),
    Directive {
        indent: String,
        text: String,
        inline_cells: Vec<String>,
        comment: Option<String>,
    },
    Cells {
        indent: String,
        cells: Vec<String>,
        comment: Option<String>,
    },
}

/// Conservatively formats complete CodeGrid source without semantic validation.
///
/// Returns `None` when a line cannot be safely understood or a grid-row group
/// has inconsistent widths. The caller should leave the original document
/// unchanged in that case. The formatter preserves comments, indentation,
/// first-seen line-ending style, and final-newline presence.
pub fn format_source(source: &str) -> Option<String> {
    if has_bare_carriage_return(source) {
        return None;
    }

    let eol = if source
        .find('\n')
        .is_some_and(|newline| newline > 0 && source.as_bytes()[newline - 1] == b'\r')
    {
        "\r\n"
    } else {
        "\n"
    };
    let has_final_newline = source.ends_with('\n');
    let mut raw_lines = source.split('\n').collect::<Vec<_>>();
    if has_final_newline {
        raw_lines.pop();
    }

    let mut lines = Vec::new();
    let mut in_block_comment = false;
    for raw in raw_lines {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let (mut parsed, remains_in_block) = parse_line(raw, in_block_comment)?;
        lines.append(&mut parsed);
        in_block_comment = remains_in_block;
    }
    if in_block_comment {
        return None;
    }

    let mut collapsed = Vec::with_capacity(lines.len());
    for line in lines {
        if line == Line::Blank && collapsed.last() == Some(&Line::Blank) {
            continue;
        }
        collapsed.push(line);
    }
    while collapsed.first() == Some(&Line::Blank) {
        collapsed.remove(0);
    }
    while collapsed.last() == Some(&Line::Blank) {
        collapsed.pop();
    }

    align_grid_groups(&mut collapsed)?;
    let output = collapsed
        .into_iter()
        .map(render_line)
        .collect::<Vec<_>>()
        .join(eol);
    Some(if has_final_newline {
        format!("{output}{eol}")
    } else {
        output
    })
}

fn has_bare_carriage_return(source: &str) -> bool {
    let bytes = source.as_bytes();
    bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'))
}

fn parse_line(raw: &str, in_block_comment: bool) -> Option<(Vec<Line>, bool)> {
    let line = raw.trim_end_matches(|character| matches!(character, ' ' | '\t'));
    if in_block_comment {
        if line
            .find("/*")
            .is_some_and(|nested| line.find("*/").map_or(true, |close| nested < close))
        {
            return None;
        }
        let Some(close) = line.find("*/") else {
            return Some((vec![Line::Comment(line.to_owned())], true));
        };
        let lines = vec![Line::Comment(line[..close + 2].to_owned())];
        let rest = line[close + 2..].trim_matches(|character| matches!(character, ' ' | '\t'));
        if rest.is_empty() {
            return Some((lines, false));
        }
        // The comment may have started after code on an earlier physical
        // line. Emitting this trailing code as a separate formatted line can
        // then split one logical grid row into two rows.
        return None;
    }
    parse_content(line)
}

fn parse_content(line: &str) -> Option<(Vec<Line>, bool)> {
    let line_comment = line.find("//");
    let block_comment = line.find("/*");
    let comment = match (line_comment, block_comment) {
        (Some(line), Some(block)) if line < block => Some((line, false)),
        (Some(_), Some(block)) => Some((block, true)),
        (Some(line), None) => Some((line, false)),
        (None, Some(block)) => Some((block, true)),
        (None, None) => None,
    };

    if let Some((start, is_block)) = comment {
        if is_block {
            let close = line[start + 2..]
                .find("*/")
                .map(|offset| start + 2 + offset);
            if line[start + 2..]
                .find("/*")
                .is_some_and(|nested| close.map_or(true, |close| start + 2 + nested < close))
            {
                return None;
            }
            let remains_in_block = close.is_none();
            return Some((vec![Line::Raw(line.to_owned())], remains_in_block));
        }
        let code = line[..start].trim_end_matches(|character| matches!(character, ' ' | '\t'));
        let comment_text = line[start..].to_owned();
        if code
            .trim_matches(|character| matches!(character, ' ' | '\t'))
            .is_empty()
        {
            return Some((vec![Line::Comment(line.to_owned())], false));
        }
        let (indent, body) = split_indent(code);
        let mut parsed = parse_code_body(body)?;
        attach_line_comment(&mut parsed, comment_text)?;
        return Some((vec![with_indent(parsed, indent)], false));
    }

    if line
        .trim_matches(|character| matches!(character, ' ' | '\t'))
        .is_empty()
    {
        return Some((vec![Line::Blank], false));
    }
    let (indent, body) = split_indent(line);
    Some((vec![with_indent(parse_code_body(body)?, indent)], false))
}

fn split_indent(line: &str) -> (&str, &str) {
    let indent_end = line
        .char_indices()
        .find(|(_, character)| !matches!(character, ' ' | '\t'))
        .map_or(line.len(), |(index, _)| index);
    line.split_at(indent_end)
}

fn parse_code_body(body: &str) -> Option<Line> {
    let atoms = body
        .split(|character| matches!(character, ' ' | '\t'))
        .filter(|atom| !atom.is_empty())
        .collect::<Vec<_>>();
    let first = *atoms.first()?;
    if first.starts_with('@') {
        let (text, inline_cells) = canonical_directive(first, &atoms[1..])?;
        return Some(Line::Directive {
            indent: String::new(),
            text,
            inline_cells,
            comment: None,
        });
    }

    let cells = atoms
        .into_iter()
        .map(|atom| parse_cell_token(atom).ok().map(render_cell))
        .collect::<Option<Vec<_>>>()?;
    Some(Line::Cells {
        indent: String::new(),
        cells,
        comment: None,
    })
}

fn with_indent(mut line: Line, indent: &str) -> Line {
    match &mut line {
        Line::Directive { indent: target, .. } | Line::Cells { indent: target, .. } => {
            target.push_str(indent);
        }
        _ => {}
    }
    line
}

fn attach_line_comment(line: &mut Line, comment: String) -> Option<()> {
    match line {
        Line::Cells {
            comment: target, ..
        } => {
            *target = Some(comment);
            Some(())
        }
        Line::Directive {
            comment: target, ..
        } => {
            *target = Some(comment);
            Some(())
        }
        _ => None,
    }
}

fn canonical_directive(head: &str, arguments: &[&str]) -> Option<(String, Vec<String>)> {
    let kind = parse_directive_head(head).ok()?;
    match kind {
        DirectiveKind::Definition(DefinitionTarget::Main) if arguments.is_empty() => {
            Some(("@main".to_owned(), Vec::new()))
        }
        DirectiveKind::Definition(target) => {
            let folded_block = is_folded_block(target);
            if !folded_block && !arguments.is_empty() {
                return None;
            }
            let cells = if folded_block {
                arguments
                    .iter()
                    .map(|atom| parse_cell_token(atom).ok().map(render_cell))
                    .collect::<Option<Vec<_>>>()?
            } else {
                Vec::new()
            };
            Some((render_definition_target(target), cells))
        }
        DirectiveKind::Size => {
            if arguments.len() != 1 {
                return None;
            }
            let size = parse_size(arguments[0]).ok()?;
            Some((format!("@size {}x{}", size.width, size.height), Vec::new()))
        }
        DirectiveKind::End if arguments.is_empty() => Some(("@end".to_owned(), Vec::new())),
        DirectiveKind::End if arguments.len() == 1 => {
            let closing_name = format!("@{}", arguments[0]);
            let DirectiveKind::Definition(target) = parse_directive_head(&closing_name).ok()?
            else {
                return None;
            };
            Some((
                format!(
                    "@end {}",
                    render_definition_target(target).trim_start_matches('@')
                ),
                Vec::new(),
            ))
        }
        DirectiveKind::End => None,
    }
}

fn is_folded_block(target: DefinitionTarget) -> bool {
    matches!(
        target,
        DefinitionTarget::RelativeFoldedBlock(_)
            | DefinitionTarget::FoldedBlock { .. }
            | DefinitionTarget::RelativeFunctionFoldedBlock { .. }
    )
}

fn render_definition_target(target: DefinitionTarget) -> String {
    match target {
        DefinitionTarget::Main => "@main".to_owned(),
        DefinitionTarget::Custom(slot) => format!("@C{}", slot.get()),
        DefinitionTarget::RelativeFunction(slot) => format!("@F{}", slot.get()),
        DefinitionTarget::RelativeFoldedBlock(slot) => format!("@M{}", slot.get()),
        DefinitionTarget::Function { codegrid, slot } => {
            format!("{}F{}", render_codegrid_path(codegrid), slot.get())
        }
        DefinitionTarget::FoldedBlock { board, slot } => {
            format!("{}M{}", render_board_path(board), slot.get())
        }
        DefinitionTarget::RelativeFunctionFoldedBlock { function, slot } => {
            format!("@F{}.M{}", function.get(), slot.get())
        }
    }
}

fn render_codegrid_path(codegrid: CodeGridPath) -> String {
    match codegrid {
        CodeGridPath::Main => "@main.".to_owned(),
        CodeGridPath::Custom(slot) => format!("@C{}.", slot.get()),
    }
}

fn render_board_path(board: BoardPath) -> String {
    match board {
        BoardPath::Main(codegrid) => render_codegrid_path(codegrid),
        BoardPath::Function { codegrid, slot } => {
            format!("{}F{}.", render_codegrid_path(codegrid), slot.get())
        }
    }
}

fn render_cell(cell: CellToken) -> String {
    match cell {
        CellToken::Empty => "_".to_owned(),
        CellToken::Entry(direction) => {
            format!("~{}", PrimaryInstruction::Direction(direction).token())
        }
        CellToken::Instruction {
            primary,
            attachment,
        } => {
            let mut token = primary.token();
            if let Some(attachment) = attachment {
                token.push_str(&attachment.token());
            }
            token
        }
    }
}

fn align_grid_groups(lines: &mut [Line]) -> Option<()> {
    let mut group = Vec::new();
    let mut block_depth = 0usize;
    for index in 0..=lines.len() {
        match lines.get(index) {
            Some(Line::Cells { .. }) => group.push(index),
            Some(Line::Directive { text, .. }) if text == "@size" || text.starts_with("@size ") => {
                continue;
            }
            Some(Line::Directive {
                text, inline_cells, ..
            }) => {
                let block_change = if text == "@end" || text.starts_with("@end ") {
                    -1
                } else if text == "@size" || text.starts_with("@size ") {
                    0
                } else if inline_cells.is_empty() {
                    1
                } else {
                    0
                };
                align_group(lines, &group, block_depth > 0)?;
                group.clear();
                if block_change < 0 {
                    block_depth = block_depth.saturating_sub(1);
                } else {
                    block_depth += block_change as usize;
                }
            }
            _ => {
                align_group(lines, &group, block_depth > 0)?;
                group.clear();
            }
        }
    }
    Some(())
}

fn align_group(lines: &mut [Line], row_indices: &[usize], align_columns: bool) -> Option<()> {
    if row_indices.len() < 2 {
        return Some(());
    }
    let first = match lines.get(*row_indices.first()?)? {
        Line::Cells { cells, .. } => cells.len(),
        _ => return None,
    };
    if row_indices.iter().any(|index| {
        !matches!(lines.get(*index), Some(Line::Cells { cells, .. }) if cells.len() == first)
    }) {
        return None;
    }
    if !align_columns {
        return Some(());
    }

    let mut column_widths = vec![0usize; first];
    for index in row_indices {
        if let Line::Cells { cells, .. } = &lines[*index] {
            for (column, cell) in cells.iter().enumerate() {
                column_widths[column] = column_widths[column].max(cell.len());
            }
        }
    }

    for index in row_indices {
        if let Line::Cells { cells, .. } = &mut lines[*index] {
            if let Some(first_cell) = cells.first_mut() {
                let padding = column_widths[0].saturating_sub(first_cell.len());
                first_cell.extend(std::iter::repeat(' ').take(padding));
            }
            for column in 1..cells.len() {
                let padding = column_widths[column].saturating_sub(cells[column].len());
                if padding > 0 {
                    cells[column].insert_str(0, &" ".repeat(padding));
                }
            }
        }
    }
    Some(())
}

fn render_line(line: Line) -> String {
    match line {
        Line::Blank => String::new(),
        Line::Comment(text) | Line::Raw(text) => text,
        Line::Directive {
            indent,
            text,
            inline_cells,
            comment,
        } => {
            let mut line = format!("{indent}{text}");
            if !inline_cells.is_empty() {
                line.push(' ');
                line.push_str(&inline_cells.join(" "));
            }
            if let Some(comment) = comment {
                line.push(' ');
                line.push_str(&comment);
            }
            line
        }
        Line::Cells {
            indent,
            cells,
            comment,
        } => {
            let mut line = format!("{indent}{}", cells.join(" "));
            if let Some(comment) = comment {
                line.push(' ');
                line.push_str(&comment);
            }
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn aligns_rows_and_normalizes_directives_and_cells() {
        let source = "@MAIN\n~v _ + _\n_ + > .\n@END\n";
        let expected = "@main\n~v _ + _\n_  + > .\n@end\n";
        assert_eq!(format_source(source).as_deref(), Some(expected));
    }

    #[test]
    fn size_directives_do_not_split_grid_row_formatting_groups() {
        let source = "@main\n~> _ +\n@size 3x2\n_ + .\n@end\n";
        let expected = "@main\n~> _ +\n@size 3x2\n_  + .\n@end\n";
        assert_eq!(format_source(source).as_deref(), Some(expected));
    }

    #[test]
    fn preserves_comments_indent_crlf_and_final_newline_policy() {
        let source = "  ~v   _\r\n// note\r\n  _ +\r\n";
        let expected = "  ~v _\r\n// note\r\n  _ +\r\n";
        assert_eq!(format_source(source).as_deref(), Some(expected));
        assert_eq!(format_source("~v _\n_ +").as_deref(), Some("~v _\n_ +"));
    }

    #[test]
    fn formats_full_structural_paths_inline_folded_cells_and_named_closers() {
        assert_eq!(
            format_source("@m0\n+ > _ ^\n@end m0\n").as_deref(),
            Some("@M0\n+ > _ ^\n@end M0\n")
        );
        assert_eq!(
            format_source("@MAIN   // keep\n~v _\n@END // close\n").as_deref(),
            Some("@main // keep\n~v _\n@end // close\n")
        );
        assert_eq!(
            format_source("@MAIN\n~> ;\n@END MAIN\n").as_deref(),
            Some("@main\n~> ;\n@end main\n")
        );
        assert_eq!(
            format_source("@C0.F0   // function\n~v ]\n@END C0.F0 // close\n").as_deref(),
            Some("@C0.F0 // function\n~v ]\n@end C0.F0 // close\n")
        );
        assert_eq!(
            format_source("@MAIN.M0    +x3  ,<* // inline\n").as_deref(),
            Some("@main.M0 +x3 ,<* // inline\n")
        );
        assert_eq!(
            format_source("@main.F0.M0\n+x2 _\n@end main.F0.M0\n").as_deref(),
            Some("@main.F0.M0\n+x2 _\n@end main.F0.M0\n")
        );
        assert_eq!(format_source("@main.C0\n"), None);
        assert_eq!(format_source("@m0   +  > _x2\n"), None);
        assert_eq!(
            format_source("~v _ /* keep\ninside\n*/\n").as_deref(),
            Some("~v _ /* keep\ninside\n*/\n")
        );
    }

    #[test]
    fn formatter_is_idempotent_and_declines_unrecognized_or_unsafe_input() {
        let source = "@main\n~> + ;\n@end\n";
        let once = format_source(source).expect("complete Full source should be formatable");
        assert_eq!(format_source(&once).as_deref(), Some(once.as_str()));
        assert_eq!(format_source("~x\n"), None);
        assert_eq!(format_source("~v _\n_\n"), None);
        assert_eq!(format_source("~> #\n"), None);
        assert_eq!(format_source("/* never closed"), None);
        assert_eq!(format_source("~v _\r_ +"), None);
    }

    #[test]
    fn declines_multiline_comment_that_would_split_a_logical_grid_row() {
        let source = "~v _ /* comment starts\n*/ _ _\n";
        assert_eq!(format_source(source), None);
    }
}
