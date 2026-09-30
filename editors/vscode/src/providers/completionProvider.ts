import * as vscode from 'vscode';
import { EMPTY_CELL, ENTRIES, PRIMARY_CELL_SPELLINGS } from '../language/metadata';

interface TokenSpan {
  start: number;
  end: number;
  text: string;
}

function tokenSpanAt(line: string, character: number): TokenSpan {
  let start = character;
  while (start > 0 && !/\s/.test(line[start - 1])) start--;
  let end = character;
  while (end < line.length && !/\s/.test(line[end])) end++;
  return { start, end, text: line.slice(start, end) };
}

function cellItem(label: string, detail: string, documentation: string): CompletionItemData {
  return { label, detail, documentation, kind: vscode.CompletionItemKind.Keyword };
}

interface CompletionItemData {
  label: string;
  detail: string;
  documentation?: string;
  kind: vscode.CompletionItemKind;
}

const CELL_ITEMS: CompletionItemData[] = [
  ...PRIMARY_CELL_SPELLINGS.map(({ token, primary, attachment }) => {
    const attachmentDetail = attachment === '*'
      ? vscode.l10n.t(' with ReadCode')
      : attachment === '='
        ? vscode.l10n.t(' with WriteCode')
        : attachment
          ? vscode.l10n.t(' repeated {0} times', attachment.slice(1))
          : '';
    const documentation = attachment === '*'
      ? vscode.l10n.t('ReadCode attachment.')
      : attachment === '='
        ? vscode.l10n.t('WriteCode attachment.')
        : attachment
          ? vscode.l10n.t('Repeat attachment.')
          : primary.summary;
    return cellItem(token, `${primary.name}${attachmentDetail}`, documentation);
  }),
  ...ENTRIES.map((entry) => ({
    label: entry.token,
    detail: vscode.l10n.t('Entry marker ({0})', vscode.l10n.t(entry.direction)),
    documentation: entry.summary,
    kind: vscode.CompletionItemKind.Constant,
  })),
  {
    label: EMPTY_CELL.token,
    detail: 'Empty cell',
    documentation: EMPTY_CELL.summary,
    kind: vscode.CompletionItemKind.Keyword,
  },
];

interface DirectiveItem {
  label: string;
  detail: string;
  documentation?: string;
}

function numberedItems(prefix: string, detail: string): DirectiveItem[] {
  return Array.from({ length: 10 }, (_, id) => ({
    label: `${prefix}${id}`,
    detail: `${vscode.l10n.t(detail)} ${id}`,
  }));
}

const DIRECTIVE_ROOTS: DirectiveItem[] = [
  { label: '@main', detail: 'Define the outer Main CodeGrid and Main Board' },
  { label: '@size', detail: 'Declare positive WIDTHxHEIGHT dimensions' },
  { label: '@end', detail: 'Close the current explicitly delimited definition' },
  ...numberedItems('@C', 'Define Custom CodeGrid'),
  ...numberedItems('@F', 'Define a Function in the current CodeGrid'),
  ...numberedItems('@M', 'Define a Folded Block in the current Board'),
];

function structuralPathItems(prefix: string): DirectiveItem[] {
  if (!prefix.includes('.')) {
    return DIRECTIVE_ROOTS.filter((item) => item.label.toLowerCase().startsWith(prefix.toLowerCase()));
  }

  const parts = prefix.slice(1).split('.');
  const depth = parts.length;
  const allPaths: DirectiveItem[] = [];
  for (const board of ['main', ...Array.from({ length: 10 }, (_, id) => `C${id}`)]) {
    for (const kind of ['F', 'M']) {
      for (let id = 0; id < 10; id++) {
        const first = `${board}.${kind}${id}`;
        allPaths.push({
          label: `@${first}`,
          detail: kind === 'F' ? 'Define a qualified Function path' : 'Define a qualified Folded Block path',
        });
        if (kind === 'F') {
          for (let folded = 0; folded < 10; folded++) {
            allPaths.push({ label: `@${first}.M${folded}`, detail: 'Define a Function-owned Folded Block path' });
          }
        }
      }
    }
  }
  for (let fn = 0; fn < 10; fn++) {
    for (let folded = 0; folded < 10; folded++) {
      allPaths.push({ label: `@F${fn}.M${folded}`, detail: 'Define a Function-owned Folded Block path' });
    }
  }

  return allPaths.filter((item) =>
    item.label.split('.').length - 1 === depth - 1 &&
    item.label.toLowerCase().startsWith(prefix.toLowerCase())
  );
}

function closeNameRoots(): DirectiveItem[] {
  return [
    { label: 'main', detail: 'Close Main' },
    ...numberedItems('C', 'Close Custom CodeGrid'),
    ...numberedItems('F', 'Close local Function or a qualified Function'),
    ...numberedItems('M', 'Close local Folded Block or a qualified Folded Block'),
  ];
}

function closeNamePaths(prefix: string): DirectiveItem[] {
  if (!prefix.includes('.')) {
    return closeNameRoots().filter((item) => item.label.toLowerCase().startsWith(prefix.toLowerCase()));
  }

  const paths: DirectiveItem[] = [];
  for (const board of ['main', ...Array.from({ length: 10 }, (_, id) => `C${id}`)]) {
    for (let id = 0; id < 10; id++) {
      paths.push({ label: `${board}.F${id}`, detail: 'Qualified Function closing name' });
      paths.push({ label: `${board}.M${id}`, detail: 'Qualified Folded Block closing name' });
      for (let folded = 0; folded < 10; folded++) {
        paths.push({ label: `${board}.F${id}.M${folded}`, detail: 'Qualified Folded Block closing name' });
      }
    }
  }
  for (let fn = 0; fn < 10; fn++) {
    for (let folded = 0; folded < 10; folded++) {
      paths.push({ label: `F${fn}.M${folded}`, detail: 'Relative Folded Block closing name' });
    }
  }
  const depth = prefix.split('.').length;
  return paths.filter((item) =>
    item.label.split('.').length === depth && item.label.toLowerCase().startsWith(prefix.toLowerCase())
  );
}

const SNIPPETS = [
  {
    label: 'main-board',
    detail: 'CodeGrid: one Main Board with an Entry',
    body: '@main\n@size 5x5\n~${1|v,^,<,>|} _ _ _ _\n_  + > . _\n_  _ _ _ _\n_  _ _ _ _\n_  _ _ _ _\n@end main',
  },
  {
    label: 'minimal-board',
    detail: 'CodeGrid: one-row board with HALT',
    body: '~${1|>,v,^,<|} ;',
  },
];

function completion(
  item: CompletionItemData | DirectiveItem,
  kind: vscode.CompletionItemKind,
  range: vscode.Range
): vscode.CompletionItem {
  const result = new vscode.CompletionItem(item.label, kind);
  result.detail = vscode.l10n.t(item.detail);
  if ('documentation' in item && item.documentation) {
    result.documentation = new vscode.MarkdownString(item.documentation);
  }
  result.range = range;
  return result;
}

export class CodeGridCompletionProvider implements vscode.CompletionItemProvider {
  provideCompletionItems(
    document: vscode.TextDocument,
    position: vscode.Position
  ): vscode.CompletionList {
    const line = document.lineAt(position.line).text;
    const span = tokenSpanAt(line, position.character);
    const range = new vscode.Range(position.line, span.start, position.line, span.end);
    const partial = span.text;
    const beforeToken = line.slice(0, span.start);

    if (/^\s*@end(?:[ \t]+[^\s]*)?[ \t]*$/i.test(beforeToken)) {
      const names = closeNamePaths(partial);
      return new vscode.CompletionList(
        names.map((item) => completion(item, vscode.CompletionItemKind.Reference, range)),
        false
      );
    }

    if (/^\s*@size(?:[ \t]+[^\s]*)?[ \t]*$/i.test(beforeToken)) {
      const size = { label: '5x5', detail: 'Example positive board dimensions' };
      const items = size.label.startsWith(partial)
        ? [completion(size, vscode.CompletionItemKind.Value, range)]
        : [];
      return new vscode.CompletionList(items, false);
    }

    if (partial.startsWith('@')) {
      const directives = structuralPathItems(partial).map((item) =>
        completion(item, vscode.CompletionItemKind.Class, range)
      );
      if (partial.toLowerCase() === '@' || partial.toLowerCase() === '@m') {
        for (const snippet of SNIPPETS) {
          const item = new vscode.CompletionItem(snippet.label, vscode.CompletionItemKind.Snippet);
          item.detail = vscode.l10n.t(snippet.detail);
          item.insertText = new vscode.SnippetString(snippet.body);
          item.filterText = `@${snippet.label}`;
          item.range = range;
          directives.push(item);
        }
      }
      return new vscode.CompletionList(directives, false);
    }

    const items = CELL_ITEMS
      .filter((item) => item.label.startsWith(partial))
      .map((item) => completion(item, item.kind, range));
    return new vscode.CompletionList(items, false);
  }
}
