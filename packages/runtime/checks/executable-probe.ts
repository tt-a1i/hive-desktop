/** Real Hive HTTP/SQLite and real executable processes. No model session or inference. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const dir=await mkdtemp(join(tmpdir(),'hive-program-check-'));
async function fixture(name:string,body:string){
 const script=join(dir,name+'.cjs');await writeFile(script,'#!/usr/bin/env node\n'+body,{mode:0o700});
 if(process.platform!=='win32')return script;
 const shim=join(dir,name+'.cmd');await writeFile(shim,`@echo off\r\n"${process.execPath}" "${script}" %*\r\n`);return shim;
}
const marker=join(dir,'arguments.json');
const normal=await fixture('program ; literal',`require('node:fs').writeFileSync(${JSON.stringify(marker)},JSON.stringify(process.argv.slice(2)));console.log('codex-cli 0.166.0');`);
const fail=await fixture('failure',"console.error('Unrecognizable sensitive fixture output');process.exit(9);");
const bad=await fixture('bad version',"console.log('Unknown leading text 2.3.4');");
const stderr=await fixture('stderr version',"console.error('2.1.317 (Claude Code)');");
const pidFile=join(dir,'running.pid');
const slow=await fixture('slow',`require('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(process.pid));setInterval(()=>{},1000);`);
const flood=await fixture('flood',`require('node:fs').writeFileSync(${JSON.stringify(pidFile)},String(process.pid));process.stdout.write('x'.repeat(65537));setInterval(()=>{},1000);`);
const options={executableEnvironment:{}};let runtime=await createDesktopRuntime(join(dir,'desktop'),()=>{},options);
const dead=async()=>{const pid=Number(await readFile(pidFile,'utf8'));assert.throws(()=>process.kill(pid,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');};
try{
 await runtime.settings.update({piCommand:null,codexCommand:normal,claudeCommand:null});
 const before=await runtime.settings.get();
 const available=await runtime.settings.probe('codex',normal);assert.equal(available.status,'available');assert.equal(available.version,'0.166.0');assert.equal(available.command,normal);assert.equal(available.source,'app');assert.deepEqual(JSON.parse(await readFile(marker,'utf8')),['--version']);
 assert.equal(available.verifiedVersion,'0.167.0');
 assert.equal((await runtime.settings.probe('claude',stderr)).version,'2.1.317');
 assert.equal((await runtime.settings.probe('pi',join(dir,'missing program'))).status,process.platform==='win32'?'failed':'not_found');
 const failed=await runtime.settings.probe('pi',fail);assert.equal(failed.status,'failed');assert.ok(!JSON.stringify(failed).includes('sensitive'));const invalid=await runtime.settings.probe('pi',bad);assert.equal(invalid.status,'failed');assert.equal(invalid.version,undefined);assert.equal(invalid.source,'draft');
 for(const value of [undefined,{},true,'\ncommand'])await assert.rejects(runtime.settings.probe('pi',value));
 await assert.rejects(runtime.settings.probe('unknown',normal));
 assert.deepEqual(await runtime.settings.get(),before,'Draft checks must never save paths or change launch selection');
 const timed=await runtime.settings.probe('pi',slow);assert.equal(timed.status,'timeout');await dead();
 const overflow=await runtime.settings.probe('pi',flood);assert.equal(overflow.status,'failed');await dead();
 const pending=runtime.settings.probe('pi',slow);
 const deadline=Date.now()+5000;let ready=false;
 while(Date.now()<deadline){try{const pid=Number(await readFile(pidFile,'utf8'));process.kill(pid,0);ready=true;break;}catch{}await delay(10);}
 assert.ok(ready);await runtime.close();assert.equal((await pending).status,'cancelled');await dead();
 runtime=await createDesktopRuntime(join(dir,'environment'),()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:normal}});
 const overridden=await runtime.settings.probe('codex',join(dir,'missing draft'));assert.equal(overridden.command,normal);assert.equal(overridden.source,'env');assert.equal(overridden.version,'0.166.0');
 await runtime.close();runtime=await createDesktopRuntime(join(dir,'native'),()=>{},options);
 const native:Record<string,unknown>={};
 for(const [backend,key]of [['codex','HIVE_CODEX_COMMAND'],['pi','HIVE_PI_COMMAND'],['claude','HIVE_CLAUDE_COMMAND']]as const){
  const command=process.env[key];if(!command)continue;
  const result=await runtime.settings.probe(backend,command);assert.equal(result.status,'available');native[backend]={command:result.command,version:result.version,onlyVersion:true};
 }
 await mkdir(resolve('../../.artifacts/program-check'),{recursive:true});await writeFile(resolve('../../.artifacts/program-check/core-receipt.json'),JSON.stringify({pass:true,dir,native,modelInference:false},null,2));
 console.log('PROGRAM_CHECK_OK: real settings store/processes, unsaved input, env precedence, literal path/argv, errors, output bound, timeout termination and shutdown cancellation; no model.');
}finally{await runtime.close();}
