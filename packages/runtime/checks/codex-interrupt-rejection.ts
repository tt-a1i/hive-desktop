import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {setTimeout as delay} from 'node:timers/promises';
import {createCodexSession} from '../src/agents/codex.js';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';
import type {PiSessionHandle} from '../src/agents/types.js';
import type {PiEvent} from '../src/agents/types.js';

// Real JSONL subprocess; synthetic rejection/terminal ordering, no vendor inference.
const directory=await mkdtemp(join(tmpdir(),'hive-codex-interrupt-rejection-'));
await writeFile(join(directory,'package.json'),'{"type":"commonjs"}');
const peer=String.raw`
const fs=require('node:fs');const{createInterface}=require('node:readline');const mode=require('node:path').basename(process.cwd());fs.writeFileSync('pid',String(process.pid));
const thread={id:'interrupt-'+mode,path:null,historyMode:'legacy',turns:[]};const send=v=>process.stdout.write(JSON.stringify(v)+'\n');const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});let held;
function started(q){const turn=thread.turns.at(-1);send({id:q.id,result:{turn}});notify('turn/started',{turn});notify('item/started',{turnId:turn.id,item:{id:'tool-'+turn.id,type:'commandExecution',status:'inProgress',command:'fixture owned operation',cwd:process.cwd(),aggregatedOutput:''}});}
(async()=>{for await(const line of createInterface({input:process.stdin})){const q=JSON.parse(line);if(!q.method)continue;const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start'||q.method==='thread/resume')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read'){answer({thread});if(held){const pending=held;held=undefined;started(pending);}}
 else if(q.method==='turn/start'){thread.turns.push({id:'turn-'+thread.turns.length,status:'inProgress',items:[]});if(mode==='early'){held=q;fs.writeFileSync('start-pending','ready');}else {started(q);if(thread.turns.length>1){const turn=thread.turns.at(-1);turn.status='completed';notify('turn/completed',{turn});}}}
 else if(q.method==='turn/interrupt'){fs.appendFileSync('interrupts',q.params.turnId+'\n');if(mode==='exit'){process.exit(7);}if(mode==='completed'){const turn=thread.turns.at(-1);turn.status='completed';notify('turn/completed',{turn});}send({id:q.id,error:{code:-32000,message:'Synthetic interrupt rejection',data:{fixture:true}}});}
}})().catch(e=>{console.error(e);process.exit(1)});
`;
async function waitFile(path:string){const deadline=Date.now()+3000;for(;;){try{return await readFile(path,'utf8');}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT'||Date.now()>deadline)throw error;await delay(10);}}}
try{
 for(const mode of ['rejected','completed','early','exit']){
  const cwd=join(directory,mode);await mkdir(cwd);await writeFile(join(cwd,'app-server'),peer);let ready!:()=>void;const started=new Promise<void>(resolve=>{ready=resolve;});const exits:Error[]=[];const events:PiEvent[]=[];
  const session=await createCodexSession({cwd,sessionDir:cwd,command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(event){events.push(event);if(event.type==='tool_start')ready();},onUnexpectedExit(error){exits.push(error);}});const pid=Number(await waitFile(join(cwd,'pid')));let settled=false;
  const prompt=session.prompt('Synthetic cancellation probe',{deliveryId:'pending-delivery'}).finally(()=>{settled=true;});void prompt.catch(()=>{});
  try{
   if(mode==='early')await waitFile(join(cwd,'start-pending'));else await started;
   const abort=session.abort();void abort.catch(()=>{});
   if(mode==='early')await session.reasoning!(); // A native read releases the held start response, after cancellation was requested.
   if(mode==='completed'){
    await abort;await assert.rejects(prompt);assert.equal(exits.length,0);assert.doesNotThrow(()=>process.kill(pid,0));await session.prompt('Next round remains usable');
   }else{
    await assert.rejects(abort,error=>error instanceof Error&&(mode==='exit'||('code' in error&&error.code===-32000)));
    assert.equal(settled,true,'A rejected interruption must settle the owned prompt before abort returns');await assert.rejects(prompt);assert.throws(()=>process.kill(pid,0));assert.equal(exits.length,1);assert.ok(events.some(event=>event.type==='settled'&&event.aborted));
   }
   assert.deepEqual(session.completedDeliveryIds(),[],'Rejected or cancelled delivery must never be acknowledged');
   assert.equal((await readFile(join(cwd,'interrupts'),'utf8')).trim().split('\n').length,1,'Concurrent cancellation paths must issue one native interrupt');
  }finally{
   if(!settled){try{process.kill(pid,'SIGTERM');}catch(error){if((error as NodeJS.ErrnoException).code!=='ESRCH')throw error;}}
   await prompt.catch(()=>{});await session.abort().catch(()=>{});await session.dispose();
  }
 }
 const cwd=join(directory,'runtime');await mkdir(cwd);await writeFile(join(cwd,'app-server'),peer);await writeFile(join(cwd,'package.json'),'{"type":"commonjs"}');let ready!:()=>void;const started=new Promise<void>(resolve=>{ready=resolve;});
 const runtime=await createDesktopRuntime(join(directory,'state'),event=>{if(event.type==='tool_start')ready();},{executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}});
 const live=()=> (runtime.chats as unknown as {live:Map<string,{queue:DeliveryQueue;handle:PiSessionHandle}>}).live;
 try{
  const workspace=await runtime.hive.request('POST','/api/workspaces',{name:'Rejected interruption',path:cwd,autostart_orchestrator:false}) as {id:string};
  const chat=await runtime.chats.create(workspace.id,'codex');await runtime.chats.prompt(chat.id,'Retain this pending delivery after cancellation failure');await started;
  const before=await runtime.chats.open(chat.id);assert.ok(before.runId);assert.equal(before.pending,1);const pending=live().get(chat.id)!.queue.pending.map(item=>item.id);const pid=Number(await waitFile(join(cwd,'pid')));
  await assert.rejects(runtime.chats.abort(chat.id));assert.equal(await runtime.agents.waitForRunExit(before.runId,3000),true);assert.equal(runtime.agents.findLiveRun(before.runId)?.status,'error');assert.equal(runtime.agents.peekAgentToken(`${workspace.id}:orchestrator`),undefined);assert.throws(()=>process.kill(pid,0));
  const reopened=await runtime.chats.open(chat.id);assert.equal(reopened.busy,false);assert.equal(reopened.pending,1);assert.equal(reopened.runId,null);assert.deepEqual(live().get(chat.id)!.queue.pending.map(item=>item.id),pending);assert.deepEqual(reopened.completedDeliveryIds,[]);assert.notEqual(Number(await readFile(join(cwd,'pid'),'utf8')),pid);
  await runtime.close();
  const cold=await createDesktopRuntime(join(directory,'state'),()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}});
  try{const restored=await cold.chats.open(chat.id);assert.equal(restored.busy,false);assert.equal(restored.pending,1);assert.equal(restored.runId,null);assert.deepEqual(restored.completedDeliveryIds,[]);const saved=(cold.chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live.get(chat.id)!.queue.pending.map(item=>item.id);assert.deepEqual(saved,pending);}finally{await cold.close();}
 }finally{await runtime.close();}
 console.log('Codex interrupt rejection: prompt settled and subprocess reaped on rejection or peer death; late terminal keeps connection; cancellation before start response; real Hive HTTP/SQLite revokes failed run/token and restores the exact pending delivery across a full runtime restart without execution. Synthetic JSONL peers, no vendor/model requests.');
}finally{await rm(directory,{recursive:true,force:true});}
