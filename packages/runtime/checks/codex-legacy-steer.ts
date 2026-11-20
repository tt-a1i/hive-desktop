/** Historical spool replay through real Hive/SQLite and production Codex JSONL adapter. No model requests. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm,rename} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {DeliveryQueue} from '../src/delivery-queue.js';
import {codexMetadataFile} from '../src/agents/codex-metadata.js';
const root=await mkdtemp(join(tmpdir(),'hive-legacy-steer-'));
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const options={executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}};
async function wait(check:()=>Promise<boolean>){const end=Date.now()+15000;while(!await check()){assert.ok(Date.now()<end,'Legacy queue fixture timeout');await delay(20);}}
try{
 // New-format definitely unattempted inputs and non-Codex legacy queues retain their contract.
 const queueDirectory=join(root,'unit');let executed=0;
 const queueOptions={directory:queueDirectory,sessionId:'modern',startPaused:true,holdLegacyCodexSteers:true,completedIds:()=>[],onError:(error:Error)=>{throw error;},execute:async()=>{executed++;}};
 const modern=new DeliveryQueue(queueOptions);await modern.enqueue('new unattempted',true,{version:1,origin:'user',displayText:'new unattempted'},undefined,{steerUser:true});
 const modernCold=new DeliveryQueue(queueOptions);assert.equal(modernCold.uncertainSteers.length,0);assert.equal(modernCold.pendingUserInputs[0]!.editable,true);modernCold.resume();await modernCold.idle();assert.equal(executed,1);
 const nonCodex=new DeliveryQueue({...queueOptions,sessionId:'non-codex',holdLegacyCodexSteers:false});await nonCodex.enqueue('legacy other backend',true,{version:1,origin:'user',displayText:'legacy other backend'},undefined,{steerUser:true});
 const otherCold=new DeliveryQueue({...queueOptions,sessionId:'non-codex',holdLegacyCodexSteers:false});assert.equal(otherCold.uncertainSteers.length,0);otherCold.resume();await otherCold.idle();assert.equal(executed,2);
 for(const scenario of ['never-sent','rpc-unsettled','ack-lost','exact-receipt','stop-retry']){
  const project=join(root,scenario),data=join(project,'state');await mkdir(project);
  await writeFile(join(project,'package.json'),'{"type":"commonjs"}');
  await writeFile(join(project,'app-server'),await readFile(fileURLToPath(new URL('./fixtures/codex-queue-peer.cjs',import.meta.url))));
  runtime=await createDesktopRuntime(data,()=>{},options);
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:scenario,autostart_orchestrator:false})as{id:string};
  const chat=await runtime.chats.create(workspace.id,'codex');
  await runtime.chats.prompt(chat.id,'Held primary');await wait(async()=>(await runtime!.chats.open(chat.id)).busy);
  const originalFile=join(project,'reference.txt');await writeFile(originalFile,'legacy attachment');
  const attachment=await runtime.chats.attachments.add(workspace.id,originalFile);
  await runtime.chats.prompt(chat.id,'Original legacy input',[attachment.attachment.id]);
  await runtime.chats.abort(chat.id);
  const snapshot=await runtime.chats.open(chat.id),item=snapshot.pendingUserInputs[0]!;
  const liveItem=(runtime.chats as any).live.get(chat.id).queue.pending[0];
  const audit=runtime.hive.getUserInput(workspace.id,snapshot.agentId!,liveItem.presentation.messageSequence);
  await runtime.close();runtime=undefined;
  const filename=createHash('sha256').update(chat.id).digest('hex')+'.json';
  const spool=join(data,'chats','deliveries',filename);
  const rows=JSON.parse(await readFile(spool,'utf8'));assert.equal(rows.length,1);
  rows[0].steerUser=true;rows[0].queueState='pending';delete rows[0].steerQueueVersion;delete rows[0].steerAttemptVersion;
  await writeFile(spool,JSON.stringify(rows));
  const nativeFile=join(project,'queue-peer-thread.json');
  const native=JSON.parse(await readFile(nativeFile,'utf8'));
  // A text lookalike, even with an ACK-shaped native record, is not delivery provenance.
  if(scenario==='ack-lost'){native.turns.push({id:'unrelated-turn',status:'completed',items:[{id:'unrelated-user',type:'userMessage',content:[{type:'text',text:rows[0].text}]}]});await writeFile(nativeFile,JSON.stringify(native));}
  if(scenario==='exact-receipt'){
   const file=codexMetadataFile(join(data,'chats','codex'),snapshot.codexThreadId!);
   const metadata=JSON.parse(await readFile(file,'utf8'));metadata.completed.push(item.deliveryId);await writeFile(file,JSON.stringify(metadata));
  }
  const wireBefore=await readFile(join(project,'queue-peer-queries.jsonl'),'utf8'),nativeBefore=await readFile(nativeFile);
  runtime=await createDesktopRuntime(data,()=>{},options);
  const review=await runtime.chats.uncertainDeliveries(chat.id);
  assert.equal((runtime.chats as any).live.has(chat.id),false,'Cold review must not open a native process');
  if(scenario==='exact-receipt'){assert.equal(review.deliveries.length,0);assert.equal(JSON.parse(await readFile(spool,'utf8')).length,0);await runtime.close();runtime=undefined;console.log('LEGACY_CODEX_STEER_OK exact-delivery-receipt');continue;}
  const reviewed=review.deliveries[0]!;assert.equal(review.deliveries.length,1);assert.equal(reviewed.deliveryId,item.deliveryId);assert.equal(reviewed.text,item.text);assert.deepEqual('attachments' in reviewed?reviewed.attachments:[],item.attachments);
  const cold=await runtime.chats.open(chat.id);assert.equal(cold.pendingUserInputs[0]!.editable,false);
  await runtime.chats.resume(chat.id);await wait(async() => !(await runtime!.chats.open(chat.id)).busy);
  assert.equal(await readFile(join(project,'queue-peer-queries.jsonl'),'utf8'),wireBefore,'Resume must not resend an ambiguous legacy input');
  assert.deepEqual(await readFile(nativeFile),nativeBefore);
  const decision=[chat.id,item.deliveryId,item.text,review.profile!,review.nativeSessionId!,'version' in reviewed?reviewed.version:undefined]as const;
  await assert.rejects(runtime.chats.retryUncertainDelivery(...[...decision.slice(0,5),'stale'] as Parameters<typeof runtime.chats.retryUncertainDelivery>),/identity changed/);
  await assert.rejects(runtime.chats.retryUncertainDelivery(chat.id,item.deliveryId,item.text,{...review.profile!,revision:review.profile!.revision+1},review.nativeSessionId!,decision[5]),/identity changed/);
  await assert.rejects(runtime.chats.retryUncertainDelivery(chat.id,item.deliveryId,item.text,review.profile!,'wrong-native',decision[5]),/identity changed/);
  assert.deepEqual(runtime.hive.getUserInput(workspace.id,snapshot.agentId!,liveItem.presentation.messageSequence),audit);

  if(scenario==='rpc-unsettled'){
   runtime.hive.mutateUserInput(workspace.id,snapshot.agentId!,liveItem.presentation.messageSequence,audit!.text,null,()=>{});
   await assert.rejects(runtime.chats.retryUncertainDelivery(...decision),/original Hive input has changed/);
   await runtime.chats.discardUncertainDelivery(...decision);
   assert.equal((await runtime.chats.discardUncertainDelivery(...decision)).alreadyDiscarded,true);
   await assert.rejects(runtime.chats.retryUncertainDelivery(...decision),/disposition changed/);
  }else{
   // Persist decision but fail queue transition. Cold recovery must apply it once.
   const backup=spool+'.saved';await rename(spool,backup);await mkdir(spool);
   try{await assert.rejects(runtime.chats.retryUncertainDelivery(...decision),/EISDIR|EPERM|EACCES/);}finally{await rm(spool,{recursive:true});await rename(backup,spool);}
   await runtime.close();
   let hiveReadyDrained=false;
   runtime=await createDesktopRuntime(data,event=>{if(event.type==='settled'&&event.sessionId===chat.id)hiveReadyDrained=true;},options);
   if(scenario==='ack-lost'){
    const original=await readFile(spool);
    const changed=JSON.parse(original.toString('utf8'));changed[0].text+=' changed after decision';
    await writeFile(spool,JSON.stringify(changed));
    try{await assert.rejects(runtime.chats.uncertainDeliveries(chat.id),/does not match the original queued input/);}
    finally{await writeFile(spool,original);}
   }
   await runtime.chats.uncertainDeliveries(chat.id);
   await runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(snapshot.agentId!)}/start`);
   await wait(async()=>{
    return hiveReadyDrained || await readFile(join(project,'queue-peer-queries.jsonl'),'utf8')!==wireBefore;
   });
   assert.equal(await readFile(join(project,'queue-peer-queries.jsonl'),'utf8'),wireBefore,'Hive ready cannot send an approved retry');
   assert.equal((await runtime.chats.open(chat.id)).deliveryFailure,undefined,'Earlier uncertainty must not hide Resume processing after approval');
   assert.equal((await runtime.chats.retryUncertainDelivery(...decision)).alreadyApproved,true);
   assert.equal(await readFile(join(project,'queue-peer-queries.jsonl'),'utf8'),wireBefore,'Approval is inert until explicit Resume');
   if(scenario==='never-sent'){
    // Crash after the approved retry reached turn/start must not grant it twice.
    await runtime.close();runtime=undefined;
    const attempted=JSON.parse(await readFile(spool,'utf8'));attempted[0].steerAttemptVersion=1;attempted[0].queueState='started';await writeFile(spool,JSON.stringify(attempted));
    runtime=await createDesktopRuntime(data,()=>{},options);
    const retried=await runtime.chats.uncertainDeliveries(chat.id),again=retried.deliveries[0]!;
    assert.equal(again.deliveryId,item.deliveryId);assert.notEqual('version' in again?again.version:undefined,decision[5]);
    await runtime.chats.resume(chat.id);await wait(async()=>!(await runtime!.chats.open(chat.id)).busy);
    assert.equal(await readFile(join(project,'queue-peer-queries.jsonl'),'utf8'),wireBefore);
    await runtime.chats.discardUncertainDelivery(chat.id,item.deliveryId,item.text,review.profile!,review.nativeSessionId!,'version' in again?again.version:undefined);
    await runtime.close();runtime=undefined;console.log('LEGACY_CODEX_STEER_OK retry-crash-no-auto-replay');continue;
   }
   if(scenario==='stop-retry'){
    await runtime.chats.resume(chat.id);
    await wait(async()=> (await readFile(join(project,'queue-peer-queries.jsonl'),'utf8')).trim().split('\n').length===2);
    await runtime.chats.abort(chat.id);
    const afterStop=await runtime.chats.uncertainDeliveries(chat.id);
    assert.equal(afterStop.deliveries[0]?.deliveryId,item.deliveryId,'Stop must preserve an interrupted approved retry');
    await runtime.close();runtime=undefined;console.log('LEGACY_CODEX_STEER_OK stop-preserved-retry');continue;
   }
   await writeFile(join(project,'queue-peer-control.json'),'{"complete":true}');
   await runtime.chats.resume(chat.id);await wait(async()=>{const state=await runtime!.chats.open(chat.id);return !state.busy&&state.pending===0;});
   const attempts=(await readFile(join(project,'queue-peer-queries.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
   assert.equal(attempts.length,2);
   assert.equal(attempts[1].input[0].text,rows[0].text,'The approved retry sends the exact original transport text and attachment references');
   assert.ok(JSON.parse(await readFile(codexMetadataFile(join(data,'chats','codex'),snapshot.codexThreadId!),'utf8')).completed.includes(item.deliveryId));
   assert.deepEqual(runtime.hive.getUserInput(workspace.id,snapshot.agentId!,liveItem.presentation.messageSequence),audit);
   assert.equal((await runtime.chats.retryUncertainDelivery(...decision)).alreadyApproved,true);
  }
  await runtime.close();runtime=await createDesktopRuntime(data,()=>{},options);
  assert.equal((await runtime.chats.open(chat.id)).pending,0);
  await runtime.close();runtime=undefined;
  console.log('LEGACY_CODEX_STEER_OK',scenario);
 }
}finally{await runtime?.close();await rm(root,{recursive:true,force:true});}
