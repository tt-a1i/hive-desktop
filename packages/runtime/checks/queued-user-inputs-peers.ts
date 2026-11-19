/** Production Codex adapter / official Claude SDK, synthetic JSONL peers, real Hive/SQLite/spool. No inference. */
import assert from 'node:assert/strict';
import { mkdtemp,mkdir,writeFile,readFile,rename,rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
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
   // A current-turn invalid-request reply proves non-delivery. Observe the
   // actual final event before opening again, so a cold read cannot mask it.
   await writeFile(control,JSON.stringify({complete:false,rejectSteerWhileActive:true,rejectSteerCode:-32600}));
   await runtime.chats.prompt(chat.id,'Active before definite decline');
   await wait(async()=>await queryCount()===5&&(await runtime!.chats.open(chat.id)).busy);
   await runtime.chats.prompt(chat.id,'Declined original',[attachment.attachment.id],'steer');
   await wait(async()=>!!await readFile(join(project,'queue-peer-rejected-steer.json'),'utf8').catch(error=>{if(error.code==='ENOENT')return '';throw error;}));
   const beforeStop=await runtime.chats.open(chat.id);
   const declinedId=beforeStop.pendingUserInputs.find(item=>item.text==='Declined original')!.deliveryId;
   await runtime.chats.abort(chat.id);
   await wait(async()=>events.findLast(e=>e.type==='state'&&e.sessionId===chat.id)?.pendingUserInputs?.find((item:any)=>item.deliveryId===declinedId)?.editable===true);
   const last=events.findLast(e=>e.type==='state'&&e.sessionId===chat.id);
   assert.equal(last.busy,false);assert.ok(last.sequence>beforeStop.eventSequence);
   assert.equal(await queryCount(),5);
   const warm=await runtime.chats.open(chat.id);assert.equal(warm.pendingUserInputs[0]!.editable,true);
   assert.equal(warm.pendingUserInputs[0]!.deliveryId,declinedId);
   await runtime.chats.mutateQueuedInput(chat.id,declinedId,'Declined original','Declined edited');
   await runtime.close();runtime=await createDesktopRuntime(data,observe,options);
   const fresh=await runtime.chats.open(chat.id);assert.equal(fresh.pendingUserInputs[0]!.deliveryId,declinedId);
   assert.equal(fresh.pendingUserInputs[0]!.text,'Declined edited');assert.equal(fresh.pendingUserInputs[0]!.attachments[0]!.id,attachment.attachment.id);
   assert.equal(await queryCount(),5);
   await writeFile(control,JSON.stringify({complete:true}));await runtime.chats.resume(chat.id);
   await wait(async()=>{const state=await runtime!.chats.open(chat.id);return !state.busy&&state.pending===0;});
   assert.equal(await queryCount(),6);assert.equal((await runtime.chats.open(chat.id)).completedDeliveryIds.filter(id=>id===declinedId).length,1);
   console.log('CODEX_DEFINITE_DECLINE_STOP_OK: final event/cold state agree, original attachment and ID retained, edited continuation consumed once.');
   // Internal error is not a negative delivery receipt. Keep it protected,
   // then explicitly discard only its local wait through the shared review API.
   await rm(join(project,'queue-peer-rejected-steer.json'),{force:true});
   await writeFile(control,JSON.stringify({complete:false,rejectSteerWhileActive:true,rejectSteerCode:-32603}));
   await runtime.chats.prompt(chat.id,'Primary before uncertain append');
   await wait(async()=>await queryCount()===7&&(await runtime!.chats.open(chat.id)).busy);
   await runtime.chats.prompt(chat.id,'Uncertain original',[attachment.attachment.id],'steer');
   await wait(async()=>!!await readFile(join(project,'queue-peer-rejected-steer.json'),'utf8').catch(error=>{if(error.code==='ENOENT')return '';throw error;}));
   await runtime.chats.prompt(chat.id,'Unattempted tail',[attachment.attachment.id]);
   await runtime.chats.abort(chat.id);
   const uncertain=await runtime.chats.open(chat.id),original=uncertain.pendingUserInputs.find(item=>item.text==='Uncertain original')!,tail=uncertain.pendingUserInputs.find(item=>item.text==='Unattempted tail')!;
   assert.equal(original.editable,false);assert.equal(tail.editable,true);
   const review=await runtime.chats.uncertainDeliveries(chat.id),row=review.deliveries[0]!;
   assert.equal(review.deliveries.length,1);assert.equal(row.deliveryId,original.deliveryId);assert.equal(row.text,original.text);
   const profile=review.profile!,native=review.nativeSessionId!;
   const discard=(text=original.text,p:unknown=profile,n=native,v:unknown='version' in row?row.version:undefined,id=original.deliveryId)=>runtime!.chats.discardUncertainDelivery(chat.id,id,text,p,n,v);
   await assert.rejects(discard('changed'),/matching|changed/);
   await assert.rejects(discard(original.text,{...profile,revision:profile.revision+1}),/identity changed/);
   await assert.rejects(discard(original.text,profile,'wrong-native'),/identity changed/);
   await assert.rejects(discard(original.text,profile,native,'stale'),/identity changed/);
   await assert.rejects(discard(tail.text,profile,native,'stale',tail.deliveryId),/matching/);
   const nativeBefore=await readFile(join(project,'queue-peer-thread.json'));
   const stored=(runtime.chats as any).live.get(chat.id).queue.pending.find((item:any)=>item.id===original.deliveryId);
   const sourceBefore=runtime.hive.getUserInput(workspace.id,uncertain.agentId!,stored.presentation.messageSequence);
   const filename=createHash('sha256').update(chat.id).digest('hex')+'.json';
   const receiptPath=join(data,'chats','steer-dispositions',filename);
   await mkdir(receiptPath);
   await assert.rejects(discard(),/EISDIR|EPERM|EACCES/);
   assert.deepEqual((await runtime.chats.open(chat.id)).pendingUserInputs.map(item=>item.deliveryId),[original.deliveryId,tail.deliveryId],'A failed decision write cannot change the waiting queue');
   await rm(receiptPath,{recursive:true});
   const spool=join(data,'chats','deliveries',filename),backup=spool+'.saved';
   await rename(spool,backup);await mkdir(spool);
   try{await assert.rejects(discard(),/EISDIR|EPERM|EACCES/);}finally{await rm(spool,{recursive:true});await rename(backup,spool);}
   assert.equal(JSON.parse(await readFile(receiptPath,'utf8'))[0].deliveryId,original.deliveryId,'The decision survives a failed spool removal');
   await runtime.close();runtime=await createDesktopRuntime(data,observe,options);
   // chat.open is intentionally not serialized by withWorkspace. Hold the
   // profile await, let it acquire a live owner, then reject the stale reviewer.
   const profiles=runtime.chats.profiles,getRef=profiles.getRef.bind(profiles);
   let entered!:()=>void,release!:()=>void;
   const enteredGate=new Promise<void>(resolve=>entered=resolve),releaseGate=new Promise<void>(resolve=>release=resolve);
   let firstReview=true;
   profiles.getRef=async(...args:Parameters<typeof getRef>)=>{if(firstReview){firstReview=false;entered();await releaseGate;}return getRef(...args);};
   const overlapping=runtime.chats.uncertainDeliveries(chat.id);
   await enteredGate;
   try{await runtime.chats.open(chat.id);}finally{release();profiles.getRef=getRef;}
   await assert.rejects(overlapping,/ownership changed/);
   const recovered=await runtime.chats.uncertainDeliveries(chat.id);
   assert.equal(recovered.deliveries.length,0,'Cold review reconciles the committed decision without executing it');
   const decision=await discard();assert.equal(decision.nativeOutcome,'unknown');assert.equal(decision.alreadyDiscarded,true);
   assert.equal((await discard()).alreadyDiscarded,true,'A lost reply cannot discard a later input');
   assert.deepEqual(await readFile(join(project,'queue-peer-thread.json')),nativeBefore,'Local disposal must not edit native history');
   assert.deepEqual(runtime.hive.getUserInput(workspace.id,uncertain.agentId!,stored.presentation.messageSequence),sourceBefore,'Hive source/audit remains unchanged');
   assert.equal(await queryCount(),7);
   assert.deepEqual((await runtime.chats.open(chat.id)).pendingUserInputs.map(item=>item.deliveryId),[tail.deliveryId]);
   await runtime.close();runtime=await createDesktopRuntime(data,observe,options);
   const decisions=await runtime.chats.uncertainDeliveries(chat.id);
   assert.equal(decisions.deliveries.length,0);assert.ok('resolved' in decisions&&decisions.resolved.some((item:any)=>item.deliveryId===original.deliveryId&&item.text===original.text));
   assert.deepEqual((await runtime.chats.open(chat.id)).pendingUserInputs.map(item=>item.deliveryId),[tail.deliveryId]);
   assert.equal(await queryCount(),7);
   await writeFile(control,JSON.stringify({complete:true}));await runtime.chats.resume(chat.id);
   await wait(async()=>{const state=await runtime!.chats.open(chat.id);return !state.busy&&state.pending===0;});
   assert.equal(await queryCount(),8);assert.equal((await runtime.chats.open(chat.id)).completedDeliveryIds.filter(id=>id===tail.deliveryId).length,1);
   console.log('CODEX_UNCERTAIN_LOCAL_DISCARD_OK: scoped text/version/identity checks, durable idempotent receipt, native/audit unchanged, cold tail order and no original resend.');
  }
  await runtime.close();runtime=undefined;chatId=undefined;console.log('QUEUED_INPUT_PEER_OK',backend,': held native turn, serialized follow-up+file, rejection isolation, stop/cold pause and exact consumed delivery. Synthetic CLI only.');
 }
}finally{
 if(runtime){try{if(chatId)await runtime.chats.abort(chatId);}finally{await runtime.close();}}
 for(const key of keys){if(before[key]===undefined)delete process.env[key];else process.env[key]=before[key];}
 await rm(directory,{recursive:true,force:true});
}
