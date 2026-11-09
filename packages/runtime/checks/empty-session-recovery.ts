// Real Hive HTTP/SQLite + Pi SDK JSONL recovery, with model queues held paused.
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {SessionManager} from '@earendil-works/pi-coding-agent';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {Chats} from '../src/chats.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';
const backend=process.argv.includes('--codex')?'codex':'pi';
const directory=await mkdtemp(join(tmpdir(),'hive-empty-recovery-'));
const queues=(chats:Chats)=>(chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 const state=join(directory,'state');
 runtime=await createDesktopRuntime(state,()=>{});
 const records=[];
 for(const name of (backend==='codex'?['empty','queued']:['empty','queued','history','delivery','extension'])) {
  const project=join(directory,name);await mkdir(project);
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:`Recovery ${name}`,autostart_orchestrator:false}) as {id:string};
  const chat=await runtime.chats.create(workspace.id,backend);
  if(backend==='codex')await runtime.chats.setModel(chat.id,'openai','gpt-6.1-terra');
  await runtime.chats.resume(chat.id);
  const initialized=await runtime.chats.open(chat.id);
  assert.ok(initialized.startupContext);if(backend==='pi')assert.ok(initialized.sessionFile);
  const queue=queues(runtime.chats).get(chat.id)!.queue;
  await queue.pause();
  if(name==='queued')await queue.enqueue('Retained real user task',true,{version:1,origin:'user',displayText:'Retained real user task'});
  const run=runtime.agents.getActiveRunByAgentId(workspace.id,`${workspace.id}:orchestrator`)!;
  runtime.agents.stopAgentRun(run.runId);
  assert.equal(await runtime.agents.waitForRunExit(run.runId,5000),true);
  records.push({name,id:chat.id,workspaceId:workspace.id,path:initialized.sessionFile,initialContext:initialized.startupContext});
 }
 await runtime.close();runtime=undefined;
 // Native persisted user history represents an existing unfinished conversation.
 const history=records.find(x=>x.name==='history');
 if(history)SessionManager.open(history.path!).appendMessage({role:'user',content:'Earlier unfinished task',timestamp:Date.now()});
 for(const name of (backend==='pi'?['delivery','extension']:[])) {
  const record=records.find(x=>x.name===name)!;
  SessionManager.open(record.path!).appendCustomMessageEntry(name==='delivery'?'hive-delivery':'third-party-context','Existing opaque work',false, name==='delivery'?{version:1,origin:'system',displayText:'Workflow finished'}:undefined);
 }
 runtime=await createDesktopRuntime(state,()=>{});
 for(const record of records) {
  await runtime.chats.open(record.id);
  const queue=queues(runtime.chats).get(record.id)!.queue;
  // Hold only execution, preserving the actual HTTP start and recovery delivery.
  queue.resume=()=>{};
  await runtime.chats.resume(record.id);
  const restored=await runtime.chats.open(record.id);
  if(record.name==='empty') {
   assert.equal(restored.pending,0,'Context-only chat must not queue a model wake-up');
   assert.ok(restored.startupContext);
   assert.notEqual(restored.startupContext,record.initialContext);
   if(backend==='pi')assert.ok(restored.history.some(message=>!!message&&typeof message==='object'&&'content' in message&&message.content===restored.startupContext));
   assert.ok(restored.history.every(message=>!!message&&typeof message==='object'&&'role' in message&&message.role==='custom'));
  } else if(record.name==='queued') {
   assert.equal(restored.pending,1);
   assert.equal(queue.pending[0]?.text,'Retained real user task');
   assert.equal(queue.pending[0]?.presentation?.origin,'user');
  } else {
   assert.equal(restored.pending,1,'Existing user history still needs the normal recovery wake-up');
   assert.equal(queue.pending[0]?.presentation?.origin,'recovery');
   if(record.name==='history')assert.ok(restored.history.some(message=>!!message&&typeof message==='object'&&'role' in message&&message.role==='user'));
   else assert.ok(restored.history.some(message=>!!message&&typeof message==='object'&&'customType' in message&&message.customType===(record.name==='delivery'?'hive-delivery':'third-party-context')));
  }
  const run=runtime.agents.getActiveRunByAgentId(record.workspaceId,`${record.workspaceId}:orchestrator`)!;
  runtime.agents.stopAgentRun(run.runId);assert.equal(await runtime.agents.waitForRunExit(run.runId,5000),true);
 }
 await runtime.close();runtime=undefined;
 runtime=await createDesktopRuntime(state,()=>{});
 assert.equal((await runtime.chats.open(records[0]!.id)).pending,0);
 assert.equal((await runtime.chats.open(records[1]!.id)).pending,1);
 for(const record of records.slice(2))assert.equal((await runtime.chats.open(record.id)).pending,1);
 console.log(`Empty ${backend} recovery passed: context-only chat stays idle; queued user input preserved; cold spools verified.${backend==='pi'?' Native user/system delivery/unknown extension history still wakes.':''} No model requests.`);
}finally{await runtime?.close();await rm(directory,{recursive:true,force:true});}
