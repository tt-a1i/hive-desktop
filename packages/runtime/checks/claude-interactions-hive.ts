/** Official Claude SDK + synthetic CLI + Hive runtime/HTTP/SQLite. No model inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, readdir } from 'node:fs/promises';
import { join,resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
const dir=await mkdtemp(join(tmpdir(),'hive-claude-interactions-'));const cwd=join(dir,'project');await mkdir(cwd);
const peer=resolve('checks/fixtures/claude-interactions-peer.cjs');
process.env.CLAUDE_CONFIG_DIR=join(dir,'claude-config');process.env.HIVE_CLAUDE_FIXTURE_DIR=dir;process.env.HIVE_CLAUDE_FIXTURE_CONTROL=join(dir,'control.json');process.env.HIVE_CLAUDE_FIXTURE_RECEIPT=join(dir,'wire.jsonl');
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
const events:unknown[]=[];const runtime=await createDesktopRuntime(join(dir,'state'),event=>events.push(event),{executableEnvironment:{}});
const wait=async<T>(fn:()=>Promise<T>|T,ready:(v:T)=>boolean)=>{const until=Date.now()+10000;for(;;){const v=await fn();if(ready(v))return v;if(Date.now()>until)throw Error('timeout');await new Promise(r=>setTimeout(r,20));}};
try{
 await runtime.settings.update({piCommand:null,codexCommand:null,claudeCommand:peer});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:cwd,name:'Synthetic Claude',autostart_orchestrator:false}) as {id:string};
 const chat=await runtime.chats.create(ws.id,'claude');
 const live=(runtime.chats as unknown as {live:Map<string,{handle:{prompt(text:string):Promise<void>;processId:number;interactions():unknown[];completedDeliveryIds():readonly string[]}} >}).live;
 const handle=live.get(chat.id)!.handle;const sentinel='CLAUDE-ANSWER-'+randomUUID();
 for(const scenario of ['permission','questions']){
  await writeFile(join(dir,'control.json'),JSON.stringify({scenario}));const done=handle.prompt('Synthetic '+scenario);void done.catch(()=>{});
  const snap=await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===1);const card=snap.interactions[0]!;
  assert.equal(card.backend,'claude');
  await assert.rejects(runtime.chats.respondInteraction('wrong-session',card.id,{decision:'cancel'}),(e:unknown)=>(e as {code?:string}).code==='INTERACTION_STALE');
  const response=scenario==='permission'?{decision:'accept'}:{answers:{'请选择一个选项？':['选项甲'],'请选择多个选项？':['选项一',sentinel]}};
  await runtime.chats.respondInteraction(chat.id,card.id,response);await done;
  assert.equal((await runtime.chats.open(chat.id)).interactions.length,0);
  await assert.rejects(runtime.chats.respondInteraction(chat.id,card.id,response),(e:unknown)=>(e as {code?:string}).code==='INTERACTION_STALE');
 }
 const wire=(await readFile(join(dir,'wire.jsonl'),'utf8')).trim().split('\n').map(x=>JSON.parse(x));const replies=wire.filter(x=>x.event==='answer');
 assert.equal(replies.length,2);assert.deepEqual(replies[0].response.updatedInput,{command:'echo fixture-only'});assert.equal(replies[0].response.updatedPermissions,undefined);
 assert.deepEqual(replies[1].response.updatedInput.answers,{'请选择一个选项？':'选项甲','请选择多个选项？':'选项一, '+sentinel});
 assert.equal(JSON.stringify(events).includes(sentinel),false);
 assert.equal(JSON.stringify(await runtime.chats.open(chat.id)).includes(sentinel),false);
 async function scan(path:string):Promise<void>{for(const e of await readdir(path,{withFileTypes:true})){const child=join(path,e.name);if(e.isDirectory())await scan(child);else if(e.isFile())assert.equal((await readFile(child)).includes(Buffer.from(sentinel)),false,child);}}
 await scan(join(dir,'state'));
 await writeFile(join(dir,'control.json'),JSON.stringify({scenario:'permission'}));
 const beforeCompleted=handle.completedDeliveryIds().length;
 const accepted=await runtime.chats.prompt(chat.id,'Synthetic queued delivery');assert.equal(accepted.accepted,true);
 const queued=await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===1);assert.equal(queued.busy,true);
 await runtime.chats.respondInteraction(chat.id,queued.interactions[0]!.id,{decision:'accept'});
 const finished=await wait(()=>runtime.chats.open(chat.id),s=>!s.busy&&s.pending===0&&s.interactions.length===0);
 assert.ok(finished.history.some(m=>(m as {role?:string}).role==='assistant'));assert.equal(handle.completedDeliveryIds().length,beforeCompleted+1);
 const abortBase=handle.completedDeliveryIds().length;
 await runtime.chats.prompt(chat.id,'Synthetic queued cancellation');
 const toAbort=await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===1);const staleId=toAbort.interactions[0]!.id;
 await runtime.chats.abort(chat.id);const aborted=await runtime.chats.open(chat.id);assert.equal(aborted.busy,false);assert.equal(aborted.pending,0);assert.equal(aborted.interactions.length,0);
 assert.equal(handle.completedDeliveryIds().length,abortBase);await assert.rejects(runtime.chats.respondInteraction(chat.id,staleId,{decision:'accept'}),(e:unknown)=>(e as {code?:string}).code==='INTERACTION_STALE');
 await writeFile(join(dir,'control.json'),JSON.stringify({scenario:'permission'}));const dying=handle.prompt('Synthetic death');void dying.catch(()=>{});
 await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===1);process.kill(handle.processId,'SIGKILL');await assert.rejects(dying);assert.equal(handle.interactions().length,0);
 await writeFile(join(dir,'receipt.json'),JSON.stringify({pass:true,synthetic:true,noInference:true,officialSdk:true,realHiveHttpWorkspace:true,processDeathCleared:true,queuedChatPromptCompleted:true,queuedAbortClearedWithoutCompletion:true,uniqueAnswerAbsentFromHiveStateEventsSnapshot:true,scope:'chats.prompt queued delivery and direct handle prompt; not desktop UI or response HTTP'},null,2));console.log('Evidence:',dir);
}catch(error){await writeFile(join(dir,'failure.json'),JSON.stringify({error:String(error),stack:(error as Error).stack,events},null,2));console.error('Evidence:',dir);throw error;}finally{await runtime.close();}
