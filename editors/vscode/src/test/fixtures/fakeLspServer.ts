import { stdin, stdout } from 'process';

let input = Buffer.alloc(0);

function send(message: Record<string, unknown>): void {
  const body = Buffer.from(JSON.stringify(message), 'utf8');
  stdout.write(`Content-Length: ${body.length}\r\n\r\n`);
  stdout.write(body);
}

function handle(message: Record<string, unknown>): void {
  if (message.method === 'exit') {
    process.exit(0);
  }

  if (message.id === undefined) return;

  if (message.method === 'initialize') {
    send({
      jsonrpc: '2.0',
      id: message.id,
      result: {
        capabilities: {
          textDocumentSync: { openClose: true, change: 2 },
          completionProvider: { triggerCharacters: ['#'] }
        },
        serverInfo: { name: 'CodeGrid test server', version: '0.0.0' }
      }
    });
  } else if (message.method === 'textDocument/completion') {
    send({
      jsonrpc: '2.0',
      id: message.id,
      result: {
        isIncomplete: false,
        items: [{ label: '#lsp-only', kind: 14 }]
      }
    });
    if (process.argv.includes('--exit-after-completion')) {
      setTimeout(() => process.exit(1), 100);
    }
  } else {
    send({ jsonrpc: '2.0', id: message.id, result: null });
  }
}

stdin.on('data', (chunk: Buffer) => {
  input = Buffer.concat([input, chunk]);
  while (true) {
    const boundary = input.indexOf('\r\n\r\n');
    if (boundary < 0) return;
    const headers = input.subarray(0, boundary).toString('ascii');
    const length = Number(/Content-Length:\s*(\d+)/i.exec(headers)?.[1]);
    const messageStart = boundary + 4;
    if (!Number.isSafeInteger(length) || length < 0 || input.length < messageStart + length) return;
    const body = input.subarray(messageStart, messageStart + length);
    input = input.subarray(messageStart + length);
    handle(JSON.parse(body.toString('utf8')) as Record<string, unknown>);
  }
});
