import * as assert from 'assert';
import { formatCodeGrid } from '../../formatter/formatter';
import { TEMPLATES } from '../../language/templates';

suite('CodeGrid formatter', () => {
  test('aligns rectangular rows without changing cells or order', () => {
    const source = '@main\n@size 3x2\n~> _ ;\n_ + .\n@end\n';
    const expected = '@main\n@size 3x2\n~> _ ;\n_  + .\n@end\n';
    const formatted = formatCodeGrid(source);
    assert.strictEqual(formatted, expected);
    assert.strictEqual(formatCodeGrid(formatted!), formatted, 'formatting must be idempotent');
  });

  test('@size between rows does not split the grid alignment group', () => {
    assert.strictEqual(
      formatCodeGrid('~> _\n@SIZE 2x2\n_ +\n'),
      '~> _\n@size 2x2\n_  +\n'
    );
  });

  test('preserves comments, indentation, and CRLF', () => {
    const source = '@MAIN\r\n~v _ // entry\r\n// keep\r\n_ +\r\n@END\r\n';
    const expected = '@main\r\n~v _ // entry\r\n// keep\r\n_  +\r\n@end\r\n';
    assert.strictEqual(formatCodeGrid(source), expected);
  });

  test('preserves missing final newline and blank-line policy', () => {
    const source = '\n~> _\n\n\n// note';
    assert.strictEqual(formatCodeGrid(source), '~> _\n\n// note');
    assert.ok(!formatCodeGrid(source)!.endsWith('\n'));
  });

  test('formats every packaged template idempotently', () => {
    for (const template of TEMPLATES) {
      const formatted = formatCodeGrid(template.body);
      assert.ok(formatted !== null, `template ${template.id} must be understood`);
      assert.strictEqual(formatCodeGrid(formatted!), formatted, `template ${template.id} must be idempotent`);
    }
  });

  test('canonicalizes @size dimensions with leading zeros', () => {
    assert.strictEqual(formatCodeGrid('@SIZE 05x2\n~> ;\n'), '@size 5x2\n~> ;\n');
    assert.strictEqual(
      formatCodeGrid('@main\n@size 05x2\n~> ;\n@end\n'),
      '@main\n@size 5x2\n~> ;\n@end\n'
    );
  });

  test('formats Full Primary and attached cell spellings as complete cells', () => {
    const source = '~> +x3 ,<* [0= ]= $&x2\n';
    const formatted = formatCodeGrid(source);
    assert.strictEqual(formatted, source);
    assert.strictEqual(formatCodeGrid(formatted!), formatted);
  });

  test('declines incomplete tokens, malformed attachments, and structural directives it cannot preserve', () => {
    const declines = [
      '~> # _\n', '~> $ _\n', '~> , _\n', '~> [\n', '~> #\n', '~> $\n',
      '~> ?x1\n', '~> #]x2\n', '~> *\n', '~> +x6\n', '~> ++\n', '~> ;x2\n',
      '~> ~x\n', '~> _x2\n', '@C0\n~> ;\n', '@F0\n~> ;\n', '@M0 +\n',
      '@size 0x2\n~> ;\n', '@size 00x2\n~> ;\n', '@size 2x2x2\n~> ;\n', '@end main extra\n', '@end main.bad\n', '@unknown\n',
      '@size 99999999999999999999999x1\n~> ;\n',
      '~> _\n_\n',
    ];
    for (const source of declines) {
      assert.strictEqual(formatCodeGrid(source), null, `must decline ${JSON.stringify(source)}`);
    }
  });

  test('handles empty source', () => {
    assert.strictEqual(formatCodeGrid(''), '');
  });

  test('aligns the reported 5x5 board with a named end and a repeat token', () => {
    const source = '@main\n@size 5x5\n~v _ _ _ _\n_ _ _ . _\n_ _ _ _ _\n_ _ _ +x3 _\n> _ _ ^ _\n@end main\n';
    const expected = '@main\n@size 5x5\n~v _ _ _   _\n_  _ _ .   _\n_  _ _ _   _\n_  _ _ +x3 _\n>  _ _ ^   _\n@end main\n';
    assert.strictEqual(formatCodeGrid(source), expected);
    assert.strictEqual(formatCodeGrid(expected), expected);
    const starts = expected.split('\n').slice(2, 7).map((row) => [...row.matchAll(/\S+/g)].map((cell) => cell.index));
    for (const row of starts) assert.deepStrictEqual(row, starts[0]);
  });

  test('preserves directive comments and named closing paths', () => {
    assert.strictEqual(formatCodeGrid('@MAIN // board\n~> _\n_ +\n@END main // close\n'),
      '@main // board\n~> _\n_  +\n@end main // close\n');
    assert.strictEqual(formatCodeGrid('@end C0.F1.M2\n'), '@end C0.F1.M2\n');
  });
});
