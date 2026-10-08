import * as vscode from 'vscode';
import { errorLabel, localizeError, presentRuntimeErrors } from '../language/errorMessages';
import * as path from 'path';
import * as fs from 'fs';
import { NativeRuntime, SourceLocation, RuntimeReply } from './nativeRuntime';
import { CodedError, codedError } from './errors';

interface Request { seq: number; command: string; arguments?: any }
interface Breakpoint { id: number; line: number; column?: number; location?: SourceLocation }

export function runtimePath(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration('codegrid').get<string>('runtime.path', '').trim();
  if (configured) return configured;
  const bundled = path.join(context.extensionPath, 'runtime', process.platform === 'win32' ? 'codegrid.exe' : 'codegrid');
  if (fs.existsSync(bundled)) return bundled;
  return process.platform === 'win32' ? 'codegrid.exe' : 'codegrid';
}

/** A DAP presentation adapter over the native VM's atomic global ticks. */
export class CodeGridDebugAdapter implements vscode.DebugAdapter {
  private readonly emitter = new vscode.EventEmitter<any>();
  readonly onDidSendMessage = this.emitter.event;
  private sequence = 1;
  private runtime: NativeRuntime | undefined;
  private snapshot: Record<string, any> = {};
  private locations: SourceLocation[] = [];
  private sourcePath = '';
  private sourceText = '';
  private breakpoints: Breakpoint[] = [];
  private nextBreakpoint = 1;
  private running = false;
  private pauseRequested = false;
  private closed = false;
  private skipCurrentBreakpoint = false;
  private configured = false;
  private launchOptions: any;
  private readonly variables = new Map<number, any>();
  private nextVariable = 1;
  private readonly frameThreads = new Map<number, any>();
  private readonly threadIds = new Map<string, number>();
  private nextFrame = 1;
  private lastEvents: unknown[] = [];

  constructor(private readonly context: vscode.ExtensionContext) {}

  handleMessage(message: any): void {
    if (message.type === 'request') void this.dispatch(message as Request);
  }

  dispose(): void {
    this.closed = true;
    this.running = false;
    this.runtime?.dispose();
    this.emitter.dispose();
  }

  private send(message: any): void { if (!this.closed) this.emitter.fire({ seq: this.sequence++, ...message }); }
  private event(event: string, body: any = {}): void { this.send({ type: 'event', event, body }); }
  private reply(request: Request, body: any = {}, error?: CodedError): void {
    this.send({ type: 'response', request_seq: request.seq, command: request.command, success: !error, body: error ? { error: { id: Number(error.errorNumber), format: '[{error_number}] [{code}] {message}', variables: { error_number: error.errorNumber, code: error.code, message: error.message }, showUser: true } } : body, ...(error ? { message: error.code } : {}) });
  }
  private output(output: string, category = 'console'): void { this.event('output', { output, category }); }

  private async dispatch(request: Request): Promise<void> {
    const args = request.arguments || {};
    try {
      switch (request.command) {
        case 'initialize':
          this.reply(request, { supportsConfigurationDoneRequest: true, supportsTerminateRequest: true, supportsEvaluateForHovers: true, supportsBreakpointLocationsRequest: true });
          this.event('initialized'); return;
        case 'launch': await this.launch(args); this.reply(request); this.beginIfReady(); return;
        case 'configurationDone': this.configured = true; this.reply(request); this.beginIfReady(); return;
        case 'setBreakpoints': {
          const sameFile = path.resolve(args.source?.path || '') === path.resolve(this.sourcePath || args.source?.path || '');
          this.breakpoints = sameFile ? (args.breakpoints || []).map((bp: any) => ({ id: this.nextBreakpoint++, line: bp.line, column: bp.column })) : this.breakpoints;
          if (sameFile) this.resolveBreakpoints();
          this.reply(request, { breakpoints: sameFile ? this.breakpoints.map((bp) => this.breakpointView(bp)) : (args.breakpoints || []).map(() => ({ verified: false, message: vscode.l10n.t('Only the launched source file is supported.') })) }); return;
        }
        case 'breakpointLocations':
          this.reply(request, { breakpoints: this.locations.filter((location) => this.isOuterCell(location) && location.line >= args.line && location.line <= (args.endLine || args.line)).map((location) => ({ line: location.line, column: location.column, endLine: location.endLine, endColumn: location.endColumn })) }); return;
        case 'threads':
          this.reply(request, { threads: this.liveThreads().map((thread) => ({ id: this.threadId(thread), name: `Outer ${thread.id} (${this.boardPath(thread)})` })) }); return;
        case 'stackTrace': this.reply(request, this.stackTrace(args.threadId)); return;
        case 'scopes': {
          const thread = this.frameThreads.get(args.frameId);
          this.reply(request, { scopes: [
            { name: vscode.l10n.t('Registers'), variablesReference: this.reference(thread?.private_registers ?? this.snapshot.registers ?? []), expensive: false },
            { name: 'Status flag F', variablesReference: this.reference({ F: thread?.status_flag ?? 0 }), expensive: false },
            { name: vscode.l10n.t('Thread'), variablesReference: this.reference(thread || {}), expensive: false },
            { name: vscode.l10n.t('Memory'), variablesReference: this.reference(this.snapshot.memory || []), expensive: false },
            { name: vscode.l10n.t('Input / Output'), variablesReference: this.reference({ remaining_input: this.snapshot.remaining_input, output: this.snapshot.output }), expensive: false },
            { name: vscode.l10n.t('Metrics'), variablesReference: this.reference(this.snapshot.metrics || {}), expensive: false },
            { name: vscode.l10n.t('Last Tick Events'), variablesReference: this.reference(this.lastEvents), expensive: false },
            { name: vscode.l10n.t('Errors'), variablesReference: this.reference({ messages: this.snapshot.status === 'error' ? presentRuntimeErrors(this.snapshot).map(({ code, message }) => `${errorLabel(code)} ${message}`) : [], errors: this.snapshot.errors, fault: this.snapshot.fault }), expensive: false },
          ] }); return;
        }
        case 'variables': {
          const value = this.variables.get(args.variablesReference);
          const entries = value && typeof value === 'object' ? Object.entries(value) : [];
          const start = args.start || 0;
          this.reply(request, { variables: entries.slice(start, args.count ? start + args.count : undefined).map(([name, item]) => this.variable(name, item)) }); return;
        }
        case 'evaluate': {
          const thread = this.frameThreads.get(args.frameId) || this.liveThreads()[0];
          const data = { ...this.snapshot, registers: thread?.private_registers ?? this.snapshot.registers, thread, events: this.lastEvents };
          const expression = String(args.expression).trim();
          if (!/^[A-Za-z_][\w]*(?:(?:\.[A-Za-z_][\w]*)|(?:\[\d+\]))*$/.test(expression)) throw new CodedError('editor.invalid_expression', 'Use a state path such as registers[0], thread.data_stack or metrics.global_tick.');
          const keys = expression.replace(/\[(\d+)\]/g, '.$1').split('.');
          let value: any = data;
          for (const key of keys) {
            if (!value || typeof value !== 'object' || !Object.prototype.hasOwnProperty.call(value, key)) throw new CodedError('editor.unknown_state_path', vscode.l10n.t('Unknown state path: {0}', expression));
            value = value[key];
          }
          const variable = this.variable(expression, value);
          this.reply(request, { result: variable.value, variablesReference: variable.variablesReference }); return;
        }
        case 'continue': this.ensureStopped(); this.reply(request, { allThreadsContinued: true }); void this.execute(false); return;
        case 'next': case 'stepIn': this.ensureStopped(); this.reply(request); void this.execute(true); return;
        case 'stepOut': {
          this.ensureStopped();
          const thread = this.liveThreads().find((item) => this.threadId(item) === args.threadId);
          if (!thread || !thread.call_stack.length) throw new CodedError('editor.no_caller_frame', 'The selected thread has no caller frame.');
          this.reply(request); void this.execute(false, { id: thread.id, depth: thread.call_stack.length }); return;
        }
        case 'pause': this.pauseRequested = true; this.reply(request); return;
        case 'disconnect': case 'terminate': this.reply(request); this.finish(); return;
        case 'setExceptionBreakpoints': this.reply(request); return;
        default: this.reply(request, {}, new CodedError('editor.unsupported_debug_request', vscode.l10n.t('Unsupported debug request: {0}', request.command)));
      }
    } catch (error) {
      this.reply(request, {}, codedError(error));
      if (request.command === 'launch') this.finish();
    }
  }

  private async launch(args: any): Promise<void> {
    if (!vscode.workspace.isTrusted) throw new CodedError('editor.workspace_untrusted', 'Trust this workspace before running CodeGrid.');
    if (typeof args.program !== 'string' || !args.program) throw new CodedError('editor.invalid_program', 'program must be a source file path.');
    this.sourcePath = path.resolve(args.program);
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(this.sourcePath));
    this.sourceText = document.getText();
    this.launchOptions = args;
    const input = args.input ?? [];
    if (!Array.isArray(input) || !input.every((value: unknown) => typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 255)) throw new CodedError('editor.invalid_input', 'input must be an array of byte integers (0..255).');
    this.runtime = new NativeRuntime(args.runtimePath || runtimePath(this.context), (text) => this.output(text, 'stderr'));
    const loaded = await this.runtime.request({ command: 'launch', source: this.sourceText, input, boundary: args.boundary || 'exit', seed: args.seed ?? '0', custom_limit: args.customLimit ?? '10000', max_ticks: args.maxTicks ?? '100000', max_work_units: args.maxWorkUnits ?? '1000000' });
    if (loaded.diagnostics) {
      const bytes = Buffer.from(this.sourceText, 'utf8');
      for (const diagnostic of loaded.diagnostics) {
        const prefix = bytes.subarray(0, Number(diagnostic.span.start)).toString('utf8');
        const lines = prefix.split('\n');
        this.output(`${this.sourcePath}:${lines.length}:${lines[lines.length - 1].length + 1}: ${errorLabel(diagnostic.code)} ${localizeError(diagnostic.code, diagnostic.message)}\n`, 'stderr');
      }
      throw new CodedError('editor.source_errors', 'The program has source errors. See the Debug Console.');
    }
    this.snapshot = loaded.snapshot!;
    this.locations = loaded.locations!;
    this.resolveBreakpoints();
    for (const breakpoint of this.breakpoints) this.event('breakpoint', { reason: 'changed', breakpoint: this.breakpointView(breakpoint) });
    this.output(vscode.l10n.t('CodeGrid: each step executes one atomic Global Tick across all live threads. Custom execution is included in that tick.') + '\n');
  }

  private beginIfReady(): void {
    if (!this.configured || !this.runtime || !this.snapshot.status || this.running) return;
    this.configured = false;
    if (!this.launchOptions.noDebug && this.launchOptions.stopOnEntry !== false) this.stop('entry');
    else void this.execute(false);
  }

  private ensureStopped(): void {
    if (!this.runtime || this.closed) throw new CodedError('editor.no_session', 'No active runtime.');
    if (this.running) throw new CodedError('editor.session_running', 'Pause execution before stepping.');
    if (this.snapshot.status !== 'running') throw new CodedError('editor.vm_terminal', 'The VM is terminal. Restart the session to execute again.');
  }

  private async execute(single: boolean, caller?: { id: string; depth: number }): Promise<void> {
    this.running = true;
    this.pauseRequested = false;
    this.event('continued', { threadId: this.liveThreads()[0] ? this.threadId(this.liveThreads()[0]) : 1, allThreadsContinued: true });
    let skip = this.skipCurrentBreakpoint;
    this.skipCurrentBreakpoint = false;
    try {
      while (this.running && !this.closed && !this.pauseRequested) {
        if (!single && !skip && !this.launchOptions.noDebug) {
          const hit = this.hitBreakpoint();
          if (hit) { this.skipCurrentBreakpoint = true; this.stop('breakpoint', { hitBreakpointIds: [hit.breakpoint.id], threadId: hit.threadId }); return; }
        }
        skip = false;
        const result = await this.runtime!.request({ command: 'step' });
        if (this.closed) return;
        this.update(result);
        if (this.snapshot.status === 'error') {
          const issues = presentRuntimeErrors(this.snapshot);
          const description = issues.map((issue) => `${errorLabel(issue.code)} ${issue.message}`).join('\n');
          this.output(description + '\n', 'stderr');
          this.output(vscode.l10n.t('Details: {0}', JSON.stringify(this.snapshot.fault || this.snapshot.errors)) + '\n', 'stderr');
          if (this.launchOptions.noDebug) { this.event('exited', { exitCode: 5 }); this.finish(); }
          else this.stop('exception', { description, text: description, code: issues[0].code, codes: issues.map((issue) => issue.code) });
          return;
        }
        if (this.snapshot.status === 'halted') { this.output(vscode.l10n.t('Program halted after {0} Global Ticks.', this.snapshot.committed_ticks) + '\n'); this.event('exited', { exitCode: 0 }); this.finish(); return; }
        if (single || (caller && !this.liveThreads().some((thread) => thread.id === caller.id && thread.call_stack.length >= caller.depth))) { this.stop('step'); return; }
      }
      if (!this.closed) this.stop('pause');
    } catch (error) {
      if (!this.closed) {
        const failure = codedError(error);
        const message = `${errorLabel(failure.code)} ${failure.message}`;
        this.output(message + '\n', 'stderr');
        if (this.launchOptions.noDebug) { this.event('exited', { exitCode: 1 }); this.finish(); }
        else this.stop('exception', { description: message, text: message, code: failure.code });
      }
    }
  }

  private update(reply: RuntimeReply): void {
    this.snapshot = reply.snapshot!;
    this.lastEvents = reply.events || [];
    if (reply.newly_emitted_output?.length) {
      const bytes = reply.newly_emitted_output;
      this.output(vscode.l10n.t('Output bytes: {0}', bytes.join(', ')) + '\n', 'stdout');
    }
  }

  private stop(reason: string, details: any = {}): void {
    this.running = false;
    this.variables.clear(); this.nextVariable = 1;
    this.frameThreads.clear();
    this.nextFrame = 1;
    this.event('stopped', { reason, threadId: this.liveThreads()[0] ? this.threadId(this.liveThreads()[0]) : 1, allThreadsStopped: true, ...details });
  }

  private finish(): void {
    if (this.closed) return;
    this.running = false;
    this.runtime?.dispose();
    this.event('terminated');
    this.closed = true;
  }

  private liveThreads(): any[] { return this.snapshot.threads || []; }
  private threadId(thread: any): number {
    let id = this.threadIds.get(thread.id);
    if (id === undefined) { id = this.threadIds.size + 1; this.threadIds.set(thread.id, id); }
    return id;
  }
  private boardPath(thread: any): string { return thread.board.kind === 'main' ? '@main' : `@main.F${thread.board.id}`; }
  private cellPath(thread: any, board = thread.board, position = thread.position): string {
    const prefix = board.kind === 'main' ? '@main' : `@main.F${board.id}`;
    if (board === thread.board && thread.phase.kind === 'fold') return `${prefix}.M${thread.phase.fold_id}[${thread.phase.internal_position.x}]`;
    const runtimeBoard = board.kind === 'main' ? this.snapshot.runtime_program.main : this.snapshot.runtime_program.functions[String(board.id)];
    return `${prefix}[${position.y * Number(runtimeBoard.width) + position.x}]`;
  }
  private isOuterCell(location: SourceLocation): boolean { return location.path.startsWith('@main') && location.path.endsWith(']'); }
  private resolveBreakpoints(): void {
    for (const bp of this.breakpoints) bp.location = this.locations.filter((location) => this.isOuterCell(location) && location.line === bp.line).sort((a, b) => a.column - b.column).find((location) => !bp.column || location.endColumn > bp.column);
  }
  private breakpointView(bp: Breakpoint): any {
    return { id: bp.id, verified: !!bp.location, line: bp.location?.line || bp.line, column: bp.location?.column || bp.column, source: { path: this.sourcePath }, ...(bp.location ? {} : { message: vscode.l10n.t('Select a Main/Function/Folded Block cell. Custom runs atomically inside its caller tick.') }) };
  }
  private hitBreakpoint(): { breakpoint: Breakpoint; threadId: number } | undefined {
    for (const thread of this.liveThreads().filter((item) => item.phase.kind !== 'terminated')) {
      const cell = this.cellPath(thread);
      const breakpoint = this.breakpoints.find((bp) => bp.location?.path === cell);
      if (breakpoint) return { breakpoint, threadId: this.threadId(thread) };
    }
    return undefined;
  }

  private stackTrace(threadId: number): any {
    const thread = this.liveThreads().find((item) => this.threadId(item) === threadId);
    if (!thread) return { stackFrames: [], totalFrames: 0 };
    const entries = [{ name: `${this.boardPath(thread)} (${thread.position.x}, ${thread.position.y})`, path: this.cellPath(thread), state: thread }, ...[...thread.call_stack].reverse().map((frame: any) => ({ name: frame.caller_board.kind === 'main' ? '@main' : `@main.F${frame.caller_board.id}`, path: this.cellPath(thread, frame.caller_board, frame.call_position), state: { ...thread, board: frame.caller_board, position: frame.call_position, register_pointer: frame.saved_register_pointer, status_flag: frame.saved_status_flag, private_registers: frame.saved_registers } }))];
    const stackFrames = entries.map((entry) => {
      const id = this.nextFrame++;
      this.frameThreads.set(id, entry.state);
      const location = this.locations.find((item) => item.path === entry.path);
      return { id, name: entry.name, source: { name: path.basename(this.sourcePath), path: this.sourcePath }, line: location?.line || 1, column: location?.column || 1, endLine: location?.endLine, endColumn: location?.endColumn };
    });
    return { stackFrames, totalFrames: stackFrames.length };
  }
  private reference(value: any): number { const id = this.nextVariable++; this.variables.set(id, value); return id; }
  private variable(name: string, value: any): any {
    const composite = value !== null && typeof value === 'object';
    return { name, value: composite ? (Array.isArray(value) ? `Array(${value.length})` : 'Object') : String(value), type: Array.isArray(value) ? 'array' : typeof value, variablesReference: composite ? this.reference(value) : 0, ...(Array.isArray(value) ? { indexedVariables: value.length } : {}) };
  }
}

export function registerExecution(context: vscode.ExtensionContext): vscode.Disposable[] {
  const start = async (noDebug: boolean, uri?: vscode.Uri): Promise<boolean> => {
    const document = uri ? await vscode.workspace.openTextDocument(uri) : vscode.window.activeTextEditor?.document;
    if (!document || document.languageId !== 'codegrid') { void vscode.window.showErrorMessage(`${errorLabel('editor.no_codegrid_file')} ${vscode.l10n.t('[editor.no_codegrid_file] Open a CodeGrid (.cg) file first.')}`); return false; }
    if (document.isUntitled && !await document.save()) return false;
    const defaults = vscode.workspace.getConfiguration('codegrid.execution');
    return vscode.debug.startDebugging(vscode.workspace.getWorkspaceFolder(document.uri), { type: 'codegrid', request: 'launch', name: noDebug ? vscode.l10n.t('Run CodeGrid') : vscode.l10n.t('Debug CodeGrid'), program: document.uri.fsPath, noDebug, stopOnEntry: !noDebug, input: defaults.get('input', []), boundary: defaults.get('boundary', 'exit'), seed: defaults.get('seed', '0'), customLimit: defaults.get('customLimit', '10000'), maxTicks: defaults.get('maxTicks', '100000'), maxWorkUnits: defaults.get('maxWorkUnits', '1000000'), internalConsoleOptions: 'openOnSessionStart' }, { noDebug });
  };
  return [
    vscode.commands.registerCommand('codegrid.run', (uri?: vscode.Uri) => start(true, uri).catch((error) => { const failure = codedError(error); void vscode.window.showErrorMessage(`${errorLabel(failure.code)} ${failure.message}`); return false; })),
    vscode.commands.registerCommand('codegrid.debug', (uri?: vscode.Uri) => start(false, uri).catch((error) => { const failure = codedError(error); void vscode.window.showErrorMessage(`${errorLabel(failure.code)} ${failure.message}`); return false; })),
    vscode.debug.registerDebugAdapterDescriptorFactory('codegrid', { createDebugAdapterDescriptor: () => new vscode.DebugAdapterInlineImplementation(new CodeGridDebugAdapter(context)) }),
    vscode.debug.registerDebugConfigurationProvider('codegrid', {
      provideDebugConfigurations: () => [{ type: 'codegrid', request: 'launch', name: vscode.l10n.t('Debug CodeGrid'), program: '${file}', stopOnEntry: true }],
      resolveDebugConfiguration: (_folder, configuration) => {
        if (!configuration.type && vscode.window.activeTextEditor?.document.languageId === 'codegrid') Object.assign(configuration, { type: 'codegrid', request: 'launch', name: vscode.l10n.t('Debug CodeGrid'), program: '${file}', stopOnEntry: true });
        return configuration;
      },
    }),
  ];
}
