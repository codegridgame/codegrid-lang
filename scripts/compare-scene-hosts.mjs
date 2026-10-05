import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
const root = resolve(import.meta.dirname, '..');
const read = name => JSON.parse(readFileSync(resolve(root,'target',name),'utf8'));
const native = read('scene-native-report.json');
const portable = read('scene-portable-report.json');
const browser = read('scene-browser-report.json');
const wasmtime = read('scene-wasmtime-report.json');
assert.equal(portable.artifact_sha256,wasmtime.artifact_sha256,'same portable artifact');
const count = JSON.parse(readFileSync(resolve(root,'fixtures/scene-v2/conformance-v2.json'),'utf8')).cases.length;
for (const [name, report] of [['native',native],['portable',portable],['browser',browser],['Wasmtime',wasmtime]]) {
  assert.equal(report.results.length,count,name+': current manifest coverage');
  for(let i=0;i<count;i++) {
    assert.equal(report.results[i].id,native.results[i].id,name+': fixture identity');
    assert.deepEqual(report.results[i].result,native.results[i].result,name+': complete result '+native.results[i].id);
    if(name!=='native') assert.deepEqual(report.results[i].events,portable.results[i].events,name+': complete Debug events '+native.results[i].id);
  }
}
console.log(`PASS: ${count} complete scene results agree across native CLI, browser Worker, Node WASM, and Wasmtime; all three WASM Debug traces agree.`);
