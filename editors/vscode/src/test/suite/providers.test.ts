import * as assert from 'assert';
import * as vscode from 'vscode';
import { PRIMARIES } from '../../language/metadata';

const EXT_ID = 'codegrid.codegrid-vscode';

async function openDoc(content: string): Promise<vscode.TextDocument> {
  return vscode.workspace.openTextDocument({ language: 'codegrid', content });
}

async function complete(doc: vscode.TextDocument, line: number, character: number): Promise<string[]> {
  const list = await vscode.commands.executeCommand<vscode.CompletionList>(
    'vscode.executeCompletionItemProvider',
    doc.uri,
    new vscode.Position(line, character)
  );
  return list.items.map((item) => (typeof item.label === 'string' ? item.label : item.label.label));
}

suite('Full standalone providers', () => {
  suiteSetup(async () => {
    const extension = vscode.extensions.getExtension(EXT_ID)!;
    await extension.activate();
    await vscode.workspace.getConfiguration('editor').update('wordBasedSuggestions', 'off', vscode.ConfigurationTarget.Global);
  });

  test('completion offers the Full Primary, Entry, and Empty inventories', async () => {
    const document = await openDoc('~> ');
    const labels = await complete(document, 0, 3);
    const primaries = PRIMARIES.map((primary) => primary.token);
    for (const token of [...primaries, '~^', '~v', '~<', '~>', '_']) {
      assert.ok(labels.includes(token), `missing ${token}`);
    }
    for (const token of ['[0', '[9', '$0', '$9', '#0', '#9', '#]']) {
      assert.ok(labels.includes(token), `missing structural instruction ${token}`);
    }
  });

  test('completion offers only the applicable static attachment spellings', async () => {
    const add = await complete(await openDoc('~> +x'), 0, 5);
    assert.deepStrictEqual(add.sort(), ['+x2', '+x3', '+x4', '+x5'].sort());

    const call = await complete(await openDoc('~> [0'), 0, 5);
    assert.ok(call.includes('[0*'));
    assert.ok(call.includes('[0='));
    assert.ok(!call.includes('[0x2'), 'Repeat is not offered on CALL');

    const ret = await complete(await openDoc('~> ]'), 0, 4);
    assert.ok(ret.includes(']*'));
    assert.ok(ret.includes(']='));
    assert.ok(!ret.includes(']x2'), 'Repeat is not offered on RETURN');

    const unencodable = await complete(await openDoc('~> $0'), 0, 5);
    assert.ok(!unencodable.some((label) => label.startsWith('$0*') || label.startsWith('$0=') || label.startsWith('$0x')));
  });

  test('prefix completion returns matching Full instruction tokens', async () => {
    const hash = await complete(await openDoc('~> #'), 0, 4);
    for (const token of ['#^', '#v', '#<', '#>', '#0', '#9', '#]']) {
      assert.ok(hash.includes(token), `missing ${token}`);
    }
    const shift = await complete(await openDoc('~> $'), 0, 4);
    for (const token of ['$&', '$(', '$)', '$+', '$-', '$<', '$>', '$0', '$9']) {
      assert.ok(shift.includes(token), `missing ${token}`);
    }
    const read = await complete(await openDoc('~> ,'), 0, 4);
    for (const token of [',<', ',>', ',^', ',v', ',<*', ',<x2']) {
      assert.ok(read.includes(token), `missing ${token}`);
    }
  });

  test('directive completion offers Full roots and qualified structural paths', async () => {
    const root = await complete(await openDoc('@'), 0, 1);
    for (const token of ['@main', '@size', '@end', '@C0', '@C9', '@F0', '@F9', '@M0', '@M9']) {
      assert.ok(root.includes(token), `missing ${token}`);
    }
    const customChild = await complete(await openDoc('@C2.'), 0, 4);
    for (const token of ['@C2.F0', '@C2.F9', '@C2.M0', '@C2.M9']) {
      assert.ok(customChild.includes(token), `missing ${token}`);
    }
    assert.ok(!customChild.includes('@C2.F0.M0'), 'nested paths are suggested after their owner path is typed');

    const functionFolded = await complete(await openDoc('@C2.F1.'), 0, 7);
    for (const token of ['@C2.F1.M0', '@C2.F1.M9']) {
      assert.ok(functionFolded.includes(token), `missing ${token}`);
    }
    const relativeFolded = await complete(await openDoc('@F2.'), 0, 4);
    assert.ok(relativeFolded.includes('@F2.M3'));
  });

  test('named @end completion offers aliases and qualified closing-name shapes', async () => {
    const names = await complete(await openDoc('@end '), 0, 5);
    for (const token of ['main', 'C0', 'C9', 'F0', 'F9', 'M0', 'M9']) {
      assert.ok(names.includes(token), `missing ${token}`);
    }
    const mainChildren = await complete(await openDoc('@end main.'), 0, 10);
    for (const token of ['main.F0', 'main.F9', 'main.M0', 'main.M9']) {
      assert.ok(mainChildren.includes(token), `missing ${token}`);
    }
    assert.ok(!mainChildren.includes('main.F0.M0'));
    const nestedClose = await complete(await openDoc('@end C2.F1.'), 0, 11);
    for (const token of ['C2.F1.M0', 'C2.F1.M9']) {
      assert.ok(nestedClose.includes(token), `missing ${token}`);
    }
  });

  test('completion includes the main-board and minimal-board snippets', async () => {
    const document = await openDoc('@');
    const list = await vscode.commands.executeCommand<vscode.CompletionList>(
      'vscode.executeCompletionItemProvider', document.uri, new vscode.Position(0, 1)
    );
    const snippets = list.items.filter((item) => item.kind === vscode.CompletionItemKind.Snippet);
    assert.deepStrictEqual(
      snippets.map((item) => typeof item.label === 'string' ? item.label : item.label.label).sort(),
      ['main-board', 'minimal-board'].sort()
    );
  });

  test('hover explains known instructions and incomplete prefixes', async () => {
    const document = await openDoc('~> ,> #^ $> + ; #');
    const hoverAt = async (character: number): Promise<string> => {
      const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
        'vscode.executeHoverProvider', document.uri, new vscode.Position(0, character)
      );
      return hovers.flatMap((hover) => hover.contents).map((content) =>
        content instanceof vscode.MarkdownString ? content.value : String(content)
      ).join('\n');
    };
    assert.ok((await hoverAt(4)).includes('input is exhausted'));
    assert.ok((await hoverAt(7)).includes('R0 is zero'));
    assert.ok((await hoverAt(10)).includes('logically'));
    assert.ok((await hoverAt(12)).includes('8-bit wrapping'));
    assert.ok((await hoverAt(14)).includes('does not move'));
    assert.ok((await hoverAt(16)).includes('must be completed'));
  });

  test('hover describes Main directives and Entry markers', async () => {
    const document = await openDoc('@main\n~v ;');
    const directive = await vscode.commands.executeCommand<vscode.Hover[]>(
      'vscode.executeHoverProvider', document.uri, new vscode.Position(0, 2)
    );
    assert.ok(directive.flatMap((hover) => hover.contents).some((content) =>
      content instanceof vscode.MarkdownString && content.value.includes('single Main Board')
    ));
    const entry = await vscode.commands.executeCommand<vscode.Hover[]>(
      'vscode.executeHoverProvider', document.uri, new vscode.Position(1, 1)
    );
    assert.ok(entry.flatMap((hover) => hover.contents).some((content) =>
      content instanceof vscode.MarkdownString && content.value.includes('Entry marker')
    ));
  });
});
