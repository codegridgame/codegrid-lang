import * as vscode from 'vscode';
import { formatCodeGrid } from '../formatter/formatter';

/**
 * Whole-document formatting. Declines (returns no edits) when the formatter
 * cannot safely understand the source, leaving the document unchanged.
 */
export class CodeGridFormattingEditProvider implements vscode.DocumentFormattingEditProvider {
  provideDocumentFormattingEdits(document: vscode.TextDocument): vscode.TextEdit[] {
    const original = document.getText();
    const formatted = formatCodeGrid(original);
    if (formatted === null || formatted === original) return [];
    const fullRange = new vscode.Range(document.positionAt(0), document.positionAt(original.length));
    return [vscode.TextEdit.replace(fullRange, formatted)];
  }
}
