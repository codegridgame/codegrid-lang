import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as vscode from 'vscode';
import { CodeGridDebugAdapter } from '../../debug/adapter';

const executable = process.env.CODEGRID_CLI_TEST_BIN;

class Client {
  readonly messages: any[] = [];
  readonly adapter: CodeGridDebugAdapter;
  private seq = 1;
  constructor() {
    const extension = vscode.extensions.getExtension('codegrid.codegrid-vscode')!;
    this.adapter = new CodeGridDebugAdapter({ extensionPath: extension.extensionPath } as vscode.ExtensionContext);
    this.adapter.onDidSendMessage((message) => this.messages.push(message));
  }
  async request(command: string, args: any = {}): Promise<any> {
    const seq = this.seq++;
    this.adapter.handleMessage({ seq, type: 'request', command, arguments: args });
    const response = await this.wait((message) => message.type === 'response' && message.request_seq === seq);
    assert.strictEqual(response.success, true, response.message);
    return response.body;
  }
  async wait(predicate: (message: any) => boolean, start = 0): Promise<any> {
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      const match = this.messages.slice(start).find(predicate);
      if (match) return match;
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
    throw new Error('Debug response timed out.');
  }
  async launch(program: string, options: any = {}): Promise<void> {
    await this.request('initialize');
    await this.request('launch', { program, runtimePath: executable, ...options });
    await this.request('configurationDone');
  }
}

suite('Native Run and Debug', () => {
  let folder: string;
  let client: Client;
  setup(function () {
    if (!executable) this.skip();
    folder = fs.mkdtempSync(path.join(os.tmpdir(), 'codegrid-debug-'));
    client = new Client();
  });
  teardown(() => {
    client?.adapter.dispose();
    if (folder) fs.rmSync(folder, { recursive: true, force: true });
  });
  function file(source: string): string {
    const program = path.join(folder, 'program.cg');
    fs.writeFileSync(program, source);
    return program;
  }

  test('active and suspended Function frames expose independent F values', async () => {
    await client.launch(file('@main\n~> ) [0 ?!.1 ;\n@F0\n~> + ] _ _\n@end F0\n@end main\n'));
    await client.wait((message) => message.event === 'stopped');
    for (let tick = 0; tick < 5; tick++) {
      const mark = client.messages.length;
      await client.request('stepIn', { threadId: 1 });
      await client.wait((message) => message.event === 'stopped', mark);
    }
    const frames = await client.request('stackTrace', { threadId: 1 });
    assert.strictEqual(frames.stackFrames.length, 2);
    assert.strictEqual((await client.request('evaluate', { expression: 'thread.status_flag', frameId: frames.stackFrames[0].id })).result, '0');
    assert.strictEqual((await client.request('evaluate', { expression: 'thread.status_flag', frameId: frames.stackFrames[1].id })).result, '1');
    const mark = client.messages.length;
    await client.request('stepIn', { threadId: 1 });
    await client.wait((message) => message.event === 'stopped', mark);
    assert.strictEqual((await client.request('evaluate', { expression: 'thread.status_flag' })).result, '1');
  });

  test('entry, tick step, UTF-16 source frame and watch use the actual VM', async () => {
    await client.launch(file('/*🙂*/ ~> + . ;\r\n'), { seed: '18446744073709551615' });
    await client.wait((message) => message.event === 'stopped' && message.body.reason === 'entry');
    const threads = await client.request('threads');
    assert.strictEqual(threads.threads.length, 1);
    const trace = await client.request('stackTrace', { threadId: 1 });
    assert.strictEqual(trace.stackFrames[0].line, 1);
    assert.strictEqual(trace.stackFrames[0].column, 8);
    const mark = client.messages.length;
    await client.request('next', { threadId: 1 });
    await client.wait((message) => message.event === 'stopped', mark);
    assert.strictEqual((await client.request('evaluate', { expression: 'committed_ticks' })).result, '1');
    const second = client.messages.length;
    await client.request('stepIn', { threadId: 1 });
    await client.wait((message) => message.event === 'stopped', second);
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]' })).result, '1');
    const scopes = await client.request('scopes', { frameId: trace.stackFrames[0].id });
    const registers = await client.request('variables', { variablesReference: scopes.scopes[0].variablesReference });
    assert.strictEqual(registers.variables[0].value, '1');
  });

  test('column breakpoint stops before its cell and continues without re-hitting it', async () => {
    const program = file('~> + . ;\n');
    await client.request('initialize');
    await client.request('setBreakpoints', { source: { path: program }, breakpoints: [{ line: 1, column: 6 }] });
    await client.request('launch', { program, runtimePath: executable, stopOnEntry: false });
    await client.request('configurationDone');
    await client.wait((message) => message.event === 'stopped' && message.body.reason === 'breakpoint');
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]' })).result, '1');
    await client.request('continue');
    await client.wait((message) => message.event === 'terminated');
    assert.ok(client.messages.some((message) => message.event === 'output' && message.body.output.includes('Output bytes: 1')));
  });

  test('run without debugging emits input/output and terminates', async () => {
    await client.launch(file('~> , . ;\n'), { noDebug: true, input: [65] });
    await client.wait((message) => message.event === 'terminated');
    assert.ok(client.messages.some((message) => message.event === 'output' && message.body.output.includes('Output bytes: 65')));
    assert.ok(client.messages.some((message) => message.event === 'exited' && message.body.exitCode === 0));
  });

  test('work ceiling pauses with rolled-back VM state', async () => {
    await client.launch(file('~> ~<\n'), { maxWorkUnits: '1' });
    const mark = client.messages.length;
    await client.request('next');
    const failure = await client.wait((message) => message.event === 'stopped' && message.body.reason === 'exception', mark);
    assert.strictEqual(failure.body.code, 'debug.work_limit_exceeded');
    assert.strictEqual((await client.request('evaluate', { expression: 'committed_ticks' })).result, '0');
  });

  test('tick ceiling pauses and stop terminates the native process', async () => {
    await client.launch(file('~> _\n'), { maxTicks: '2' });
    await client.request('continue');
    const failure = await client.wait((message) => message.event === 'stopped' && message.body.reason === 'exception');
    assert.strictEqual(failure.body.code, 'debug.tick_limit_exceeded');
    assert.strictEqual((await client.request('evaluate', { expression: 'committed_ticks' })).result, '2');
    await client.request('disconnect');
    await client.wait((message) => message.event === 'terminated');
  });

  test('a real VM conflict stops with its stable code and readable error text', async () => {
    await client.launch(file('~> . ~<\n'));
    await client.request('continue');
    const failure = await client.wait((message) => message.event === 'stopped' && message.body.reason === 'exception');
    assert.strictEqual(failure.body.code, 'ConcurrentOutputConflict');
    assert.deepStrictEqual(failure.body.codes, ['ConcurrentOutputConflict']);
    assert.ok(failure.body.description.includes('[ConcurrentOutputConflict]'));
    assert.ok(client.messages.some((message) => message.event === 'output' && message.body.output.includes('[ConcurrentOutputConflict]')));
    assert.strictEqual((await client.request('evaluate', { expression: 'errors[0].code' })).result, 'ConcurrentOutputConflict');
  });

  test('invalid source produces compiler diagnostics and no execution', async () => {
    await client.request('initialize');
    let rejected = false;
    try { await client.request('launch', { program: file('~x\n'), runtimePath: executable }); }
    catch { rejected = true; }
    assert.ok(rejected);
    await client.wait((message) => message.event === 'terminated');
    assert.ok(client.messages.some((message) => message.event === 'output' && message.body.category === 'stderr'));
  });

  test('editor exposes Run, Debug and the CodeGrid debugger', () => {
    const manifest = vscode.extensions.getExtension('codegrid.codegrid-vscode')!.packageJSON;
    assert.ok(manifest.contributes.commands.some((command: any) => command.command === 'codegrid.run'));
    assert.ok(manifest.contributes.commands.some((command: any) => command.command === 'codegrid.debug'));
    assert.strictEqual(manifest.contributes.debuggers[0].type, 'codegrid');
    assert.strictEqual(manifest.contributes.breakpoints[0].language, 'codegrid');
  });

  test('Function call frames and step out discard private registers', async () => {
    await client.launch(file('@main\n@size 4x1\n~> [0 . ;\n@F0\n@size 3x1\n~> + ]\n@end F0\n@end main\n'));
    for (let count = 0; count < 2; count++) {
      const mark = client.messages.length;
      await client.request('next');
      await client.wait((message) => message.event === 'stopped', mark);
    }
    const frames = await client.request('stackTrace', { threadId: 1 });
    assert.strictEqual(frames.stackFrames.length, 2);
    assert.strictEqual(frames.stackFrames[0].line, 6);
    const mark = client.messages.length;
    await client.request('stepOut', { threadId: 1 });
    await client.wait((message) => message.event === 'stopped', mark);
    assert.strictEqual((await client.request('evaluate', { expression: 'thread.call_stack' })).result, 'Array(0)');
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]' })).result, '0');
  });

  test('Function and suspended caller scopes expose different register banks', async () => {
    await client.launch(file('@main\n~> , [0 . ;\n@F0\n~> $! } ]\n@end F0\n@end main\n'), { input: [5] });
    for (let count = 0; count < 5; count++) {
      const mark = client.messages.length;
      await client.request('next', { threadId: 1 });
      await client.wait((message) => message.event === 'stopped', mark);
    }
    const frames = await client.request('stackTrace', { threadId: 1 });
    assert.strictEqual(frames.stackFrames.length, 2);
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]', frameId: frames.stackFrames[0].id })).result, '251');
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]', frameId: frames.stackFrames[1].id })).result, '5');
    const scopes = await client.request('scopes', { frameId: frames.stackFrames[0].id });
    const registers = await client.request('variables', { variablesReference: scopes.scopes[0].variablesReference });
    assert.strictEqual(registers.variables[0].value, '251');
    const mark = client.messages.length;
    await client.request('stepOut', { threadId: 1 });
    await client.wait((message) => message.event === 'stopped', mark);
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]' })).result, '5');
    assert.strictEqual((await client.request('evaluate', { expression: 'thread.register_pointer' })).result, '0');
  });

  test('pause remains responsive during a continuing wrapped program', async () => {
    await client.launch(file('~> _\n'), { maxTicks: '1000000' });
    const mark = client.messages.length;
    await client.request('continue');
    await client.request('pause');
    await client.wait((message) => message.event === 'stopped' && message.body.reason === 'pause', mark);
    await client.request('disconnect');
  });

  test('Folded Block column breakpoints use compiler-owned source cells', async () => {
    const program = file('@main\n@size 4x1\n~> $0 . ;\n@M0 + + _ ^\n@end main\n');
    await client.launch(program);
    const breakpoints = await client.request('setBreakpoints', { source: { path: program }, breakpoints: [{ line: 4, column: 9 }] });
    assert.strictEqual(breakpoints.breakpoints[0].verified, true);
    await client.request('continue');
    await client.wait((message) => message.event === 'stopped' && message.body.reason === 'breakpoint');
    const trace = await client.request('stackTrace', { threadId: 1 });
    assert.strictEqual(trace.stackFrames[0].line, 4);
    assert.strictEqual(trace.stackFrames[0].column, 9);
    assert.strictEqual((await client.request('evaluate', { expression: 'registers[0]' })).result, '2');
  });

  test('a sibling thread breakpoint selects that thread and advances all threads atomically', async () => {
    const program = file('~> _ ;\n~> _ ;\n');
    await client.launch(program);
    const threads = await client.request('threads');
    assert.strictEqual(threads.threads.length, 2);
    await client.request('setBreakpoints', { source: { path: program }, breakpoints: [{ line: 2, column: 4 }] });
    await client.request('continue');
    const stopped = await client.wait((message) => message.event === 'stopped' && message.body.reason === 'breakpoint');
    assert.strictEqual(stopped.body.threadId, threads.threads[1].id);
    assert.strictEqual((await client.request('evaluate', { expression: 'committed_ticks' })).result, '1');
    const first = await client.request('stackTrace', { threadId: threads.threads[0].id });
    const second = await client.request('stackTrace', { threadId: threads.threads[1].id });
    assert.strictEqual(first.stackFrames[0].line, 1);
    assert.strictEqual(second.stackFrames[0].line, 2);
    assert.ok((await client.request('scopes', { frameId: first.stackFrames[0].id })).scopes.length);
  });

  test('VS Code starts and terminates a real CodeGrid debug session', async () => {
    const extension = vscode.extensions.getExtension('codegrid.codegrid-vscode')!;
    await extension.activate();
    const name = 'CodeGrid native integration';
    let subscription: vscode.Disposable | undefined;
    const terminated = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('VS Code debug session did not terminate.')), 10000);
      subscription = vscode.debug.onDidTerminateDebugSession((session) => {
        if (session.name === name) { clearTimeout(timer); resolve(); }
      });
    });
    try {
      assert.ok(await vscode.debug.startDebugging(undefined, { name, type: 'codegrid', request: 'launch', program: file('~> , . ;\n'), runtimePath: executable, noDebug: true, input: [65] }, { noDebug: true }));
      await terminated;
    } finally { subscription?.dispose(); }
  });
});
