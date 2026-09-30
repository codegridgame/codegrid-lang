/**
 * Conservative whole-document formatter for CodeGrid source.
 *
 * Formatting policy:
 * - Deterministic and idempotent: formatting the output again yields the same
 *   text.
 * - Preserves row and cell counts, cell token order, entry order, comments,
 *   and executable meaning. Whitespace between cells is normalized and
 *   consecutive grid rows are aligned into columns.
 * - Line endings: the dominant style of the document is detected from the
 *   first line break and applied to the whole document (LF unless a CRLF is
 *   seen first). Mixed endings are normalized to the detected style.
 * - Final newline: preserved exactly as in the input.
 * - Runs of blank lines collapse to a single blank line; leading and trailing
 *   blank lines are removed.
 * - Supported directive heads are canonicalized (`@main`, `@end`,
 *   `@size 5x5`).
 * - Conservative decline: if any line cannot be fully understood as comments,
 *   a valid directive, or a row of complete cell tokens (including
 *   nonrectangular row groups and unterminated block comments), the formatter
 *   returns null and the caller must leave the document unchanged.
 *
 * The formatter deliberately performs no semantic validation beyond complete
 * cell-token matching; it is not a second parser or validator.
 */

import { parseCellToken } from '../language/metadata';
import { matchDirective, type DirectiveKind } from '../language/directives';

type Line =
  | { kind: 'blank' }
  | { kind: 'comment'; text: string }
  | { kind: 'raw'; text: string }
  | { kind: 'directive'; indent: string; text: string; directiveKind: DirectiveKind }
  | { kind: 'cells'; indent: string; cells: string[]; comment: string | null };

interface ContentResult {
  lines: Line[];
  inBlock: boolean;
}

export function detectEol(text: string): string {
  const match = /\r\n|\n/.exec(text);
  return match && match[0] === '\r\n' ? '\r\n' : '\n';
}

function findComment(line: string): { type: 'line' | 'block'; index: number } | null {
  const lineIdx = line.indexOf('//');
  const blockIdx = line.indexOf('/*');
  if (lineIdx < 0 && blockIdx < 0) return null;
  if (blockIdx < 0 || (lineIdx >= 0 && lineIdx < blockIdx)) return { type: 'line', index: lineIdx };
  return { type: 'block', index: blockIdx };
}

function parseContent(line: string): ContentResult | null {
  const comment = findComment(line);
  if (comment && comment.type === 'block') {
    // Preserve lines containing inline block comments verbatim (trimmed of
    // trailing whitespace). Track whether the block stays open.
    const close = line.indexOf('*/', comment.index + 2);
    return { lines: [{ kind: 'raw', text: line }], inBlock: close < 0 };
  }
  if (!comment && line.trim() === '') {
    return { lines: [{ kind: 'blank' }], inBlock: false };
  }
  const codePart = comment ? line.slice(0, comment.index).trimEnd() : line;
  const commentText = comment ? line.slice(comment.index) : null;
  const trimmed = codePart.trim();
  if (trimmed === '') {
    // Comment-only line: preserved verbatim (already right-trimmed).
    return { lines: [{ kind: 'comment', text: line }], inBlock: false };
  }
  const indent = codePart.slice(0, codePart.length - codePart.trimStart().length);
  if (trimmed.startsWith('@')) {
    const tokens = trimmed.split(/\s+/);
    const directive = matchDirective(tokens[0], tokens.slice(1));
    if (!directive) return null;
    return {
      lines: [{ kind: 'directive', indent, text: directive.canonical + (commentText ? ` ${commentText}` : ''), directiveKind: directive.kind }],
      inBlock: false,
    };
  }
  const cells = trimmed.split(/\s+/);
  for (const cell of cells) {
    if (!parseCellToken(cell)) return null;
  }
  return { lines: [{ kind: 'cells', indent, cells, comment: commentText }], inBlock: false };
}

function parseLine(raw: string, inBlock: boolean): ContentResult | null {
  const line = raw.replace(/[ \t]+$/, '');
  if (!inBlock) return parseContent(line);
  const close = line.indexOf('*/');
  if (close < 0) {
    return { lines: [{ kind: 'comment', text: line }], inBlock: true };
  }
  const out: Line[] = [{ kind: 'comment', text: line.slice(0, close + 2) }];
  const rest = line.slice(close + 2).trim();
  if (rest === '') return { lines: out, inBlock: false };
  const inner = parseContent(rest);
  if (!inner) return null;
  return { lines: [...out, ...inner.lines], inBlock: inner.inBlock };
}

interface AlignedGroup {
  rows: Extract<Line, { kind: 'cells' }>[];
}

/**
 * Format CodeGrid source. Returns the formatted text, or null when the input
 * is not safely understood and must be left unchanged.
 */
export function formatCodeGrid(text: string): string | null {
  const eol = detectEol(text);
  const endsWithNewline = text.endsWith('\n');
  const rawLines = text.split(/\r?\n/);
  if (endsWithNewline) rawLines.pop();

  const lines: Line[] = [];
  let inBlock = false;
  for (const raw of rawLines) {
    const result = parseLine(raw, inBlock);
    if (!result) return null;
    lines.push(...result.lines);
    inBlock = result.inBlock;
  }
  if (inBlock) return null; // unterminated block comment

  // Collapse blank runs; drop leading/trailing blank lines.
  const collapsed: Line[] = [];
  for (const line of lines) {
    if (line.kind === 'blank') {
      const previous = collapsed[collapsed.length - 1];
      if (!previous || previous.kind === 'blank') continue;
    }
    collapsed.push(line);
  }
  while (collapsed.length > 0 && collapsed[collapsed.length - 1].kind === 'blank') {
    collapsed.pop();
  }

  // Group consecutive cell rows into alignment groups. Blank and comment
  // lines may interleave (they cannot appear between different boards, which
  // always require a directive line), so they do not break a group. A
  // directive or raw line ends the group.
  const groups: AlignedGroup[] = [];
  let current: AlignedGroup | null = null;
  for (const line of collapsed) {
    if (line.kind === 'cells') {
      if (!current) {
        current = { rows: [] };
        groups.push(current);
      }
      current.rows.push(line);
    } else if (line.kind === 'directive' && line.directiveKind !== 'size') {
      current = null;
    } else if (line.kind === 'raw') {
      current = null;
    }
  }

  for (const group of groups) {
    const counts = group.rows.map((row) => row.cells.length);
    if (group.rows.length > 1 && counts.some((c) => c !== counts[0])) {
      // Nonrectangular grid geometry: decline rather than rewrite.
      return null;
    }
    const widths: number[] = [];
    for (const row of group.rows) {
      row.cells.forEach((cell, i) => {
        widths[i] = Math.max(widths[i] ?? 0, cell.length);
      });
    }
    for (const row of group.rows) {
      row.cells = row.cells.map((cell, i) =>
        i < row.cells.length - 1 ? cell.padEnd(widths[i], ' ') : cell
      );
    }
  }

  const outLines = collapsed.map((line) => {
    switch (line.kind) {
      case 'blank':
        return '';
      case 'comment':
      case 'raw':
        return line.text;
      case 'directive':
        return line.indent + line.text;
      case 'cells': {
        const body = line.cells.join(' ');
        return line.indent + body + (line.comment ? ` ${line.comment}` : '');
      }
    }
  });

  return outLines.join(eol) + (endsWithNewline ? eol : '');
}
