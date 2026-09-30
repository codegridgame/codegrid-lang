import * as vscode from 'vscode';
import { describeToken } from '../language/describe';

/** Hover descriptions from the editor metadata table (localized text). */
export class CodeGridHoverProvider implements vscode.HoverProvider {
  provideHover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | null {
    const range = document.getWordRangeAtPosition(position, /[^\s]+/);
    if (!range) return null;
    const token = document.getText(range);
    const markdown = describeToken(token);
    if (!markdown) return null;
    return new vscode.Hover(new vscode.MarkdownString(markdown), range);
  }
}
