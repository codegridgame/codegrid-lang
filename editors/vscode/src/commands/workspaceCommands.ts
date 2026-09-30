import * as vscode from 'vscode';
import { TEMPLATES, findTemplate } from '../language/templates';

export const NEW_FILE_COMMAND = 'codegrid.newFile';
export const OPEN_README_COMMAND = 'codegrid.openReadme';

/**
 * Opens a new untitled CodeGrid document from a template. Without an argument
 * a template picker is shown; with a template id the document opens directly
 * (usable from tests and keybindings).
 */
export async function newCodeGridFile(templateId?: string): Promise<void> {
  let template = templateId ? findTemplate(templateId) : undefined;
  if (templateId && !template) {
    vscode.window.showWarningMessage(vscode.l10n.t('[editor.invalid_template] Unknown CodeGrid template: {0}', templateId));
    return;
  }
  if (!template) {
    const picked = await vscode.window.showQuickPick(
      TEMPLATES.map((entry) => ({ label: vscode.l10n.t(entry.label), description: vscode.l10n.t(entry.detail), entry })),
      { placeHolder: vscode.l10n.t('Select a CodeGrid template') }
    );
    if (!picked) return;
    template = picked.entry;
  }
  const document = await vscode.workspace.openTextDocument({
    language: 'codegrid',
    content: template.body,
  });
  await vscode.window.showTextDocument(document);
}

/** Opens the packaged README, which documents features and limitations. */
export async function openReadme(context: vscode.ExtensionContext): Promise<void> {
  const readme = vscode.Uri.joinPath(context.extensionUri, 'README.md');
  const document = await vscode.workspace.openTextDocument(readme);
  await vscode.window.showTextDocument(document);
}

export function registerWorkspaceCommands(context: vscode.ExtensionContext): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand(NEW_FILE_COMMAND, (templateId?: string) => newCodeGridFile(templateId)),
    vscode.commands.registerCommand(OPEN_README_COMMAND, () => openReadme(context)),
  ];
}
