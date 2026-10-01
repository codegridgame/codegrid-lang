import * as vscode from 'vscode';
import { errorNumbers } from './errorNumbers';
import { errorMessage } from './errorCatalog';

/** Numeric presentation only; source acceptance and VM behavior remain in Rust. */
export function errorNumber(code: string): string {
  for (const layer of ['source', 'ir', 'vm', 'fault', 'debug', 'editor', 'level']) {
    const number = errorNumbers[`${layer}:${code}`];
    if (number) return number;
  }
  return errorNumbers['editor:editor.operation_failed'];
}

export function errorLabel(code: string): string {
  return `[${errorNumber(code)}] [${code}]`;
}

export function localizeError(code: string, originalMessage?: string): string {
  if (vscode.env.language.toLowerCase().startsWith('en') && originalMessage) return originalMessage;
  const number = ['source', 'ir', 'vm', 'fault', 'debug', 'editor', 'cli', 'level']
    .map(layer => errorNumbers[`${layer}:${code}`]).find(Boolean);
  return errorMessage(number || '', vscode.env.language);
}

export function localizeDiagnostic(uri: vscode.Uri, diagnostic: vscode.Diagnostic): vscode.Diagnostic {
  const identity = typeof diagnostic.code === 'object' ? diagnostic.code.value : diagnostic.code;
  const code = typeof identity === 'string' || typeof identity === 'number' ? String(identity) : 'editor.operation_failed';
  const localized = Object.assign(new vscode.Diagnostic(diagnostic.range, diagnostic.message, diagnostic.severity), diagnostic);
  localized.message = `${errorLabel(code)} ${localizeError(code, diagnostic.message)}`;
  localized.relatedInformation = [
    ...(diagnostic.relatedInformation || []),
    new vscode.DiagnosticRelatedInformation(new vscode.Location(uri, diagnostic.range),
      vscode.l10n.t('Original diagnostic: {0}', diagnostic.message)),
  ];
  return localized;
}

export interface PresentedRuntimeError {
  code: string;
  message: string;
  details: unknown;
}

/** Project structured VM errors without changing their order or raw state. */
export function presentRuntimeErrors(snapshot: Record<string, any>): PresentedRuntimeError[] {
  const errors = Array.isArray(snapshot.errors) ? snapshot.errors : [];
  const issues = errors.map((error: Record<string, any>) => {
    const code = typeof error.code === 'string' ? error.code : 'editor.invalid_debug_response';
    return { code, message: localizeError(code), details: error };
  });
  if (snapshot.fault) {
    const code = typeof snapshot.fault.kind === 'string' ? snapshot.fault.kind : 'editor.invalid_debug_response';
    issues.push({ code, message: localizeError(code), details: snapshot.fault });
  }
  if (!issues.length) {
    const code = 'editor.invalid_debug_response';
    issues.push({ code, message: localizeError(code), details: snapshot });
  }
  return issues;
}
