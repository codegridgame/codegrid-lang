// Actual portable WASM versus native CLI, without scene semantics in JS.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {runSceneConformance} from '../crates/codegrid-level-wasm-browser/tests/scene-conformance.mjs';
import {memoryMaximum} from '../crates/codegrid-level-wasm-browser/tests/wasm-memory.mjs';
const root = resolve(import.meta.dirname, '..');
const bytes = readFileSync(resolve(root, 'target/wasm32-unknown-unknown/release/codegrid_level_wasm_server.wasm'));
assert.equal(memoryMaximum(bytes), BigInt(process.env.CODEGRID_WASM_MAX_MEMORY_BYTES ?? '67108864'));
const module = await WebAssembly.compile(bytes);
assert.equal(WebAssembly.Module.imports(module).length, 0);
const guest = new WebAssembly.Instance(module).exports;
assert.equal(guest.level_abi_version(), 2);
assert.equal(guest.level_alloc(0), 0);
assert.equal(guest.level_alloc(8388609), 0);
assert.equal(guest.level_request(1, 1), 0n, 'unknown live buffer pair');
const allocator = new WebAssembly.Instance(module).exports;
const buffers = [];
for (let index = 0; index < 1024; index++) {
  const pointer = allocator.level_alloc(1);
  assert.notEqual(pointer, 0);
  buffers.push(pointer);
}
assert.equal(allocator.level_alloc(1), 0, 'buffer count ceiling');
for (const pointer of buffers) {
  assert.equal(allocator.level_dealloc(pointer, 2), 0, 'exact live length');
  assert.equal(allocator.level_dealloc(pointer, 1), 1);
  assert.equal(allocator.level_dealloc(pointer, 1), 0, 'double release');
}
assert.throws(() => allocator.memory.grow(65536), RangeError, 'linked memory ceiling');
function transport(value) {
  const input = new TextEncoder().encode(JSON.stringify(value));
  const pointer = guest.level_alloc(input.length);
  assert.notEqual(pointer, 0);
  new Uint8Array(guest.memory.buffer, pointer, input.length).set(input);
  const pair = guest.level_request(pointer, input.length);
  assert.equal(guest.level_dealloc(pointer, input.length), 1);
  assert.notEqual(pair, 0n);
  const responsePointer = Number(pair >> 32n), length = Number(pair & 0xffffffffn);
  const text = new TextDecoder('utf-8', {fatal:true}).decode(new Uint8Array(guest.memory.buffer, responsePointer, length));
  assert.equal(guest.level_dealloc(responsePointer, length), 1);
  return text;
}
const profile = readFileSync(resolve(root, 'examples/scene-host-v2/profile-local-v2.json'), 'utf8');
assert.equal(JSON.parse(transport({abi_version:2, api_version:2, operation:'initialize', profile_json:profile})).status, 'ok');
const manifest = JSON.parse(readFileSync(resolve(root, 'fixtures/scene-v2/conformance-v2.json'), 'utf8'));
const cases = manifest.cases.map(f => ({...f, level_json:readFileSync(resolve(root,'fixtures/scene-v2',f.level),'utf8'), source:readFileSync(resolve(root,'fixtures/scene-v2',f.program),'utf8')}));
const results = runSceneConformance(request_json => transport({abi_version:2,api_version:2,operation:'request',request_json}), cases, (condition,message)=>assert.ok(condition,message));
const native = cases.map(f => {
  const execution = spawnSync(resolve(root,process.platform==='win32'?'target/debug/codegrid.exe':'target/debug/codegrid'), ['evaluate',resolve(root,'fixtures/scene-v2',f.level),resolve(root,'fixtures/scene-v2',f.program),'--api-version','2','--mode',f.mode.toLowerCase(),'--boundary',f.boundary.toLowerCase(),'--seed',f.seed,'--custom-limit',f.custom_limit,'--limits-file',resolve(root,'examples/scene-host-v2/profile-local-v2.json')], {encoding:'utf8',windowsHide:true});
  assert.ok([0,8,9,10,11].includes(execution.status),execution.stderr);
  const response = JSON.parse(execution.stdout);
  assert.equal(response.status,'result');
  return {id:f.id,result:response.result};
});
for (let i=0;i<results.length;i++) assert.deepEqual(results[i].result,native[i].result,cases[i].id+': native/portable result');
for (const id of ['robot','mechanical-arm']) {
  assert.deepEqual(results.find(r=>r.id===id+'.pass').result,results.find(r=>r.id===id+'.slice').result,id+': slice result');
  assert.deepEqual(results.find(r=>r.id===id+'.pass').events,results.find(r=>r.id===id+'.slice').events,id+': slice trace');
}
const reactive = results.find(r=>r.id==='robot.read-observation');
const reactiveSlice = results.find(r=>r.id==='robot.read-observation-slice');
assert.deepEqual(reactive.result,reactiveSlice.result,'appended observation: slice result');
assert.deepEqual(reactive.events,reactiveSlice.events,'appended observation: slice events');
assert.equal(reactive.result.partial_metrics.ticks,'6');
assert.equal(reactive.result.visible_cases[0].summary.frames,'2');
assert.equal(reactive.result.visible_cases[0].summary.rounds,'1');
writeFileSync(resolve(root,'target/scene-native-report.json'),JSON.stringify({runtime:'native CLI',results:native},null,2));
writeFileSync(resolve(root,'target/scene-portable-report.json'),JSON.stringify({runtime:process.version,wasm_host:'Node WebAssembly',artifact_sha256:createHash('sha256').update(bytes).digest('hex'),results},null,2));
console.log(`PASS: ${results.length} API-2 scene results match actual native CLI and portable WASM; slice traces agree.`);
