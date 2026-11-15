/** Controlled version-only Node peer, production profile/settings API. No vendor CLI or account. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-profile-probe-'));await mkdir(join(directory,'pi'));
const old=process.env.PI_CODING_AGENT_DIR;process.env.PI_CODING_AGENT_DIR=join(directory,'pi');
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try{
 const peer=join(directory,'version.cjs');await writeFile(peer,"if(process.argv[2]!=='--version')throw Error('Version only');process.stdout.write('1.2.3\\n');");
 runtime=await createDesktopRuntime(join(directory,'state'),()=>{},{executableEnvironment:{}});
 for(const [driver,backend,verifiedVersion]of [['codex','codex','0.167.0'],['pi-rpc','pi','1.4.0']] as const){
  const profile=(await runtime.profiles.create({driver,label:'Version-only fixture',launch:{command:process.execPath,args:[peer]}},(await runtime.profiles.list()).storeVersion)).profile;
  const result=await runtime.settings.probe(backend,undefined,profile.ref);assert.equal(result.verifiedVersion,verifiedVersion);assert.equal(typeof result.verifiedVersion,'string');assert.equal(result.status,'available');assert.equal(result.version,'1.2.3');assert.equal(result.source,'profile');assert.deepEqual(result.profile,profile.ref);
 }
 console.log(JSON.stringify({pass:true,directory,verifiedVersionCompatible:true,versionOnly:true,noVendorProcess:true}));
}finally{await runtime?.close();if(old===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=old;}
