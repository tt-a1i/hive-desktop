/** Real JSONL child and Hive HTTP/SQLite; synthetic cancellation failures, no inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { createExternalPiSession } from '../src/agents/external-pi.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { ChatEvent } from '../src/chats.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-pi-abort-rejection-'));
const peer = `#!/usr/bin/env node
const fs=require('node:fs'),path=require('node:path');
const mode=path.basename(process.cwd());fs.writeFileSync('pid',String(process.pid));
if(!fs.existsSync('session.jsonl'))fs.writeFileSync('session.jsonl',JSON.stringify({type:'session',version:3,id:'owned-'+mode,timestamp:new Date().toISOString(),cwd:process.cwd()})+'\\n');
const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');
require('node:readline').createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line),answer=data=>send({type:'response',id:q.id,command:q.type,success:true,data});
 const start=()=>{send({type:'agent_start'});send({type:'tool_execution_start',toolCallId:'owned-operation',toolName:'fixture',args:{}});};
 if(q.type==='get_state')answer({sessionId:'owned-'+mode,sessionFile:path.join(process.cwd(),'session.jsonl'),isStreaming:false,model:{provider:'openai',id:'gpt-6.1-terra',name:'Terra'}});
 else if(q.type==='get_commands')answer({commands:[{name:process.env.HIVE_PI_BRIDGE_COMMAND,source:'extension',sourceInfo:{path:process.argv[process.argv.indexOf('--extension')+1]}}]});
 else if(q.type==='get_available_models')answer({models:[{provider:'openai',id:'gpt-6.1-terra',name:'Terra'}]});
 else if(q.type==='get_entries')answer({entries:[],leafId:null});
 else if(q.type==='prompt'){
  if(q.message.startsWith('/'+process.env.HIVE_PI_BRIDGE_COMMAND+' ')){
   const action=JSON.parse(q.message.slice(q.message.indexOf(' ')+1)).action;
   answer({disposition:'handled'});if(action==='delivery')start();
  }else{answer({disposition:'prompted'});start();}
 }else if(q.type==='clear_queue'||q.type==='abort'){
  fs.appendFileSync('cancellations',q.type+'\\n');
  if(mode==='exit'&&q.type==='abort')process.exit(7);
  if((mode.endsWith('clear-rejected')&&q.type==='clear_queue')||(mode.endsWith('abort-rejected')&&q.type==='abort'))send({type:'response',id:q.id,command:q.type,success:false,error:'Owned rejection'});
  else {answer({});if(q.type==='abort')send({type:'agent_settled',aborted:true});}
 }else answer({});
});
`;
const limit = async <T>(task: Promise<T>) => {
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{return await Promise.race([task,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error('Owned cancellation check timed out')),5000);})]);}
 finally{clearTimeout(timer);}
};
for (const mode of ['clear-rejected', 'abort-rejected', 'exit', 'success']) {
 const cwd=join(directory,mode);await mkdir(cwd);const command=join(cwd,'peer');await writeFile(command,peer,{mode:0o700});
 let started!:()=>void;const ready=new Promise<void>(resolve=>started=resolve);let settled=false;const exits:Error[]=[];
 const session=await createExternalPiSession({cwd,sessionDir:cwd,command,onEvent(event){if(event.type==='tool_start')started();},onUnexpectedExit(error){exits.push(error);}});
 const pid=Number(await readFile(join(cwd,'pid'),'utf8'));
 const prompt=session.prompt('Owned pending delivery',{deliveryId:'retain-this-delivery'}).finally(()=>{settled=true;});void prompt.catch(()=>{});
 try {
  await limit(ready);
  if(mode==='success')await limit(session.abort());else await assert.rejects(limit(session.abort()));
  assert.equal(settled,true,'Rejected cancellation must settle the owned prompt before returning');
  await assert.rejects(prompt);
  if(mode==='success')assert.doesNotThrow(()=>process.kill(pid,0));else assert.throws(()=>process.kill(pid,0));
  assert.equal(exits.length,mode==='success'?0:1);
  assert.deepEqual(session.completedDeliveryIds(),[]);
 } finally {
  if(!settled){try{process.kill(pid,'SIGTERM');}catch(error){if((error as NodeJS.ErrnoException).code!=='ESRCH')throw error;}}
  await prompt.catch(()=>{});await session.dispose();
 }
}
const snapshots=[];
for(const mode of ['clear-rejected','abort-rejected']){
 const cwd=join(directory,'runtime-'+mode);await mkdir(cwd);const command=join(cwd,'peer');await writeFile(command,peer,{mode:0o700});
 let started!:()=>void;const ready=new Promise<void>(resolve=>started=resolve);
 const state=join(cwd,'state'),options={executableEnvironment:{HIVE_PI_COMMAND:command}};
 const live=(runtime:Awaited<ReturnType<typeof createDesktopRuntime>>)=>(runtime.chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live;
 const events:ChatEvent[]=[];
 const runtime=await createDesktopRuntime(state,event=>{events.push(event);if(event.type==='tool_start')started();},options);
 let pending:string[]=[];let chatId='';let runId='';
 try{
  const ws=await runtime.hive.request('POST','/api/workspaces',{path:cwd,name:mode,autostart_orchestrator:false}) as {id:string};
  const chat=await runtime.chats.create(ws.id,'pi-rpc');chatId=chat.id;
  await runtime.chats.prompt(chat.id,'Preserve this exact pending input');await limit(ready);
  const before=await runtime.chats.open(chat.id);assert.ok(before.runId);runId=before.runId;assert.equal(before.pending,1);
  pending=live(runtime).get(chat.id)!.queue.pending.map(item=>item.id);
  const pid=Number(await readFile(join(cwd,'pid'),'utf8'));
  let abortError:Error|undefined;
  await assert.rejects(limit(runtime.chats.abort(chat.id)),error=>{assert.ok(error instanceof Error);abortError=error;return true;});
  assert.equal(await runtime.agents.waitForRunExit(runId,3000),true);
  assert.equal(runtime.agents.findLiveRun(runId)?.status,'error');
  assert.equal(runtime.agents.peekAgentToken(`${ws.id}:orchestrator`),undefined);
  assert.throws(()=>process.kill(pid,0));
  const reopened=await runtime.chats.open(chat.id);
  assert.equal(reopened.runId,null);assert.equal(reopened.busy,false);assert.equal(reopened.pending,1);
  assert.deepEqual(live(runtime).get(chat.id)!.queue.pending.map(item=>item.id),pending);
  assert.deepEqual(reopened.completedDeliveryIds,[]);
  assert.ok(events.some(event=>event.type==='error'&&event.message===abortError!.message),'Cleanup must surface the original cancellation failure');
 }finally{await runtime.close();}
 const cold=await createDesktopRuntime(state,()=>{},options);
 try{
  const restored=await cold.chats.open(chatId);assert.equal(restored.pending,1);assert.equal(restored.busy,false);assert.equal(restored.runId,null);
  assert.deepEqual(live(cold).get(chatId)!.queue.pending.map(item=>item.id),pending);assert.deepEqual(restored.completedDeliveryIds,[]);
  assert.equal(live(cold).get(chatId)!.queue.pending[0]!.presentation?.displayText,'Preserve this exact pending input');
 }finally{await cold.close();}
 const db=new DatabaseSync(join(state,'hive/runtime.sqlite'),{readOnly:true});
 try{
  assert.deepEqual(db.prepare('select type,text from messages').all().map(row=>({...row})),[{type:'user_input',text:'Preserve this exact pending input'}]);
  assert.deepEqual(db.prepare('select status from agent_runs').all().map(row=>row.status),['error']);
  snapshots.push({mode,chatId,runId,pending,restoredPending:1,runFailed:true,tokenRevoked:true,durableOriginalUserInput:true});
 }finally{db.close();}
}
await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,noInference:true,snapshots},null,2));
console.log(JSON.stringify({pass:true,directory,noInference:true,rejectedCommands:['clear_queue','abort']}));
