/** JSONL fixtures, no vendor requests. */
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {parseCodexLimits, probeCodexLimits} from '../src/codex-limits.js';
const date='2026-11-15T00:00:00.000Z';
const valid={limitId:'codex',primary:{usedPercent:22,windowDurationMins:300,resetsAt:1800000000},secondary:null};
assert.deepEqual(parseCodexLimits({rateLimitsByLimitId:{codex:valid},rateLimits:{limitId:'other',primary:{usedPercent:90}}},date),{status:'ok',checkedAt:date,windows:[{position:'primary',usedPercent:22,windowDurationMins:300,resetsAt:'2027-01-15T08:00:00.000Z'}]});
for(const value of [{rateLimits:{limitId:'other',primary:{usedPercent:10}}},{rateLimitsByLimitId:{codex:null},rateLimits:valid},{rateLimits:{limitId:'codex',primary:{usedPercent:-1}}},{rateLimits:{limitId:'codex',primary:{usedPercent:101}}},{rateLimits:{limitId:'codex',primary:null}}])assert.equal(parseCodexLimits(value,date).status,'unavailable');
assert.equal(parseCodexLimits({rateLimits:{limitId:'codex',primary:{usedPercent:NaN}}},date).status,'unavailable');
assert.deepEqual(parseCodexLimits({rateLimits:{limitId:'codex',primary:{usedPercent:1,windowDurationMins:-2,resetsAt:NaN}}},date).windows[0],{position:'primary',usedPercent:1,windowDurationMins:null,resetsAt:null});
const dir=await mkdtemp(join(tmpdir(),'hive-codex-limits-'));
try {
 async function peer(name:string, mode:string, account:unknown={requiresOpenaiAuth:true,account:{type:'chatgpt',email:'SECRET_FIXTURE'}}) {
  const path=join(dir,`${name}.cjs`), log=join(dir,`${name}.log`), pid=join(dir,`${name}.pid`);
  await writeFile(path,`#!/usr/bin/env node\nconst fs=require('fs');fs.writeFileSync(${JSON.stringify(pid)},String(process.pid));require('readline').createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);fs.appendFileSync(${JSON.stringify(log)},JSON.stringify(x)+'\\n');if(x.method==='initialize')console.log(JSON.stringify({id:x.id,result:{userAgent:'fixture'}}));if(x.method==='account/read')console.log(JSON.stringify({id:x.id,result:${JSON.stringify(account)}}));if(x.method==='account/rateLimits/read'){if(${JSON.stringify(mode)}==='drop')process.exit(2);if(${JSON.stringify(mode)}==='normal')console.log(JSON.stringify({id:x.id,result:${JSON.stringify({rateLimits:valid,accountId:'PRIVATE_ACCOUNT',rateLimitResetCredits:{secret:'PRIVATE_CREDIT'}})}}));}});`,{mode:0o700});
  let command=path;if(process.platform==='win32'){command=join(dir,`${name}.cmd`);await writeFile(command,`@echo off\r\n"${process.execPath}" "${path}" %*\r\n`)}return {command,log,pid};
 }
 const normal=await peer('normal','normal'); const result=await probeCodexLimits(normal.command,new AbortController().signal);
 assert.equal(result.status,'ok');assert(!JSON.stringify(result).includes('PRIVATE'));assert(!JSON.stringify(result).includes('SECRET'));
 const requests=(await readFile(normal.log,'utf8')).trim().split('\n').map(x=>JSON.parse(x));
 assert.deepEqual(requests.map(x=>x.method),['initialize','initialized','account/read','account/rateLimits/read']);assert.deepEqual(requests[2].params,{refreshToken:false});assert.deepEqual(requests[3].params,{excludeResetCreditDetails:true});
 const pid=Number(await readFile(normal.pid,'utf8'));assert.throws(()=>process.kill(pid,0),(e:NodeJS.ErrnoException)=>e.code==='ESRCH');
 for (const [name,account,status] of [
  ['signedout',{requiresOpenaiAuth:true,account:null},'signed_out'],
  ['apikey',{requiresOpenaiAuth:true,account:{type:'apiKey'}},'not_applicable'],
  ['unknown',{requiresOpenaiAuth:true,account:{type:'future'}},'unavailable'],
  ['bad',{account:null},'failed'],
 ] as const) {
  const variant=await peer(name,'normal',account);
  assert.equal((await probeCodexLimits(variant.command,new AbortController().signal)).status,status);
  const methods=(await readFile(variant.log,'utf8')).trim().split('\n').map(x=>JSON.parse(x).method);
  assert.deepEqual(methods,['initialize','initialized','account/read']);
 }
 const drop=await peer('drop','drop');assert.equal((await probeCodexLimits(drop.command,new AbortController().signal)).status,'failed');
 const hold=await peer('hold','hold'),controller=new AbortController();const running=probeCodexLimits(hold.command,controller.signal);
 let ready=false;for(let i=0;i<500;i++){try{ready=(await readFile(hold.log,'utf8')).includes('account/rateLimits/read')}catch{}if(ready)break;await delay(10)}assert(ready);controller.abort();assert.equal((await running).status,'cancelled');
 const holdPid=Number(await readFile(hold.pid,'utf8'));assert.throws(()=>process.kill(holdPid,0),(e:NodeJS.ErrnoException)=>e.code==='ESRCH');
 const timeout=await peer('timeout','hold');assert.equal((await probeCodexLimits(timeout.command,new AbortController().signal)).status,'failed');
 const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
 let runtime=await createDesktopRuntime(join(dir,'state'),()=>{},{executableEnvironment:{}});
 try {
   const settings=await runtime.settings.get();
   const draft=await runtime.settings.codexLimits(normal.command);
   assert.equal(draft.status,'ok');assert.equal(draft.source,'draft');assert.deepEqual(await runtime.settings.get(),settings);
   await runtime.settings.update({...settings,codexCommand:normal.command});
   assert.equal((await runtime.settings.codexLimits(normal.command)).source,'app');
   await runtime.close();
   runtime=await createDesktopRuntime(join(dir,'env'),()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:normal.command}});
   assert.equal((await runtime.settings.codexLimits(join(dir,'wrong'))).source,'env');
   await runtime.close();
   runtime=await createDesktopRuntime(join(dir,'shutdown'),()=>{},{executableEnvironment:{}});
   const shutdown=await peer('shutdown','hold');const pending=runtime.settings.codexLimits(shutdown.command);
   let entered=false;for(let i=0;i<500;i++){try{entered=(await readFile(shutdown.log,'utf8')).includes('account/rateLimits/read')}catch{}if(entered)break;await delay(10)}assert(entered);
   await runtime.close();assert.equal((await pending).status,'cancelled');
   const shutdownPid=Number(await readFile(shutdown.pid,'utf8'));assert.throws(()=>process.kill(shutdownPid,0),(e:NodeJS.ErrnoException)=>e.code==='ESRCH');
 } finally {await runtime.close();}

 console.log('CODEX_LIMITS_OK: synthetic peers, whitelist, variants, exact no-refresh requests, timeout, disconnect and process cleanup.');
} finally {await rm(dir,{recursive:true,force:true});}
