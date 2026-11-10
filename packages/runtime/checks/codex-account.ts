/** Real owned JSONL subprocesses; synthetic peers are not Codex login evidence. */
import assert from 'node:assert/strict';
import {mkdtemp, writeFile, readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {probeCodexAccount} from '../src/codex-account.js';
const root = await mkdtemp(join(tmpdir(), 'hive-codex-account-'));
async function peer(name:string, response:unknown, hold=false) {
 const file=join(root,`${name}.cjs`), log=join(root,`${name}.jsonl`), pid=join(root,`${name}.pid`);
 await writeFile(file,`#!/usr/bin/env node\nconst fs=require('node:fs');fs.writeFileSync(${JSON.stringify(pid)},String(process.pid));require('node:readline').createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);fs.appendFileSync(${JSON.stringify(log)},JSON.stringify(x)+'\\n');if(x.method==='initialize')console.log(JSON.stringify({id:x.id,result:{userAgent:'fixture'}}));if(x.method==='account/read'&&!${hold})console.log(JSON.stringify({id:x.id,result:${JSON.stringify(response)}}));});`,{mode:0o700});
 let command=file;
 if(process.platform==='win32'){command=join(root,`${name}.cmd`);await writeFile(command,`@echo off\r\n"${process.execPath}" "${file}" %*\r\n`);}
 return {command,log,pid};
}
for (const [name,response,expected] of [
 ['chatgpt',{requiresOpenaiAuth:true,account:{type:'chatgpt',email:'private@example.invalid',planType:'pro'}},{status:'account_detected',accountType:'chatgpt'}],
 ['api-key',{requiresOpenaiAuth:true,account:{type:'apiKey',key:'PRIVATE_FIXTURE'}},{status:'account_detected',accountType:'apiKey'}],
 ['signed-out',{requiresOpenaiAuth:true,account:null},{status:'signed_out'}],
 ['no-auth',{requiresOpenaiAuth:false},{status:'not_required'}],
 ['future',{requiresOpenaiAuth:true,account:{type:'future'}},{status:'unknown'}],
 ['invalid',{account:null},{status:'failed'}],
] as const) {
 const test=await peer(name,response);
 const result=await probeCodexAccount(test.command,new AbortController().signal);
 assert.deepEqual(result,expected);assert.ok(!JSON.stringify(result).includes('private'));assert.ok(!JSON.stringify(result).includes('PRIVATE_FIXTURE'));
 const requests=(await readFile(test.log,'utf8')).trim().split('\n').map(x=>JSON.parse(x));
 assert.deepEqual(requests.map(x=>x.method),['initialize','initialized','account/read']);assert.deepEqual(requests[2].params,{refreshToken:false});
 const pid=Number(await readFile(test.pid,'utf8'));assert.throws(()=>process.kill(pid,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');
}
const timed=await peer('timeout',null,true);
assert.deepEqual(await probeCodexAccount(timed.command,new AbortController().signal),{status:'failed'});
const timedPid=Number(await readFile(timed.pid,'utf8'));assert.throws(()=>process.kill(timedPid,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');
const slow=await peer('cancel',null,true), abort=new AbortController();const running=probeCodexAccount(slow.command,abort.signal);
let reading=false;for(let i=0;i<500;i++){try{reading=(await readFile(slow.log,'utf8')).includes('account/read');}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;}if(reading)break;await delay(10);}
assert.ok(reading);abort.abort();assert.deepEqual(await running,{status:'cancelled'});const cancelledPid=Number(await readFile(slow.pid,'utf8'));assert.throws(()=>process.kill(cancelledPid,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');
assert.deepEqual(await probeCodexAccount(slow.command,AbortSignal.abort()),{status:'cancelled'});
assert.deepEqual(await probeCodexAccount(join(root,'missing'),new AbortController().signal),{status:'failed'});
console.log('CODEX_ACCOUNT_PEERS_OK: exact no-refresh request, account variants/redaction, malformed response, process failure, cancellation and child reaping. Synthetic peers; no inference.');

// The same check through real Hive app-state HTTP/SQLite, including env precedence
// and runtime shutdown. Only fixtures below: no inference or user auth writes.
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
const test=await peer('settings',{requiresOpenaiAuth:true,account:{type:'chatgpt',email:'private@example.invalid'}});
let runtime=await createDesktopRuntime(join(root,'data'),()=>{},{executableEnvironment:{}});
try {
 const before=await runtime.settings.get();
 const result=await runtime.settings.codexAccount(test.command);
 assert.deepEqual(result,{command:test.command,source:'draft',status:'account_detected',accountType:'chatgpt'});
 assert.deepEqual(await runtime.settings.get(),before);
 for(const invalid of [undefined,{},true,'\ncommand'])await assert.rejects(runtime.settings.codexAccount(invalid));
 await runtime.settings.update({...before,codexCommand:test.command});
 assert.equal((await runtime.settings.codexAccount(test.command)).source,'app');
 await runtime.close();
 runtime=await createDesktopRuntime(join(root,'env'),()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:test.command}});
 assert.deepEqual(await runtime.settings.codexAccount(join(root,'missing draft')),{command:test.command,source:'env',status:'account_detected',accountType:'chatgpt'});
 const native=process.env.HIVE_CODEX_COMMAND;
 if(native) {
   await runtime.close();runtime=await createDesktopRuntime(join(root,'native'),()=>{},{executableEnvironment:{}});
   const before=await runtime.settings.get();const result=await runtime.settings.codexAccount(native);
   assert.equal(result.status,'account_detected');assert.equal(result.accountType,'chatgpt');
   assert.deepEqual(await runtime.settings.get(),before);
   console.log('ORIGINAL_CODEX_ACCOUNT_READ_OK',JSON.stringify(result));
   const legacy=await runtime.settings.codexAccount('/opt/homebrew/bin/codex');assert.equal(legacy.status,'failed');
   assert.deepEqual(await runtime.settings.get(),before);console.log('LEGACY_CODEX_CONNECTION_REJECTED');
 }
 await runtime.close();
 const shutdownPeer=await peer('shutdown',null,true);
 runtime=await createDesktopRuntime(join(root,'shutdown'),()=>{},{executableEnvironment:{}});
 const running=runtime.settings.codexAccount(shutdownPeer.command);
 let reading=false;for(let i=0;i<500;i++){try{reading=(await readFile(shutdownPeer.log,'utf8')).includes('account/read');}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;}if(reading)break;await delay(10);}
 assert.ok(reading);await runtime.close();assert.equal((await running).status,'cancelled');
 const pid=Number(await readFile(shutdownPeer.pid,'utf8'));assert.throws(()=>process.kill(pid,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');
 console.log('CODEX_ACCOUNT_SETTINGS_OK: real Hive HTTP/SQLite, exact draft/app/env source, no path save, input validation and runtime-close drain.');
} finally { await runtime.close(); }
