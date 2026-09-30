import * as assert from 'assert';
import * as vscode from 'vscode';

const EXT_ID = 'codegrid.codegrid-vscode';

async function closeActiveEditor(): Promise<void> {
  await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
}

suite('Commands', () => {
  let extension: vscode.Extension<unknown>;

  suiteSetup(async () => {
    extension = vscode.extensions.getExtension(EXT_ID)!;
    await extension.activate();
  });

  suiteTeardown(async () => {
    await closeActiveEditor();
  });

  test('newFile opens the requested template as an untitled codegrid document', async () => {
    await vscode.commands.executeCommand('codegrid.newFile', 'main');
    const editor = vscode.window.activeTextEditor;
    assert.ok(editor, 'an editor must be open');
    assert.strictEqual(editor.document.languageId, 'codegrid');
    assert.ok(editor.document.isUntitled);
    assert.ok(editor.document.getText().startsWith('@main'));
    await closeActiveEditor();
  });

  test('newFile opens every built-in template id', async () => {
    for (const id of ['empty']) {
      await vscode.commands.executeCommand('codegrid.newFile', id);
      const editor = vscode.window.activeTextEditor;
      assert.ok(editor, `an editor must be open for ${id}`);
      assert.strictEqual(editor.document.languageId, 'codegrid', id);
      await closeActiveEditor();
    }
  });

  test('newFile with an unknown template id keeps the current editor', async () => {
    const doc = await vscode.workspace.openTextDocument({ language: 'codegrid', content: '~v _\n' });
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand('codegrid.newFile', 'nope');
    assert.strictEqual(
      vscode.window.activeTextEditor?.document.uri.toString(),
      doc.uri.toString()
    );
    await closeActiveEditor();
  });

  test('openReadme opens the packaged README', async () => {
    await vscode.commands.executeCommand('codegrid.openReadme');
    const doc = vscode.window.activeTextEditor?.document;
    assert.ok(doc, 'README must be open');
    assert.strictEqual(doc.languageId, 'markdown');
    assert.ok(doc.uri.fsPath.endsWith('README.md'));
    await closeActiveEditor();
  });
});
