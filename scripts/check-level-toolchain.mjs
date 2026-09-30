import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
const root=resolve(import.meta.dirname,'..');
const pin=JSON.parse(readFileSync(resolve(root,'tools/level-build-toolchain-v1.json'),'utf8'));
for(const [command,key] of [['rustc','rustc_version'],['cargo','cargo_version'],['wasm-bindgen','wasm_bindgen_version']]) {
 const execution=spawnSync(command,['--version'],{encoding:'utf8',windowsHide:true,cwd:root});
 if(execution.error||execution.status!==0||execution.stdout.trim().split(/\s+/)[1]!==pin[key]) throw new Error(`${command} must match pinned version ${pin[key]}; received ${execution.stdout ?? execution.error}`);
}
if(process.version!==pin.node_version)throw new Error(`Node must match pinned ${pin.node_version}; received ${process.version}`);
console.log('Pinned local level toolchain matches installed tools.');
