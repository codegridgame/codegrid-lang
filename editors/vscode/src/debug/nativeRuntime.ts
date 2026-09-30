import * as vscode from 'vscode';
import { ChildProcessWithoutNullStreams, spawn } from 'child_process';
import * as readline from 'readline';
import { CodedError, codedError } from './errors';

export interface SourceLocation {
  path: string;
  line: number;
  column: number;
  endLine: number;
  endColumn: number;
}

export interface RuntimeReply {
  locations?: SourceLocation[];
  snapshot?: Record<string, any>;
  diagnostics?: Array<{ code: string; message: string; span: { start: string; end: string } }>;
  newly_emitted_output?: number[];
  events?: unknown[];
}

/** Transport only: all source acceptance and execution remain in Rust. */
export class NativeRuntime {
  private readonly child: ChildProcessWithoutNullStreams;
  private readonly pending: Array<{ resolve: (reply: RuntimeReply) => void; reject: (error: Error) => void }> = [];
  private failure: Error | undefined;

  constructor(executable: string, onStderr: (text: string) => void) {
    this.child = spawn(executable, ['debug', '--stdio'], { windowsHide: true, shell: false });
    this.child.stderr.on('data', (data: Buffer) => onStderr(data.toString()));
    this.child.stdin.on('error', (error) => this.fail(error));
    readline.createInterface({ input: this.child.stdout }).on('line', (line) => {
      const request = this.pending.shift();
      if (!request) return;
      try {
        const response = JSON.parse(line);
        if (response.debug_protocol_version !== 2) throw new CodedError('editor.unsupported_debug_protocol', 'This extension requires CodeGrid debug protocol 2.');
        if (response.error) {
          if (typeof response.error.code !== 'string' || typeof response.error.message !== 'string') throw new CodedError('editor.invalid_debug_response', 'Malformed CodeGrid debug error.');
          throw new CodedError(response.error.code, response.error.message);
        }
        request.resolve(response.body);
      } catch (error) { request.reject(codedError(error, 'editor.invalid_debug_response')); }
    });
    this.child.on('error', (error) => this.fail(codedError(error, 'editor.runtime_start_failed')));
    this.child.on('exit', (code) => this.fail(new CodedError('editor.runtime_exited', vscode.l10n.t('CodeGrid runtime exited ({0}).', String(code)))));
  }

  request(command: Record<string, unknown>): Promise<RuntimeReply> {
    if (this.failure) return Promise.reject(this.failure);
    return new Promise((resolve, reject) => {
      this.pending.push({ resolve, reject });
      this.child.stdin.write(JSON.stringify(command) + '\n', (error) => { if (error) this.fail(error); });
    });
  }

  dispose(): void {
    this.fail(new CodedError('editor.session_closed', 'CodeGrid session closed.'));
    this.child.kill();
  }

  private fail(error: Error): void {
    this.failure = codedError(error, 'editor.transport_io');
    for (const request of this.pending.splice(0)) request.reject(this.failure);
  }
}
