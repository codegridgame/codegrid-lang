#!/usr/bin/env node
// Verify stable identifiers against the normative documentation registry.
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..');
const registry = JSON.parse(fs.readFileSync(path.join(root, 'spec/codegrid-error-codes.json'), 'utf8'));
const scoped = new Set();
for (const entry of registry.entries) {
  const key = `${entry.layer}:${entry.code}`;
  if (scoped.has(key)) throw new Error(`Duplicate identifier: ${key}`);
  if (!entry.condition || !entry.surface || !entry.state_effect) throw new Error(`Incomplete entry: ${key}`);
  scoped.add(key);
}
const known = new Set(registry.entries.map((entry) => String(entry.code)));
let checked = 0;
function visit(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      if (!['node_modules', 'target', 'out', 'runtime', '.vscode-test', 'test', 'tests'].includes(entry.name)) visit(file);
    } else if (/\.(rs|ts)$/.test(entry.name)) {
      let source = fs.readFileSync(file, 'utf8');
      const tests = source.lastIndexOf('#[cfg(test)]\nmod tests');
      if (tests >= 0) source = source.slice(0, tests);
      for (const pattern of [/["'`\[]((?:source|ir|cli|debug|editor|browser|lsp|level|level_api|level_abi)\.[a-z_]+)["'`\]]/g, /\b(?:input_error_json|error_response)\(\s*"([a-z_]+)"/g, /\.ok_or\(\(\s*"([a-z_]+)"/g]) {
        for (const match of source.matchAll(pattern)) {
          if (!known.has(match[1])) throw new Error(`Unregistered code ${match[1]} in ${path.relative(root, file)}`);
          checked++;
        }
      }
    }
  }
}
visit(path.join(root, 'crates'));
visit(path.join(root, 'editors/vscode/src'));
const markdown = fs.readFileSync(path.join(root, 'spec/codegrid-error-codes.md'), 'utf8');
for (const entry of registry.entries) if (!markdown.includes('`' + entry.code + '`')) throw new Error(`Missing specification row: ${entry.layer}:${entry.code}`);
console.log(`Error registry valid: ${registry.entries.length} scoped identifiers, ${checked} emitted references checked.`);
