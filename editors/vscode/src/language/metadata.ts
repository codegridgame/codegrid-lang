import * as vscode from 'vscode';

/**
 * Standalone editor presentation metadata for the Full CodeGrid token
 * inventory. Rust syntax/compiler APIs remain authoritative for acceptance.
 */

export interface PrimaryInfo {
  /** Exact, case-sensitive source spelling. */
  token: string;
  name: string;
  code: number | null;
  family:
    | 'direction' | 'random' | 'compare' | 'read' | 'clear' | 'arithmetic'
    | 'pointer' | 'output' | 'stack' | 'encoding' | 'call' | 'return'
    | 'nand' | 'memory' | 'page' | 'shift' | 'folded' | 'custom' | 'halt';
  summary: string;
}

export interface EntryInfo {
  token: string;
  direction: 'Up' | 'Down' | 'Left' | 'Right';
  summary: string;
}

function primary(
  token: string,
  name: string,
  code: number | null,
  family: PrimaryInfo['family'],
  summary?: string
): PrimaryInfo {
  return { token, name, code, family, summary: summary ? vscode.l10n.t(summary) : vscode.l10n.t('{0} Primary instruction.', name) };
}

/** The canonical Full Primary token set, kept in sync with codegrid-model. */
export const PRIMARIES: PrimaryInfo[] = [
  primary('^', 'Direction Up', 94, 'direction', 'Sets the direction to Up.'),
  primary('v', 'Direction Down', 118, 'direction', 'Sets the direction to Down.'),
  primary('<', 'Direction Left', 60, 'direction', 'Sets the direction to Left.'),
  primary('>', 'Direction Right', 62, 'direction', 'Sets the direction to Right.'),
  primary('??', 'Random Direction', 126, 'random'),
  primary('?=', 'CMP', 124, 'compare', 'Compares Data Stack top A without popping with the selected register B. Writes 0 if equal, 1 if A > B, or 2 if A < B. An empty stack preserves the register.'),
  primary(',^', 'READ Up', 138, 'read', 'Reads one input byte into R0. If input is exhausted, leaves R0 unchanged and turns Up.'),
  primary(',v', 'READ Down', 162, 'read', 'Reads one input byte into R0. If input is exhausted, leaves R0 unchanged and turns Down.'),
  primary(',<', 'READ Left', 104, 'read', 'Reads one input byte into R0. If input is exhausted, leaves R0 unchanged and turns Left.'),
  primary(',>', 'READ Right', 106, 'read', 'Reads one input byte into R0. If input is exhausted, leaves R0 unchanged and turns Right.'),
  primary('!', 'CLEAR', 33, 'clear'),
  primary('+', 'ADD', 43, 'arithmetic', 'Increments R0 with 8-bit wrapping.'),
  primary('-', 'SUB', 45, 'arithmetic', 'Decrements R0 with 8-bit wrapping.'),
  primary('{', 'MOVE_REGISTER_POINTER_LEFT', 123, 'pointer'),
  primary('}', 'MOVE_REGISTER_POINTER_RIGHT', 125, 'pointer'),
  primary('.', 'OUTPUT', 46, 'output', 'Appends the current R0 byte to output.'),
  ...Array.from({ length: 10 }, (_, digit) =>
    primary(`.${digit}`, `Immediate Output ${digit}`, null, 'output',
      'Outputs a literal byte without changing registers or the register pointer. In Custom code, pushes onto the caller stack. No Instruction Code or suffix Attachments; conditional prefixes are allowed.')
  ),
  primary('(', 'PUSH', 40, 'stack'),
  primary(')', 'POP_ADD', 41, 'stack'),
  primary('&', 'DECODE', 38, 'encoding'),
  primary('%', 'ENCODE', 37, 'encoding'),
  ...Array.from({ length: 10 }, (_, slot) =>
    primary(`[${slot}`, `CALL ${slot}`, 139 + slot, 'call')
  ),
  primary(']', 'RETURN', 93, 'return'),
  primary('$&', 'NAND', 74, 'nand'),
  primary('$(', 'MEMORY_LOAD', 76, 'memory'),
  primary('$)', 'MEMORY_STORE', 77, 'memory'),
  primary('$+', 'PAGE_INC', 79, 'page'),
  primary('$-', 'PAGE_DEC', 81, 'page'),
  primary('$<', 'SHIFT_LEFT', 96, 'shift', 'Shifts R0 left by one bit; the high bit is discarded and zero enters the low bit.'),
  primary('$>', 'SHIFT_RIGHT', 98, 'shift', 'Shifts R0 right logically by one bit; zero enters the high bit.'),
  ...Array.from({ length: 10 }, (_, slot) =>
    primary(`$${slot}`, `FOLDED_BLOCK ${slot}`, null, 'folded')
  ),
  ...Array.from({ length: 10 }, (_, slot) =>
    primary(`#${slot}`, `CUSTOM ${slot}`, null, 'custom')
  ),
  primary('#]', 'CUSTOM_RETURN', null, 'custom'),
  primary(';', 'HALT', null, 'halt', 'Halts normally after this successful tick. The VM does not move afterward.'),
];

export const ENTRIES: EntryInfo[] = [
  { token: '~^', direction: 'Up', summary: vscode.l10n.t('Entry marker facing Up.') },
  { token: '~v', direction: 'Down', summary: vscode.l10n.t('Entry marker facing Down.') },
  { token: '~<', direction: 'Left', summary: vscode.l10n.t('Entry marker facing Left.') },
  { token: '~>', direction: 'Right', summary: vscode.l10n.t('Entry marker facing Right.') },
];

export const EMPTY_CELL = {
  token: '_',
  summary: vscode.l10n.t('Empty cell.'),
};

export type CellToken =
  | { kind: 'empty' }
  | { kind: 'entry'; entry: EntryInfo }
  | { kind: 'primary'; primary: PrimaryInfo; prefix?: ConditionToken; attachment?: AttachmentToken };

export type ConditionToken = '?0' | '?1' | '?2';
export const CONDITIONS: ConditionToken[] = ['?0', '?1', '?2'];

export type AttachmentToken = '*' | '=' | 'x2' | 'x3' | 'x4' | 'x5';

export interface PrimaryCellSpelling {
  token: string;
  primary: PrimaryInfo;
  prefix?: ConditionToken; attachment?: AttachmentToken;
}

/** Complete Primary cell spellings offered by static editor assistance. */
export const PRIMARY_CELL_SPELLINGS: PrimaryCellSpelling[] = PRIMARIES.flatMap((item) => {
  const spellings: PrimaryCellSpelling[] = [{ token: item.token, primary: item }];
  if (item.code !== null) {
    spellings.push(
      { token: `${item.token}*`, primary: item, attachment: '*' },
      { token: `${item.token}=`, primary: item, attachment: '=' }
    );
    if (item.family !== 'call' && item.family !== 'return') {
      for (const count of [2, 3, 4, 5] as const) {
        const attachment = `x${count}` as AttachmentToken;
        spellings.push({ token: `${item.token}${attachment}`, primary: item, attachment });
      }
    }
  }
  return spellings.flatMap((spelling) => [spelling, ...CONDITIONS.map((prefix) => ({ ...spelling, token: `${prefix}${spelling.token}`, prefix }))]);
});

const PRIMARY_CELL_BY_TOKEN = new Map(PRIMARY_CELL_SPELLINGS.map((item) => [item.token, item]));

/** Recognize one complete lexical cell token for editor presentation. */
export function parseCellToken(token: string): CellToken | null {
  if (token === EMPTY_CELL.token) return { kind: 'empty' };
  const entry = ENTRIES.find((item) => item.token === token);
  if (entry) return { kind: 'entry', entry };
  const spelling = PRIMARY_CELL_BY_TOKEN.get(token);
  return spelling
    ? { kind: 'primary', primary: spelling.primary, prefix: spelling.prefix, attachment: spelling.attachment }
    : null;
}

/** Notes for incomplete multi-character spellings shown by hover. */
export const PREFIX_NOTES: Record<string, string> = {
  '?': vscode.l10n.t('Complete as ?? for random direction, ?= for CMP, or ?0/?1/?2 before a Primary for a conditional prefix.'),
  '#': vscode.l10n.t('`#` must be completed as a Custom call or Custom return token.'),
  $: vscode.l10n.t('`$` must be completed as a NAND, memory, Page, shift, or Folded Block token.'),
  ',': vscode.l10n.t('`,` must be completed with one of the four READ directions.'),
  '[': vscode.l10n.t('`[` must be followed by one Function ID digit (`0` through `9`).'),
};
