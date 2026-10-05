import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { setTimeout } from 'node:timers/promises';

const root=resolve(import.meta.dirname,'..');
async function version(command,args) {
  for(let attempt=0;attempt<5;attempt++) {
    const result=spawnSync(command,args,{encoding:'utf8',windowsHide:true,cwd:root});
    if(!result.error && result.status===0) return result.stdout.trim();
    if(result.error?.code==='EBUSY' && attempt<4) {
      await setTimeout(250*(attempt+1));
      continue;
    }
    throw new Error(`Cannot record ${command} identity: ${result.error ?? result.stderr}`);
  }
}
function sha(path) {return createHash('sha256').update(readFileSync(resolve(root,path))).digest('hex');}
const cli=JSON.parse(readFileSync(resolve(root,'target/level-cli-report.json'),'utf8'));
const buildIds=[...new Set(cli.results.map(record=>record.result.evaluator_build))];
if(buildIds.length!==1 || !/^codegrid-level-source-sha256:[0-9a-f]{64}$/.test(buildIds[0])) throw new Error('Results lack one exact evaluator source build identity');
const report={
  schema:'codegrid.level.build-provenance',schema_version:1,
  evaluator_build:buildIds[0],api_version:1,browser_binding_version:1,portable_abi_version:1,logical_format_version:1,
  rustc:await version('rustc',['-Vv']),cargo:await version('cargo',['--version']),wasm_bindgen:await version('wasm-bindgen',['--version']),node:process.version,
  targets:{native:'host reported by rustc -Vv',wasm:'wasm32-unknown-unknown'},
  wasm_build:{profile:'release',locked_dependencies:true,maximum_memory_bytes:process.env.CODEGRID_WASM_MAX_MEMORY_BYTES ?? '67108864'},
  cargo_lock_sha256:sha('Cargo.lock'),wasmtime_cargo_lock_sha256:sha('tools/wasmtime-host/Cargo.lock'),
  artifacts:{
    portable_wasm:sha('target/wasm32-unknown-unknown/release/codegrid_level_wasm_server.wasm'),
    browser_wasm:sha('target/level-browser-bindings/codegrid_level_wasm_browser_bg.wasm'),
    browser_javascript:sha('target/level-browser-bindings/codegrid_level_wasm_browser.js'),
  },
  source_identity_algorithm:'SHA-256 over lexical slash-relative path and file bytes, each prefixed by little-endian u64 length; inputs defined by codegrid-level-api/build.rs',
  scope:'Repository-local verification; production Steam/backend embedding remains unverified',
};
writeFileSync(resolve(root,'target/level-build-provenance.json'),JSON.stringify(report,null,2));
console.log(`Recorded evaluator source and toolchain provenance: ${report.evaluator_build}`);
