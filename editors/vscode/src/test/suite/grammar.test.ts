import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { INITIAL, Registry, parseRawGrammar, type IGrammar, type IOnigLib } from 'vscode-textmate';
import * as oniguruma from 'vscode-oniguruma';

const EXT_ID = 'codegrid.codegrid-vscode';

interface Token {
  text: string;
  scopes: string[];
}

suite('TextMate grammar', () => {
  let grammar: IGrammar;

  suiteSetup(async () => {
    const ext = vscode.extensions.getExtension(EXT_ID);
    assert.ok(ext, 'extension must be resolvable');
    await ext.activate();
    const grammarPath = path.join(ext.extensionPath, 'syntaxes', 'codegrid.tmLanguage.json');
    const wasmPath = require.resolve('vscode-oniguruma/release/onig.wasm');
    await oniguruma.loadWASM(fs.readFileSync(wasmPath).buffer as ArrayBuffer);
    const registry = new Registry({
      onigLib: Promise.resolve({
        createOnigScanner: (sources: string[]) => new oniguruma.OnigScanner(sources),
        createOnigString: (source: string) => new oniguruma.OnigString(source),
      } as IOnigLib),
      loadGrammar: async () => parseRawGrammar(fs.readFileSync(grammarPath, 'utf8'), grammarPath),
    });
    grammar = (await registry.loadGrammar('source.cg'))!;
    assert.ok(grammar, 'grammar must load');
  });

  function tokenize(line: string): Token[] {
    return grammar.tokenizeLine(line, INITIAL).tokens.map((token) => ({
      text: line.slice(token.startIndex, token.endIndex),
      scopes: token.scopes,
    }));
  }

  function assertScope(line: string, fragment: string, scope: string): void {
    const tokens = tokenize(line);
    const token = tokens.find((entry) => entry.text === fragment)
      ?? tokens.find((entry) => entry.text.includes(fragment));
    assert.ok(token, `no token contains ${JSON.stringify(fragment)} on ${JSON.stringify(line)}`);
    assert.ok(token!.scopes.includes(scope), `expected ${scope} for ${JSON.stringify(token!.text)}; got ${token!.scopes}`);
  }

  const VALID: [string, string, string][] = [
    ['.0 .9', '.0', 'keyword.operator.output.codegrid'],
    ['.0 .9', '.9', 'keyword.operator.output.codegrid'],
    ['@main', '@main', 'keyword.directive.main.codegrid'],
    ['@END main.F0.M1', '@END', 'keyword.directive.end.codegrid'],
    ['@END main.F0.M1', 'main.F0.M1', 'entity.name.definition.codegrid'],
    ['@size 05x2', '@size', 'keyword.directive.size.codegrid'],
    ['@size 05x2', '05x2', 'constant.numeric.size.codegrid'],
    ['@C3', '@C3', 'keyword.directive.structure.codegrid'],
    ['@F2.M1', '@F2.M1', 'keyword.directive.structure.codegrid'],
    ['@main.F0.M1', '@main.F0.M1', 'keyword.directive.structure.codegrid'],
    ['@C3.F9.M0', '@C3.F9.M0', 'keyword.directive.structure.codegrid'],
    ['~> _', '~>', 'constant.language.entry.codegrid'],
    ['_', '_', 'constant.language.empty-cell.codegrid'],
    ['^ v < > ??', '??', 'keyword.operator.random-direction.codegrid'],
    ['^ v < > ??', '<', 'keyword.operator.direction.codegrid'],
    ['?0^ ?1> ?2;', '?0', 'storage.modifier.condition.codegrid'],
    ['?!}', '?!', 'storage.modifier.condition.codegrid'],
    ['?!,x3', ',', 'keyword.operator.read.codegrid'],
    ['?1?==', '?=', 'keyword.operator.compare.codegrid'],
    ['?1?==', '=', 'storage.modifier.attachment.codegrid'],
    ['?0??', '??', 'keyword.operator.random-direction.codegrid'],
    [', , , ,', ',', 'keyword.operator.read.codegrid'],
    ['! { } ( ) & %', '{', 'keyword.operator.pointer.codegrid'],
    ['! { } ( ) & %', ')', 'keyword.operator.stack.codegrid'],
    ['! { } ( ) & %', '%', 'keyword.operator.encoding.codegrid'],
    ['[0 [9 ] #0 #]', '[9', 'keyword.control.function-call.codegrid'],
    ['[0 [9 ] #0 #]', '#0', 'keyword.control.custom-call.codegrid'],
    ['[0 [9 ] #0 #]', '#]', 'keyword.control.custom-return.codegrid'],
    ['$! ?0$!x2', '$!', 'keyword.operator.arithmetic.codegrid'],
    ['$& $( $) $+ $- $< $> $0 $9', '$&', 'keyword.operator.nand.codegrid'],
    ['$& $( $) $+ $- $< $> $0 $9', '$(', 'keyword.operator.memory.codegrid'],
    ['$& $( $) $+ $- $< $> $0 $9', '$+', 'keyword.operator.page.codegrid'],
    ['$& $( $) $+ $- $< $> $0 $9', '$>', 'keyword.operator.shift.codegrid'],
    ['$& $( $) $+ $- $< $> $0 $9', '$9', 'keyword.control.folded-call.codegrid'],
    ['+* ,* [0= ]= $&x2', '*', 'storage.modifier.attachment.codegrid'],
    ['+* ,* [0= ]= $&x2', 'x2', 'storage.modifier.attachment.codegrid'],
    ['.3x3', '.3', 'keyword.operator.output.codegrid'],
    ['.3x3', 'x3', 'storage.modifier.attachment.codegrid'],
    ['?0.9x5', '?0', 'storage.modifier.condition.codegrid'],
    ['?0.9x5', 'x5', 'storage.modifier.attachment.codegrid'],
    [';', ';', 'keyword.control.halt.codegrid'],
    ['// note', '// note', 'comment.line.double-slash.codegrid'],
    ['/* note */', '/*', 'comment.block.codegrid'],
  ];

  const INVALID: [string, string][] = [
    ['.10', '.10'], ['.00', '.00'], ['.3*', '.3*'], ['.3=', '.3='], ['.3x1', '.3x1'], ['.3x6', '.3x6'],
    ['@foo', '@foo'], ['@C10', '@C10'], ['@C00', '@C00'], ['@end unknown', 'unknown'], ['@', '@'],
    ['x?', '?'], ['#00', '#00'], ['[10', '[10'], ['$10', '$10'],
    ['&x0', '&x0'], ['+x0', '+x0'], ['+x1', '+x1'], ['+x6', '+x6'], ['+x2x3', '+x2x3'],
    ['?x1', '?x1'], ['++', '++'], ['..', '..'], ['__', '__'], [';;', ';;'], [';x2', ';x2'],
    ['+x2x3', '+x2x3'], ['~x', '~x'], ['~v*', '~v*'], ['_x2', '_x2'], ['#vextra', '#vextra'],
    ['@size 0x5', '0x5'], ['@size 00x2', '00x2'], ['@size 2x2x2', '2x2x2'],
  ];

  for (const [line, fragment, scope] of VALID) {
    test(`scopes Full token ${JSON.stringify(fragment)}`, () => assertScope(line, fragment, scope));
  }

  for (const [line, fragment] of INVALID) {
    test(`marks malformed token ${JSON.stringify(fragment)} invalid`, () => {
      assertScope(line, fragment, 'invalid.illegal.codegrid');
    });
  }

  test('does not split malformed cells into valid instruction scopes', () => {
    for (const line of ['#vextra', '+x6', '+x2x3', ';x2']) {
      const tokens = tokenize(line);
      assert.ok(tokens.some((token) => token.scopes.includes('invalid.illegal.codegrid')), line);
      assert.ok(!tokens.some((token) => token.scopes.includes('keyword.operator.ifzero.codegrid')), line);
      assert.ok(!tokens.some((token) => token.scopes.includes('keyword.operator.arithmetic.codegrid')), line);
      assert.ok(!tokens.some((token) => token.scopes.includes('keyword.control.halt.codegrid')), line);
    }
  });
});
