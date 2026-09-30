import * as vscode from 'vscode';

/** Localized Markdown descriptions used by the standalone hover provider. */

import { EMPTY_CELL, PREFIX_NOTES, parseCellToken } from './metadata';
import { canonicalizeSizeValue, matchDirective } from './directives';

function codeLine(code: number | null): string {
  return code === null ? '' : '\n\n*' + vscode.l10n.t('Instruction code: {0}.', code) + '*';
}

/** Describe a source token under the cursor when presentation metadata exists. */
export function describeToken(token: string): string | null {
  const trimmed = token.trim();
  if (!trimmed) return null;

  if (trimmed.startsWith('@')) {
    const directive = matchDirective(trimmed, []);
    if (directive) return `**${directive.title}**\n\n${directive.summary}`;
    if (/^@size$/i.test(trimmed)) {
      return '**@size**\n\n' + vscode.l10n.t('Declares positive board dimensions as `@size WIDTHxHEIGHT`, for example `@size 5x5`.');
    }
    return null;
  }

  if (canonicalizeSizeValue(trimmed) !== null) {
    return `**${vscode.l10n.t('Board size')}** \`${trimmed}\`\n\n${vscode.l10n.t('WIDTHxHEIGHT with positive decimal dimensions; leading zeros are allowed.')}`;
  }

  const cell = parseCellToken(trimmed);
  if (cell) {
    switch (cell.kind) {
      case 'empty':
        return `**${vscode.l10n.t('Empty cell')}** \`_\`\n\n${EMPTY_CELL.summary}`;
      case 'entry':
        return `**${vscode.l10n.t('Entry marker')}** \`${cell.entry.token}\` — ${vscode.l10n.t(cell.entry.direction)}\n\n${cell.entry.summary}\n\n${vscode.l10n.t('The Entry cell behaves as empty when execution visits it.')}`;
      case 'primary':
        return `**${cell.primary.name}** \`${cell.primary.token}\`\n\n${cell.primary.summary}${codeLine(cell.primary.code)}`;
    }
  }

  const prefixNote = PREFIX_NOTES[trimmed];
  if (prefixNote) return `**\`${trimmed}\`**\n\n${prefixNote}`;

  return null;
}
