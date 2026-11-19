/** Production Codex adapter / official Claude SDK, synthetic JSONL peers, real Hive/SQLite/spool. No inference. */
import assert from 'node:assert/strict';
import { mkdtemp,mkdir,writeFile,readFile,rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-queued-peers-'));
const keys=['CLAUDE_CONFIG_DIR','HIVE_CLAUDE_REASONING_WIRE','HIVE_CLAUDE_REASONING_CONTROL']as const;
const before=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
type Runtime=Awaited<ReturnType<typeof createDesktopRuntime>>;
let runtime:Runtime|undefined;let chatId:string|undefined;
async function rows(path:string){try{return(await readFile(path,'utf8')).trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));}catch(e){if((e as NodeJS.ErrnoException).code==='ENOENT')return[];throw e;}}
async function wait(check:()=>Promise<boolean>){const end=Date.now()+12000;while(!await check()){assert.ok(Date.now()<end,'Synthetic queue contract timed out');await delay(15);}}
try{
 for(const backend of ['codex','claude']as const){
  const project=join(directory,backend),data=join(project,'state');await mkdir(project);await writeFile(join(project,'package.json'),'{"type":"commonjs"}');
  await writeFile(join(project,'app-server'),await readFile(fileURLToPath(new URL('./fixtures/codex-queue-peer.cjs',import.meta.url))));
  process.env.CLAUDE_CONFIG_DIR=join(project,'claude');process.env.HIVE_CLAUDE_REASONING_WIRE=join(project,'claude-wire.jsonl');process.env.HIVE_CLAUDE_REASONING_CONTROL=join(project,'claude-control.json');
  const options={executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath,HIVE_CLAUDE_COMMAND:fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url))}};
  const events:any[]=[];let onEvent:((e:any)=>void)|undefined;
  const observe=(e:any)=>{events.push(e);onEvent?.(e);};
  runtime=await createDesktopRuntime(data,observe,options);
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Synthetic '+backend,autostart_orchestrator:false})as{id:string};
  const chat=await runtime.chats.create(workspace.id,backend);chatId=chat.id;
  const queryCount=async()=>backend==='codex'?(await rows(join(project,'queue-peer-queries.jsonl'))).length:(await rows(process.env.HIVE_CLAUDE_REASONING_WIRE!)).filter(r=>r.event==='query').length;
  await runtime.chats.prompt(chat.id,'Working input');await wait(async()=>await queryCount()===1&&(await runtime!.chats.open(chat.id)).busy);
  const path=join(project,'followup.txt');await writeFile(path,'Owned fixture content');const attachment=await runtime.chats.attachments.add(workspace.id,path);
  await runtime.chats.prompt(chat.id,'Queued followup',[attachment.attachment.id]);const queued=await runtime.chats.open(chat.id);
  assert.equal(await queryCount(),1);assert.equal(queued.pendingUserInputs.length,2);const followup=queued.pendingUserInputs[1]!;
  assert.equal(followup.text,'Queued followup');assert.equal(followup.processing,false);assert.equal(followup.attachments[0]!.id,attachment.attachment.id);
  await assert.rejects(runtime.chats.prompt(chat.id,'Rejected followup',['00000000-0000-4000-8000-000000000000']));assert.equal((await runtime.chats.open(chat.id)).busy,true);
  assert.ok(events.some(e=>e.type==='state'&&e.pendingUserInputs?.some((i:any)=>i.deliveryId===followup.deliveryId)));
  await runtime.chats.abort(chat.id);const paused=await runtime.chats.open(chat.id);assert.equal(paused.busy,false);assert.equal(paused.pendingUserInputs.length,1);assert.equal(paused.pendingUserInputs[0]!.deliveryId,followup.deliveryId);
  await runtime.close();runtime=await createDesktopRuntime(data,observe,options);const cold=await runtime.chats.open(chat.id);
  assert.equal(cold.busy,false);assert.equal(cold.pendingUserInputs[0]!.deliveryId,followup.deliveryId);assert.equal(cold.pendingUserInputs[0]!.attachments[0]!.id,attachment.attachment.id);assert.equal(await queryCount(),1);
  await writeFile(join(project,'queue-peer-control.json'),JSON.stringify({complete:true}));await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL!,JSON.stringify({completeQueries:true}));
  await runtime.chats.resume(chat.id);await wait(async()=>{const s=await runtime!.chats.open(chat.id);return!s.busy&&s.pending===0;});
  const done=await runtime.chats.open(chat.id);assert.deepEqual(done.pendingUserInputs,[]);assert.equal(done.completedDeliveryIds.filter(id=>id===followup.deliveryId).length,1);assert.equal(done.history.filter((m:any)=>m.presentation?.deliveryId===followup.deliveryId).length,1);
  if(backend==='codex'){
   const control=join(project,'queue-peer-control.json');
   await writeFile(control,JSON.stringify({complete:false}));
   await runtime.chats.prompt(chat.id,'Finish before the steer');
   await wait(async()=>await queryCount()===3&&(await runtime!.chats.open(chat.id)).busy);
   // Fallback would finish immediately on the old queue, allowing Stop to
   // return a failing assertion rather than leaving a held native turn.
   await writeFile(control,JSON.stringify({complete:true}));
   let stopping:ReturnType<Runtime['chats']['abort']>|undefined;
   let nextId:string|undefined;
   onEvent=e=>{
    if(e.type!=='state'||e.sessionId!==chat.id||!e.busy||e.pending!==1||e.pendingUserInputs[0]?.text!=='Steer or queue')return;
    nextId=e.pendingUserInputs[0].deliveryId;onEvent=undefined;
    // Let the queue enter the pending steer await after removing the primary.
    queueMicrotask(()=>{stopping=runtime!.chats.abort(chat.id);void stopping.catch(()=>{});});
   };
   await runtime.chats.prompt(chat.id,'Steer or queue',undefined,'steer');
   await wait(async()=>stopping!==undefined);
   await stopping;
   assert.equal((await rows(join(project,'queue-peer-steers.jsonl'))).length,1);
   assert.equal(await queryCount(),3,'Stop cannot start a new native turn after the steer declines');
   const stopped=await runtime.chats.open(chat.id);
   assert.equal(stopped.busy,false);
   assert.deepEqual(stopped.pendingUserInputs.map(item=>[item.deliveryId,item.text,item.editable]),[[nextId,'Steer or queue',true]]);
   await runtime.chats.resume(chat.id);
   await wait(async()=>{const state=await runtime!.chats.open(chat.id);return !state.busy&&state.pending===0;});
   assert.equal(await queryCount(),4);
   assert.equal((await runtime.chats.open(chat.id)).completedDeliveryIds.filter(id=>id===nextId).length,1);
   console.log('CODEX_LATE_STEER_STOP_OK: native decline, stop, retained editable input and exactly one explicit resume.');
  }
  await runtime.close();runtime=undefined;chatId=undefined;console.log('QUEUED_INPUT_PEER_OK',backend,': held native turn, serialized follow-up+file, rejection isolation, stop/cold pause and exact consumed delivery. Synthetic CLI only.');
 }
}finally{
 if(runtime){try{if(chatId)await runtime.chats.abort(chatId);}finally{await runtime.close();}}
 for(const key of keys){if(before[key]===undefined)delete process.env[key];else process.env[key]=before[key];}
 await rm(directory,{recursive:true,force:true});
}
