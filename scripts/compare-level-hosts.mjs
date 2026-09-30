import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const root=resolve(import.meta.dirname,'..');
const manifest=JSON.parse(readFileSync(resolve(root,'fixtures/levels/conformance-v1.json'),'utf8'));
const executable=resolve(root,'target/debug',process.platform==='win32'?'codegrid.exe':'codegrid');
const results=manifest.cases.map(f=>{const args=['evaluate',resolve(root,'fixtures/levels',f.level),resolve(root,'fixtures/levels',f.program),'--mode',f.mode.toLowerCase(),'--boundary',f.boundary.toLowerCase(),'--seed',f.seed,'--custom-limit',f.custom_limit,'--limits-file',resolve(root,'fixtures/levels',manifest.profile)];const execution=spawnSync(executable,args,{encoding:'utf8',windowsHide:true});if(execution.error)throw execution.error;let envelope;try{envelope=JSON.parse(execution.stdout)}catch{throw new Error(`${f.id}: invalid CLI JSON ${execution.stderr}`)}assert.equal(envelope.status,'result',f.id);assert.equal(envelope.result.status,f.expected_status,f.id);return{id:f.id,result:envelope.result};});
const report={runtime:'native CLI subprocess',results};writeFileSync(resolve(root,'target/level-cli-report.json'),JSON.stringify(report,null,2));
const hostReports=['level-portable-report.json','level-browser-report.json'];
if(existsSync(resolve(root,'target/level-wasmtime-report.json')))hostReports.push('level-wasmtime-report.json');
for(const name of hostReports){const host=JSON.parse(readFileSync(resolve(root,'target',name),'utf8'));const actual=host.results??host.cases?.map(({id,response})=>({id,result:response.result}));assert.ok(Array.isArray(actual),`${name}: missing complete results`);assert.equal(actual.length,results.length,`${name}: regenerate report against current manifest`);for(let i=0;i<results.length;i++)assert.deepEqual(actual[i],results[i],`Complete terminal semantic parity: ${name} ${results[i].id}`);}
console.log(`PASS: ${results.length} complete semantic results match actual native CLI, browser worker, and portable Node WASM.`);
