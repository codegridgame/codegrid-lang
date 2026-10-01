import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { CodedError } from '../../debug/errors';
import { errorNumber, ERROR_MESSAGES, localizeError, localizeDiagnostic, presentRuntimeErrors } from '../../language/errorMessages';

const root = path.resolve(__dirname, '../../..');
const read = (file: string): Record<string, string> => JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
const locales = ['de', 'fr', 'es', 'zh-tw', 'ja', 'zh-cn', 'ko', 'pt-br', 'ru'];
const placeholders = (value: string): string[] => (value.match(/\{\d+\}/g) || []).sort();
const codes = (value: string): string[] => (value.match(/\[editor\.[a-z_]+\]/g) || []).sort();

suite('Localization', () => {
  suiteSetup(async () => {
    await vscode.extensions.getExtension('codegrid.codegrid-vscode')!.activate();
  });
  test('the game language inventory contains exactly ten languages', () => {
    const inventory = JSON.parse(fs.readFileSync(path.join(root, 'l10n/locales.json'), 'utf8'));
    assert.deepStrictEqual(inventory, { default: 'en', supported: ['en', ...locales] });
  });

  test('every locale covers manifest and runtime catalogs without changing arguments or codes', () => {
    for (const [baseFile, translatedFile] of [
      ['package.nls.json', (locale: string) => `package.nls.${locale}.json`],
      ['l10n/bundle.l10n.json', (locale: string) => `l10n/bundle.l10n.${locale}.json`],
    ] as const) {
      const base = read(baseFile);
      for (const locale of locales) {
        const translated = read(translatedFile(locale));
        assert.deepStrictEqual(Object.keys(translated).sort(), Object.keys(base).sort(), locale);
        for (const [key, text] of Object.entries(base)) {
          assert.ok(translated[key].trim(), `${locale}: ${key}`);
          assert.ok(!translated[key].includes('\uFFFD'), `${locale}: ${key}`);
          assert.deepStrictEqual(placeholders(translated[key]), placeholders(text), `${locale}: ${key}`);
          assert.deepStrictEqual(codes(translated[key]), codes(text), `${locale}: ${key}`);
        }
      }
    }
  });

  test('all native and editor error identities have translations in every locale', () => {
    const registry = JSON.parse(fs.readFileSync(path.resolve(root, '../../spec/codegrid-error-codes.json'), 'utf8'));
    const layers = ['source', 'ir', 'vm', 'fault', 'debug', 'editor', 'cli'];
    const identities = registry.entries.filter((entry: { layer: string }) => layers.includes(entry.layer)).map((entry: { code: string }) => entry.code);
    assert.deepStrictEqual(Object.keys(ERROR_MESSAGES).sort(), identities.sort());
    for (const locale of locales) {
      const catalog = read(`l10n/bundle.l10n.${locale}.json`);
      for (const code of identities) assert.ok(catalog[ERROR_MESSAGES[code]], `${locale}: ${code}`);
    }
  });

  test('error translation follows the code and keeps native messages and diagnostic positions', () => {
    const original = 'Unrelated wording from a native host';
    const error = new CodedError('debug.work_limit_exceeded', original);
    assert.strictEqual(error.originalMessage, original);
    assert.strictEqual(error.code, 'debug.work_limit_exceeded');
    assert.strictEqual(error.errorNumber, '5106');
    assert.strictEqual(errorNumber('ConcurrentOutputConflict'), '3006');
    const english = vscode.env.language.toLowerCase().startsWith('en');
    assert.strictEqual(error.message, english ? original : vscode.l10n.t(ERROR_MESSAGES[error.code]));
    const unknown = new CodedError('future.error', original);
    assert.strictEqual(unknown.code, 'future.error');
    assert.strictEqual(unknown.message, english ? original : vscode.l10n.t('An error occurred. Inspect the details for more information.'));
    const diagnostic = new vscode.Diagnostic(new vscode.Range(2, 3, 2, 6), original);
    diagnostic.code = 'source.invalid_cell';
    diagnostic.source = 'CodeGrid';
    const translated = localizeDiagnostic(vscode.Uri.file('example.cg'), diagnostic);
    assert.strictEqual(translated.code, diagnostic.code);
    assert.deepStrictEqual(translated.range, diagnostic.range);
    assert.strictEqual(diagnostic.message, original);
    assert.ok(translated.message.includes('[1004] [source.invalid_cell]'));
    if (!english) {
      assert.ok(translated.message.includes('[source.invalid_cell]'));
      assert.ok(translated.message.includes(localizeError('source.invalid_cell')));
      assert.ok(translated.relatedInformation!.some((entry) => entry.message.includes(original)));
    }
    const numeric = new vscode.Diagnostic(diagnostic.range, original);
    numeric.code = 99901;
    const future = localizeDiagnostic(vscode.Uri.file('example.cg'), numeric);
    assert.strictEqual(future.code, 99901);
    if (!english) assert.ok(future.message.includes('[99901]'));
  });

  test('structured VM exceptions expose stable codes and localized messages without changing raw state', () => {
    const snapshot = { errors: [{ code: 'ConcurrentOutputConflict', details: { thread_ids: ['0', '1'] } }], fault: { kind: 'global_tick_overflow' } };
    const original = JSON.stringify(snapshot);
    const issues = presentRuntimeErrors(snapshot);
    assert.deepStrictEqual(issues.map((issue) => issue.code), ['ConcurrentOutputConflict', 'global_tick_overflow']);
    for (const issue of issues) assert.strictEqual(issue.message, vscode.l10n.t(ERROR_MESSAGES[issue.code]));
    assert.strictEqual(JSON.stringify(snapshot), original);
  });

  test('real native diagnostics, dispatch failures and VM conflicts use translated identities', async function () {
    const executable = process.env.CODEGRID_CLI_TEST_BIN;
    if (!executable) this.skip();
    const { NativeRuntime } = await import('../../debug/nativeRuntime');
    const runtime = new NativeRuntime(executable!, () => undefined);
    const configuration = { command: 'launch', input: [], boundary: 'wrap', seed: '0', custom_limit: '10000', max_ticks: '100', max_work_units: '1' };
    try {
      const rejected = await runtime.request({ ...configuration, source: '~x\n' });
      assert.strictEqual(rejected.diagnostics![0].code, 'source.invalid_entry');
      const diagnostic = rejected.diagnostics![0];
      assert.strictEqual(localizeError(diagnostic.code, diagnostic.message),
        vscode.env.language.startsWith('en') ? diagnostic.message : vscode.l10n.t(ERROR_MESSAGES[diagnostic.code]));
      await runtime.request({ ...configuration, source: '~> ~<\n' });
      await assert.rejects(runtime.request({ command: 'step' }), (failure: unknown) => {
        assert.ok(failure instanceof CodedError);
        assert.strictEqual(failure.code, 'debug.work_limit_exceeded');
        assert.strictEqual(failure.message, localizeError(failure.code, failure.originalMessage));
        return true;
      });
    } finally { runtime.dispose(); }
    const conflict = new NativeRuntime(executable!, () => undefined);
    try {
      await conflict.request({ ...configuration, source: '~> . ~<\n', max_work_units: '1000' });
      await conflict.request({ command: 'step' });
      const result = await conflict.request({ command: 'step' });
      assert.strictEqual(result.snapshot!.status, 'error');
      const issues = presentRuntimeErrors(result.snapshot!);
      assert.strictEqual(issues[0].code, 'ConcurrentOutputConflict');
      assert.strictEqual(issues[0].message, vscode.l10n.t(ERROR_MESSAGES.ConcurrentOutputConflict));
    } finally { conflict.dispose(); }
  });

  test('VS Code document formatting aligns the reported board with a named end', async () => {
    const source = '@main\n@size 5x5\n~v _ _ _ _\n_ _ _ . _\n_ _ _ _ _\n_ _ _ +x3 _\n> _ _ ^ _\n@end main\n';
    const document = await vscode.workspace.openTextDocument({ language: 'codegrid', content: source });
    const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>('vscode.executeFormatDocumentProvider', document.uri,
      { tabSize: 2, insertSpaces: true });
    assert.ok(edits.length > 0);
    let formatted = source;
    for (const edit of [...edits].sort((left, right) => document.offsetAt(right.range.start) - document.offsetAt(left.range.start))) {
      formatted = formatted.slice(0, document.offsetAt(edit.range.start)) + edit.newText + formatted.slice(document.offsetAt(edit.range.end));
    }
    const rows = formatted.split('\n').slice(2, 7);
    const positions = rows.map((row) => [...row.matchAll(/\S+/g)].map((cell) => cell.index));
    for (const row of positions) assert.deepStrictEqual(row, positions[0]);
    assert.ok(formatted.endsWith('@end main\n'));
    assert.ok(formatted.includes('+x3'));
  });

  test('VS Code loads runtime translations and preserves error identities', async () => {
    const { describeToken } = await import('../../language/describe');
    const requested = process.env.CODEGRID_TEST_LOCALE;
    if (requested) assert.strictEqual(vscode.env.language.toLowerCase(), requested, process.env.VSCODE_NLS_CONFIG);
    const locale = locales.find((value) => value === vscode.env.language.toLowerCase());
    const catalog = read(locale ? `l10n/bundle.l10n.${locale}.json` : 'l10n/bundle.l10n.json');
    const manifest = read(locale ? `package.nls.${locale}.json` : 'package.nls.json');
    const extension = vscode.extensions.getExtension('codegrid.codegrid-vscode')!;
    const title = extension.packageJSON.contributes.commands.find((command: { command: string }) => command.command === 'codegrid.run').title;
    assert.strictEqual(typeof title === 'string' ? title : title.value, manifest['codegrid.run.title']);
    assert.strictEqual(vscode.l10n.t('Select a CodeGrid template'), catalog['Select a CodeGrid template']);
    assert.strictEqual(vscode.l10n.t('Program halted after {0} Global Ticks.', 12), catalog['Program halted after {0} Global Ticks.'].replace('{0}', '12'));
    const error = new CodedError('editor.no_session', 'No active runtime.');
    assert.strictEqual(error.code, 'editor.no_session');
    assert.strictEqual(error.message, catalog['No active runtime.']);
    assert.ok(describeToken('+')!.includes(catalog['Increments R0 with 8-bit wrapping.']));
    assert.ok(describeToken('+')!.includes('`+`'));
    assert.ok(describeToken(',')!.includes('READ'));
    assert.strictEqual(vscode.l10n.t('Unregistered host detail: {0}', 'sample'), 'Unregistered host detail: sample');
  });
});
