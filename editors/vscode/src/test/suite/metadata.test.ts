import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import { ENTRIES, PRIMARIES, parseCellToken } from '../../language/metadata';
import { describeToken } from '../../language/describe';
import { matchDirective } from '../../language/directives';

suite('Full editor metadata', () => {
  test('contains every canonical Full Primary and Entry source token', () => {
    const modelPath = path.resolve(__dirname, '../../../../../crates/codegrid-model/src/lib.rs');
    const modelSource = fs.readFileSync(modelPath, 'utf8');
    const parserStart = modelSource.indexOf('pub fn from_token(token: &str)');
    assert.notStrictEqual(parserStart, -1, 'Rust must expose the canonical token parser');
    const parserEnd = modelSource.indexOf('Some(primary)', parserStart);
    assert.notStrictEqual(parserEnd, -1, 'Rust token parser must have a clear match boundary');
    const rustTokens = [...modelSource.slice(parserStart, parserEnd).matchAll(/^\s*"([^"]+)"\s*=>\s*Self::/gm)]
      .map((match) => match[1]);

    const slotParserStart = modelSource.indexOf('fn parse_slot_instruction', parserEnd);
    assert.notStrictEqual(slotParserStart, -1, 'Rust must expose slot-token parsing');
    const slotParserEnd = modelSource.indexOf('\n}', slotParserStart);
    assert.notStrictEqual(slotParserEnd, -1, 'Rust slot-token parser must have a clear boundary');
    const slotPrefixes = [...modelSource.slice(slotParserStart, slotParserEnd).matchAll(/b'([^']+)'\s*=>/g)]
      .map((match) => match[1]);
    for (const prefix of slotPrefixes) {
      for (let id = 0; id < 10; id++) rustTokens.push(`${prefix}${id}`);
    }

    assert.deepStrictEqual(PRIMARIES.map((primary) => primary.token).sort(), rustTokens.sort());
    for (const token of rustTokens) {
      assert.ok(parseCellToken(token), `Full Primary ${token} must be recognized`);
    }
    assert.deepStrictEqual(ENTRIES.map((entry) => entry.token), ['~^', '~v', '~<', '~>']);
    for (const entry of ENTRIES) assert.ok(parseCellToken(entry.token));
    assert.ok(parseCellToken('_'));
  });

  test('recognizes complete source atoms without accepting split or malformed tokens', () => {
    for (const token of ['$!', '$!*', '$!=', '$!x3', '?0$!', '+x3', ',*', '?!,x3', '?!}', '[0=', ']=', '$&x2', '?=', '?==', '?=*', '?=x3', '?1?=', '?0??', '?2;', '?1.3']) {
      assert.ok(parseCellToken(token), `${token} is a complete attached cell token`);
    }
    for (const token of [
      ',^', ',v', ',<', ',>', '?!,vx3', '?!?0+', '?', '#^', '#v', '#<', '#>', '?0', '?3+', '?0?1+', '?0_', '?0~>', '?x1', '+x6', '+x2x3', '^*=', '[0x2', ']x2', '$0*', ';=', '#00', '[10', '$10', '#vextra', '~V', '~v*', '_x2', '++', ';x2',
    ]) {
      assert.strictEqual(parseCellToken(token), null, `${token} must not be split into a known token`);
    }
  });

  test('uses the Full attachment matrix for complete cell spellings', () => {
    for (const primary of PRIMARIES) {
      const encodable = primary.code !== null;
      assert.strictEqual(Boolean(parseCellToken(`${primary.token}*`)), encodable, `${primary.token} ReadCode`);
      assert.strictEqual(Boolean(parseCellToken(`${primary.token}=`)), encodable, `${primary.token} WriteCode`);
      const repeatable = (encodable || primary.family === 'output') && primary.family !== 'call' && primary.family !== 'return';
      for (const count of [2, 3, 4, 5]) {
        assert.strictEqual(Boolean(parseCellToken(`${primary.token}x${count}`)), repeatable, `${primary.token} Repeat ${count}`);
      }
    }
  });

  test('canonicalizes Full portable board dimensions for presentation', () => {
    assert.strictEqual(matchDirective('@MAIN', [])?.canonical, '@main');
    assert.strictEqual(matchDirective('@END', [])?.canonical, '@end');
    assert.strictEqual(matchDirective('@SIZE', ['05x2'])?.canonical, '@size 5x2');
    assert.strictEqual(matchDirective('@size', ['00x2']), null, 'zero dimensions must be declined');
    assert.strictEqual(matchDirective('@size', ['4294967296x1']), null, 'portable dimension bound must be enforced');
    assert.strictEqual(matchDirective('@size', ['65536x65536']), null, 'portable total-cell bound must be enforced');
    assert.ok(describeToken('05x2')?.includes('Board size'));
    assert.strictEqual(describeToken('@C0'), null, 'unknown editor metadata must not claim the source is invalid');
    assert.strictEqual(describeToken('00x2'), null);
  });
});

suite('Full hover descriptions', () => {
  test('describes established instructions and generic Full instruction names', () => {
    assert.ok(describeToken(',')?.includes('F=1'));
    assert.ok(describeToken('?0^')?.includes('register equals 0'));
    assert.ok(describeToken('$>')?.includes('logically'));
    assert.ok(describeToken('+')?.includes('8-bit wrapping'));
    assert.ok(describeToken(';')?.includes('does not move'));
    assert.ok(describeToken('$&')?.includes('NAND'));
  });

  test('explains incomplete prefixes without declaring unknown Full source invalid', () => {
    assert.ok(describeToken('#')?.includes('must be completed'));
    assert.ok(describeToken('$')?.includes('Folded Block'));
    assert.ok(describeToken(',')?.includes('READ'));
    assert.strictEqual(describeToken('@C0'), null);
    assert.strictEqual(describeToken(';x2'), null);
  });
});
