import * as vscode from 'vscode';

/** Directive presentation and conservative formatter matching. */

export type DirectiveKind = 'main' | 'end' | 'size';

export interface DirectiveInfo {
  kind: DirectiveKind;
  canonical: string;
  title: string;
  summary: string;
}

/** Portable Full source bounds, identical on native and wasm32. */
const MAX_BOARD_DIMENSION = '4294967295';
const MAX_BOARD_CELLS = 4294967295n;

const SIZE_VALUE_RE = /^([0-9]+)x([0-9]+)$/;

/**
 * Validate a `WIDTHxHEIGHT` size value per source-spec §3 and return its
 * canonical spelling: ASCII decimal digits with optional leading zeros,
 * positive dimensions, and a bounded total cell count.
 */
export function canonicalizeSizeValue(value: string): string | null {
  const match = SIZE_VALUE_RE.exec(value);
  if (!match) return null;
  const width = stripLeadingZeros(match[1]);
  const height = stripLeadingZeros(match[2]);
  if (width === '' || height === '') return null;
  if (!fitsPortableDimension(width) || !fitsPortableDimension(height)) return null;
  if (BigInt(width) * BigInt(height) > MAX_BOARD_CELLS) return null;
  return `${width}x${height}`;
}

function stripLeadingZeros(digits: string): string {
  return digits.replace(/^0+/, '');
}

function fitsPortableDimension(digits: string): boolean {
  if (digits.length < MAX_BOARD_DIMENSION.length) return true;
  if (digits.length > MAX_BOARD_DIMENSION.length) return false;
  return digits <= MAX_BOARD_DIMENSION;
}

/** Match a complete directive line; malformed or legacy forms are declined. */
export function matchDirective(head: string, rest: string[]): DirectiveInfo | null {
  const normalized = head.trim().toLowerCase();
  if (normalized === '@main' && rest.length === 0) {
    return {
      kind: 'main',
      canonical: '@main',
      title: '@main',
      summary: vscode.l10n.t('Opens the program’s single Main Board.'),
    };
  }
  if (normalized === '@end' && (rest.length === 0 ||
      (rest.length === 1 && /^(?:main|[CFM][0-9])(?:\.[FM][0-9])*$/i.test(rest[0])))) {
    return {
      kind: 'end',
      canonical: rest.length === 1 ? `@end ${rest[0]}` : '@end',
      title: '@end',
      summary: vscode.l10n.t('Closes the current explicitly delimited definition.'),
    };
  }
  if (normalized === '@size' && rest.length === 1) {
    const dimensions = canonicalizeSizeValue(rest[0]);
    if (dimensions === null) return null;
    return {
      kind: 'size',
      canonical: `@size ${dimensions}`,
      title: `@size ${dimensions}`,
      summary:
        vscode.l10n.t('Declares the positive board dimensions as WIDTHxHEIGHT. Leading zeros are allowed.'),
    };
  }
  return null;
}
