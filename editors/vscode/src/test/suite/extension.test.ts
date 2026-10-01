import * as assert from 'assert';
import { spawnSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

const EXT_ID = 'codegrid.codegrid-vscode';

async function openDoc(content: string): Promise<vscode.TextDocument> {
  return vscode.workspace.openTextDocument({ language: 'codegrid', content });
}

async function completionLabels(document: vscode.TextDocument, position: vscode.Position): Promise<string[]> {
  const list = await vscode.commands.executeCommand<vscode.CompletionList>(
    'vscode.executeCompletionItemProvider', document.uri, position
  );
  return list?.items.map((item) => typeof item.label === 'string' ? item.label : item.label.label) ?? [];
}

async function waitForCompletions(
  document: vscode.TextDocument,
  position: vscode.Position,
  predicate: (labels: string[]) => boolean
): Promise<string[]> {
  const deadline = Date.now() + 8000;
  let labels: string[] = [];
  while (Date.now() < deadline) {
    labels = await completionLabels(document, position);
    if (predicate(labels)) return labels;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return labels;
}

async function waitForDiagnostics(
  document: vscode.TextDocument,
  predicate: (diagnostics: readonly vscode.Diagnostic[]) => boolean
): Promise<readonly vscode.Diagnostic[]> {
  const deadline = Date.now() + 10000;
  let diagnostics: readonly vscode.Diagnostic[] = [];
  while (Date.now() < deadline) {
    diagnostics = vscode.languages.getDiagnostics(document.uri);
    if (predicate(diagnostics)) return diagnostics;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return diagnostics;
}

async function waitForDocumentSymbols(
  document: vscode.TextDocument,
  predicate: (symbols: readonly vscode.DocumentSymbol[]) => boolean
): Promise<readonly vscode.DocumentSymbol[] | undefined> {
  const deadline = Date.now() + 8000;
  let symbols: readonly vscode.DocumentSymbol[] | undefined;
  while (Date.now() < deadline) {
    symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
      'vscode.executeDocumentSymbolProvider', document.uri
    );
    if (symbols !== undefined && predicate(symbols)) return symbols;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return symbols;
}

suite('VS Code extension and optional LSP', () => {
  test('activates and registers CodeGrid for .cg files', async () => {
    const extension = vscode.extensions.getExtension(EXT_ID);
    assert.ok(extension, 'extension must be resolvable');
    await extension!.activate();
    assert.ok(extension!.isActive);
    assert.ok((await vscode.languages.getLanguages()).includes('codegrid'));
    const smoke = path.join(extension!.extensionPath, 'src', 'test', 'fixtures', 'smoke.cg');
    assert.strictEqual((await vscode.workspace.openTextDocument(smoke)).languageId, 'codegrid');
  });

  test('uses the optional LSP without duplicate standalone completions', async () => {
    const config = vscode.workspace.getConfiguration('codegrid.languageServer');
    const target = vscode.ConfigurationTarget.Global;
    const previous = {
      enabled: config.inspect<boolean>('enabled')?.globalValue,
      path: config.inspect<string>('path')?.globalValue,
      arguments: config.inspect<string[]>('arguments')?.globalValue,
    };
    try {
      await config.update('path', process.execPath, target);
      await config.update('arguments', [path.resolve(__dirname, '../fixtures/fakeLspServer.js')], target);
      await config.update('enabled', true, target);
      const document = await openDoc('#');
      await vscode.window.showTextDocument(document);
      const labels = await waitForCompletions(document, new vscode.Position(0, 1), (items) => items.includes('#lsp-only'));
      assert.ok(labels.includes('#lsp-only'));
      assert.ok(!labels.includes('#^'), 'standalone completions must pause while LSP is running');
    } finally {
      await config.update('enabled', false, target);
      await config.update('enabled', previous.enabled, target);
      await config.update('path', previous.path, target);
      await config.update('arguments', previous.arguments, target);
    }
  });

  test('real Rust LSP serves Full completion, hover, source symbols, and diagnostics parity', async function () {
    const serverPath = process.env.CODEGRID_LSP_TEST_SERVER;
    if (!serverPath) this.skip();
    assert.ok(fs.existsSync(serverPath!), `Rust LSP executable not found: ${serverPath}`);

    const extension = vscode.extensions.getExtension(EXT_ID)!;
    await extension.activate();
    const config = vscode.workspace.getConfiguration('codegrid.languageServer');
    const target = vscode.ConfigurationTarget.Global;
    const previous = {
      enabled: config.inspect<boolean>('enabled')?.globalValue,
      path: config.inspect<string>('path')?.globalValue,
      arguments: config.inspect<string[]>('arguments')?.globalValue,
    };
    try {
      await config.update('path', serverPath, target);
      await config.update('arguments', [], target);
      await config.update('enabled', true, target);

      const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
      assert.ok(workspaceRoot, 'the extension test workspace must be available');
      const fixtureDirectory = path.join(workspaceRoot!, 'src', 'test', 'fixtures');
      const fixtureStem = path.join(fixtureDirectory, `real-lsp-${process.pid}-${Date.now()}`);
      const symbolPath = `${fixtureStem}-symbols.cg`;
      const completionPath = `${fixtureStem}-completion.cg`;
      const sourcePath = `${fixtureStem}-diagnostics.cg`;
      try {
        fs.writeFileSync(symbolPath, '@main\n@size 4x1\n~> + _ _\n@end\n', 'utf8');
        const symbolDocument = await vscode.workspace.openTextDocument(symbolPath);
        await vscode.window.showTextDocument(symbolDocument);
        const symbols = await waitForDocumentSymbols(symbolDocument, (items) =>
          items.some((symbol) => symbol.name === 'Main Board')
        );
        assert.deepStrictEqual(symbols?.map((symbol) => symbol.name), ['Main Board']);

        const source = '@main\n@size 4x1\n~> + #\n@end\n';
        fs.writeFileSync(completionPath, source, 'utf8');
        const document = await vscode.workspace.openTextDocument(completionPath);
        await vscode.window.showTextDocument(document);
        const line = source.split('\n')[2];
        const position = new vscode.Position(2, line.length);
        const labels = await waitForCompletions(document, position, (items) =>
          items.includes('#^') && items.includes('#0') && items.includes('#]')
        );
        assert.ok(labels.includes('#^'));
        assert.ok(labels.includes('#0') && labels.includes('#]'));

        const hover = await vscode.commands.executeCommand<vscode.Hover[]>(
          'vscode.executeHoverProvider', document.uri, new vscode.Position(2, 3)
        );
        assert.ok(hover.flatMap((item) => item.contents).some((content) =>
          content instanceof vscode.MarkdownString
            && content.value.includes('Increment the selected register with 8-bit wrapping')
        ));

        const cliPath = process.env.CODEGRID_CLI_TEST_BIN || path.join(
          path.dirname(serverPath!), process.platform === 'win32' ? 'codegrid.exe' : 'codegrid'
        );
        assert.ok(fs.existsSync(cliPath), `CLI executable required for parity check: ${cliPath}`);
        const invalidSource = '@main\n~> _x\n@end\n';
        fs.writeFileSync(sourcePath, invalidSource, 'utf8');
        const cli = spawnSync(cliPath, ['check', sourcePath], { encoding: 'utf8' });
        assert.strictEqual(cli.status, 4, cli.stderr);
        const cliDiagnostics = cli.stderr.trim().split(/\r?\n/).map((lineText) => {
          const match = lineText.match(/:(\d+):(\d+): (error|warning): \[([0-9]{4})\] \[([^\]]+)\] (.*)$/);
          assert.ok(match, `CLI diagnostic must use its source format: ${lineText}`);
          return {
            line: Number(match![1]),
            column: Number(match![2]),
            severity: match![3],
            code: match![5],
            message: `[${match![4]}] [${match![5]}] ${match![6]}`,
          };
        });
        const invalidDocument = await vscode.workspace.openTextDocument(sourcePath);
        await vscode.window.showTextDocument(invalidDocument);
        const lspDiagnostics = await waitForDiagnostics(invalidDocument, (items) => items.length > 0);
        const normalizedLsp = lspDiagnostics.map((diagnostic) => ({
          line: diagnostic.range.start.line + 1,
          column: diagnostic.range.start.character + 1,
          severity: diagnostic.severity === vscode.DiagnosticSeverity.Warning ? 'warning' : 'error',
          code: diagnostic.code,
          message: diagnostic.message,
        }));
        assert.deepStrictEqual(normalizedLsp, cliDiagnostics);
      } finally {
        await vscode.commands.executeCommand('workbench.action.closeAllEditors');
        for (const fixturePath of [symbolPath, completionPath, sourcePath]) {
          fs.rmSync(fixturePath, { force: true });
        }
      }
    } finally {
      await config.update('enabled', false, target);
      await config.update('enabled', previous.enabled, target);
      await config.update('path', previous.path, target);
      await config.update('arguments', previous.arguments, target);
    }
  });

  test('restores standalone providers when the optional LSP exits or cannot start', async () => {
    const config = vscode.workspace.getConfiguration('codegrid.languageServer');
    const target = vscode.ConfigurationTarget.Global;
    const previous = {
      enabled: config.inspect<boolean>('enabled')?.globalValue,
      path: config.inspect<string>('path')?.globalValue,
      arguments: config.inspect<string[]>('arguments')?.globalValue,
    };
    try {
      await config.update('path', process.execPath, target);
      await config.update('arguments', [path.resolve(__dirname, '../fixtures/fakeLspServer.js'), '--exit-after-completion'], target);
      await config.update('enabled', true, target);
      const document = await openDoc('#');
      await vscode.window.showTextDocument(document);
      const serverLabels = await waitForCompletions(document, new vscode.Position(0, 1), (items) => items.includes('#lsp-only'));
      assert.ok(serverLabels.includes('#lsp-only'));
      const fallbackLabels = await waitForCompletions(document, new vscode.Position(0, 1), (items) => items.includes('#^'));
      assert.ok(fallbackLabels.includes('#^'));
      assert.ok(fallbackLabels.includes('#0'));

      await config.update('path', path.join(__dirname, 'missing-codegrid-lsp.exe'), target);
      await config.update('arguments', [], target);
      const unavailableDocument = await openDoc('$');
      await vscode.window.showTextDocument(unavailableDocument);
      const unavailableLabels = await waitForCompletions(unavailableDocument, new vscode.Position(0, 1), (items) => items.includes('$<'));
      assert.ok(unavailableLabels.includes('$<'), 'standalone completion must survive an unavailable executable');
    } finally {
      await config.update('enabled', false, target);
      await config.update('enabled', previous.enabled, target);
      await config.update('path', previous.path, target);
      await config.update('arguments', previous.arguments, target);
    }
  });
});
