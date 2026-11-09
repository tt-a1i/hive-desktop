// Real Hive HTTP/SQLite + Pi SDK native history. Queues held paused: no model calls.
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {SessionManager} from '@earendil-works/pi-coding-agent';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {Chats} from '../src/chats.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';
const directory=await mkdtemp(join(tmpdir(),'hive-input-recovery-'));
const agent=join(directory,'agent');await mkdir(agent);await writeFile(join(agent,'settings.json'),JSON.stringify({defaultProvider:'openai',defaultModel:'gpt-6.1-terra'}));process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
const queues=(chats:Chats)=>(chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try{
 const state=join(directory,'state');runtime=await createDesktopRuntime(state,()=>{});const records=[];
 for(const name of ['manual-resume','explicit-input','failed-start']){
  const project=join(directory,name);await mkdir(project);
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name,autostart_orchestrator:false}) as {id:string};
  const chat=await runtime.chats.create(workspace.id);await runtime.chats.resume(chat.id);const initial=await runtime.chats.open(chat.id);assert.ok(initial.sessionFile);
  const run=runtime.agents.getActiveRunByAgentId(workspace.id,`${workspace.id}:orchestrator`)!;runtime.agents.stopAgentRun(run.runId);assert.equal(await runtime.agents.waitForRunExit(run.runId,5000),true);
  records.push({name,id:chat.id,workspaceId:workspace.id,path:initial.sessionFile});
 }
 await runtime.close();runtime=undefined;
 for(const record of records)SessionManager.open(record.path!).appendMessage({role:'user',content:'Prior unfinished conversation',timestamp:Date.now()});
 runtime=await createDesktopRuntime(state,()=>{});
 for(const record of records){
  await runtime.chats.open(record.id);const queue=queues(runtime.chats).get(record.id)!.queue;queue.resume=()=>{};
  if(record.name==='failed-start'){
   const db=new DatabaseSync(join(state,'hive/runtime.sqlite'));
   try{
    db.exec("CREATE TRIGGER owned_reject_start BEFORE INSERT ON agent_runs BEGIN SELECT RAISE(ABORT, 'owned start fault'); END");
    await assert.rejects(runtime.chats.prompt(record.id,'Must not be accepted on failed start'),Error);
    assert.equal(queue.pendingCount,0);
   }finally{db.exec('DROP TRIGGER IF EXISTS owned_reject_start');db.close();}
  }
  if(record.name==='explicit-input'){
   await runtime.chats.prompt(record.id,'Fresh user instruction');
   assert.equal(queue.pendingCount,1,'Explicit cold input should not get a separate recovery model turn');
   assert.equal(queue.pending[0]?.presentation?.origin,'user');assert.equal(queue.pending[0]?.presentation?.displayText,'Fresh user instruction');
   const restored=await runtime.chats.open(record.id);assert.ok(restored.startupContext);assert.ok(restored.history.some((x:any)=>x.role==='custom'&&x.content===restored.startupContext));
  }else{
   await runtime.chats.resume(record.id);assert.equal(queue.pendingCount,1,'Manual resume still needs its normal wake-up, including after failed user activation');assert.equal(queue.pending[0]?.presentation?.origin,'recovery');
  }
  const run=runtime.agents.getActiveRunByAgentId(record.workspaceId,`${record.workspaceId}:orchestrator`)!;runtime.agents.stopAgentRun(run.runId);assert.equal(await runtime.agents.waitForRunExit(run.runId,5000),true);
 }
 await runtime.close();runtime=undefined;runtime=await createDesktopRuntime(state,()=>{});
 for(const record of records){const chat=await runtime.chats.open(record.id);assert.equal(chat.pending,1);assert.equal(queues(runtime.chats).get(record.id)!.queue.pending[0]?.presentation?.origin,record.name==='explicit-input'?'user':'recovery');}
 console.log('User-input recovery passed: explicit cold input has one durable user delivery and recovery context; manual resume/fault cleanup still wake; cold spools retained. Real Hive/SQLite/Pi, no model requests.');
}finally{await runtime?.close();await rm(directory,{recursive:true,force:true});}
