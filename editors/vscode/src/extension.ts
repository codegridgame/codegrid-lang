import * as vscode from 'vscode';
import { LanguageClient, TransportKind } from 'vscode-languageclient/node';
import { State } from 'vscode-languageclient/lib/common/client';
import { registerExecution } from './debug/adapter';
import { localizeDiagnostic } from './language/errorMessages';
import { CodeGridCompletionProvider } from './providers/completionProvider';
import { CodeGridHoverProvider } from './providers/hoverProvider';
import { CodeGridFormattingEditProvider } from './providers/formatProvider';
import {
  OPEN_README_COMMAND,
  registerWorkspaceCommands,
} from './commands/workspaceCommands';

let languageClient: LanguageClient | undefined;
let standaloneProviders: vscode.Disposable[] = [];
let configurationQueue: Promise<void> = Promise.resolve();
let statusItem: vscode.LanguageStatusItem | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  registerStandaloneProviders();
  context.subscriptions.push(
    ...registerExecution(context),
    ...registerWorkspaceCommands(context),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('codegrid.languageServer')) {
        void queueLanguageServerConfiguration();
      }
    })
  );
  createStatusItem(context);
  await queueLanguageServerConfiguration();
}

function createStatusItem(context: vscode.ExtensionContext): void {
  const item = vscode.languages.createLanguageStatusItem('codegrid.status', { language: 'codegrid' });
  statusItem = item;
  item.name = 'CodeGrid';
  item.severity = vscode.LanguageStatusSeverity.Information;
  item.command = {
    command: OPEN_README_COMMAND,
    title: vscode.l10n.t('Learn about CodeGrid 0.1')
  };
  updateStatusItem(false);
  context.subscriptions.push(item);
}

function updateStatusItem(languageServerActive: boolean): void {
  if (!statusItem) return;
  statusItem.text = languageServerActive ? vscode.l10n.t('CodeGrid: language server') : vscode.l10n.t('CodeGrid: standalone');
  statusItem.detail = languageServerActive
    ? vscode.l10n.t('Compiler-backed diagnostics and Full source assistance are active.')
    : vscode.l10n.t('Static assistance only; an optional language server can add diagnostics.');
}

function registerStandaloneProviders(): void {
  if (standaloneProviders.length > 0) return;
  const selector: vscode.DocumentSelector = { language: 'codegrid' };
  standaloneProviders = [
    vscode.languages.registerDocumentFormattingEditProvider(selector, new CodeGridFormattingEditProvider()),
    vscode.languages.registerCompletionItemProvider(
      selector,
      new CodeGridCompletionProvider(),
      '@',
      '#',
      '$',
      ',',
      '~',
    ),
    vscode.languages.registerHoverProvider(selector, new CodeGridHoverProvider())
  ];
}

function disposeStandaloneProviders(): void {
  for (const provider of standaloneProviders) provider.dispose();
  standaloneProviders = [];
}

function queueLanguageServerConfiguration(): Promise<void> {
  configurationQueue = configurationQueue
    .catch(() => undefined)
    .then(() => configureLanguageServer());
  return configurationQueue;
}

async function configureLanguageServer(): Promise<void> {
  const previousClient = languageClient;
  languageClient = undefined;
  if (previousClient) {
    try {
      await previousClient.stop();
    } catch {
      // Keep the standalone providers available if stopping the process fails.
    }
  }
  registerStandaloneProviders();
  updateStatusItem(false);

  const settings = vscode.workspace.getConfiguration('codegrid.languageServer');
  if (!settings.get<boolean>('enabled', false)) return;

  const command = settings.get<string>('path', 'codegrid-lsp').trim();
  const args = settings.get<string[]>('arguments', []);
  if (!command) {
    vscode.window.showWarningMessage(
      vscode.l10n.t('[editor.lsp_path_empty] CodeGrid language server path is empty. Standalone editor features remain available.')
    );
    return;
  }

  const serverOptions = {
    run: { command, args, transport: TransportKind.stdio },
    debug: { command, args, transport: TransportKind.stdio }
  };
  const client = new LanguageClient('codegrid', vscode.l10n.t('CodeGrid Language Server'), serverOptions, {
    documentSelector: [
      { scheme: 'file', language: 'codegrid' },
      { scheme: 'untitled', language: 'codegrid' }
    ],
    diagnosticCollectionName: 'CodeGrid',
    middleware: {
      handleDiagnostics: (uri, diagnostics, next) => next(uri, diagnostics.map((diagnostic) => localizeDiagnostic(uri, diagnostic))),
    },
  });
  languageClient = client;

  client.onDidChangeState((event) => {
    if (languageClient !== client) return;
    if (event.newState === State.Running) {
      disposeStandaloneProviders();
      updateStatusItem(true);
    } else if (event.newState === State.Stopped) {
      registerStandaloneProviders();
      updateStatusItem(false);
    }
  });

  try {
    await client.start();
    if (languageClient === client) {
      disposeStandaloneProviders();
      updateStatusItem(true);
    }
  } catch (error) {
    if (languageClient === client) languageClient = undefined;
    try {
      await client.stop();
    } catch {
      // The failed client may not have created a process to stop.
    }
    registerStandaloneProviders();
    const message = error instanceof Error ? error.message : String(error);
    vscode.window.showWarningMessage(
      vscode.l10n.t('[editor.lsp_start_failed] CodeGrid language server could not be started ({0}). Standalone editor features remain available.', message)
    );
  }
}

export async function deactivate(): Promise<void> {
  await configurationQueue.catch(() => undefined);
  const client = languageClient;
  languageClient = undefined;
  if (client) await client.stop().catch(() => undefined);
  disposeStandaloneProviders();
}
