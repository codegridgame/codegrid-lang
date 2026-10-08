import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {memoryMaximum} from './wasm-memory.mjs';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {existsSync,readFileSync,mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve,sep,join,extname} from 'node:path';
const root=resolve(import.meta.dirname,'../../..');
const bytes=readFileSync(resolve(root,'target/level-browser-bindings/codegrid_level_wasm_browser_bg.wasm'));
assert.equal(memoryMaximum(bytes),BigInt(process.env.CODEGRID_WASM_MAX_MEMORY_BYTES??'67108864'),'encoded memory maximum');
const browser=process.env.CODEGRID_BROWSER??['C:/Program Files/Google/Chrome/Application/chrome.exe','C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
if(!browser)throw new Error('Set CODEGRID_BROWSER to an installed Chromium browser');
const mime={'.html':'text/html','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.json':'application/json'};
let resolveReport;const reportPromise=new Promise(r=>resolveReport=r);
const server=createServer((req,res)=>{if(req.url==='/report'&&req.method==='POST'){let text='';req.on('data',x=>text+=x);req.on('end',()=>{resolveReport(JSON.parse(text));res.writeHead(200).end('ok');});return;}
const path=resolve(root,'.'+new URL(req.url,'http://localhost').pathname);if(!path.startsWith(root+sep)){res.writeHead(403).end();return;}try{const data=readFileSync(path);res.writeHead(200,{'Content-Type':mime[extname(path)]??'text/plain'}).end(data);}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));const profile=mkdtempSync(join(tmpdir(),'codegrid-level-browser-'));let child,timer;
try{const url=`http://127.0.0.1:${server.address().port}/crates/codegrid-level-wasm-browser/tests/scene-browser.html`;
child=spawn(browser,[`--user-data-dir=${profile}`,'--headless','--disable-gpu','--no-first-run','--disable-background-networking',url],{windowsHide:true});let stderr='';child.stderr.on('data',x=>stderr+=x);child.stdout.resume();
const report=await Promise.race([reportPromise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Browser worker timed out: '+stderr)),45000);child.once('error',reject);})]);
assert.equal(report.passed,true,report.error);writeFileSync(resolve(root,'target/scene-browser-report.json'),JSON.stringify({runtime:browser,wasm_host:'Chromium browser module worker',artifact_sha256:createHash('sha256').update(bytes).digest('hex'),profile:JSON.parse(readFileSync(resolve(root,'examples/scene-host-v2/profile-local-v2.json'),'utf8')),...report},null,2));console.log(`PASS: actual browser module worker; ${report.results.length} API-2 scene executions and Debug event traces.`);
}finally{clearTimeout(timer);if(child){child.kill();await new Promise(r=>child.once('close',r));}await new Promise(r=>server.close(r));const target=resolve(profile);if(target.startsWith(resolve(tmpdir())+sep)&&target.includes('codegrid-level-browser-'))try{rmSync(target,{recursive:true,force:true});}catch{}}
