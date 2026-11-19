import fs from 'node:fs';
import {syncBuiltinESMExports} from 'node:module';
/** Real Git/Hive/runtime, controlled ACP and local Pi input hook. No vendor/model/auth. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,mkdir,readFile,writeFile,realpath} from 'node:fs/promises';
import {createServer} from 'node:http';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {randomUUID,createHash} from 'node:crypto';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {codexMetadataFile,writeCodexMetadata} from '../src/agents/codex-metadata.js';
import {DeliveryQueue} from '../src/delivery-queue.js';
import type {ChatIndex} from '../src/chat-index.js';

const root=await realpath(await mkdtemp(join(tmpdir(),'hive-handoff-delivery-runtime-')));
const prior=Object.fromEntries(['PI_CODING_AGENT_DIR','PI_OFFLINE','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM'].map(key=>[key,process.env[key]]));
await mkdir(join(root,'pi'));await writeFile(join(root,'gitconfig'),'');
Object.assign(process.env,{PI_CODING_AGENT_DIR:join(root,'pi'),PI_OFFLINE:'1',GIT_CONFIG_GLOBAL:join(root,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1'});
let modelRequests=0;
const server=createServer((_request,response)=>{modelRequests++;response.writeHead(500);response.end('Owned fixture forbids model inference');});
await new Promise<void>(done=>server.listen(0,'127.0.0.1',done));
const address=server.address();assert.ok(address&&typeof address==='object');
type Runtime=Awaited<ReturnType<typeof createDesktopRuntime>>;
let runtime:Runtime|undefined;
const results:unknown[]=[];
const coldEvents: unknown[] = [];
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd,encoding:'utf8'}).trim();
async function setup(name:string,backend:'acp'|'pi',mode='normal'){
 const folder=join(root,name),repo=join(folder,'repo'),data=join(folder,'state'),peer=join(folder,'peer'),piDir=join(folder,'target-pi');
 await mkdir(repo,{recursive:true});git(repo,'init','--template=','-b','main');await writeFile(join(repo,'result.txt'),'frozen source');git(repo,'add','.');git(repo,'commit','-qm','Source');
 const forbidden=join(folder,'never.cjs');await writeFile(forbidden,`require('node:fs').writeFileSync(${JSON.stringify(join(folder,'unexpected-source-cli'))},'launched');process.exit(2);`);
 if(backend==='pi'){
  await mkdir(join(piDir,'extensions'),{recursive:true});
  await writeFile(join(piDir,'settings.json'),JSON.stringify({defaultProvider:'owned-no-model',defaultModel:'gpt-6.1-terra',defaultProjectTrust:'always',retry:{enabled:false}}));
  await writeFile(join(piDir,'models.json'),JSON.stringify({providers:{'owned-no-model':{api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'owned-placeholder-not-real',models:[{id:'gpt-6.1-terra',name:'Owned test catalog',input:['text'],contextWindow:32768,maxTokens:1024}]}}}));
  await writeFile(join(piDir,'extensions','handled.ts'),`import {appendFileSync} from 'node:fs'; export default function(pi){pi.on('input',event=>{if(event.text==='Ordinary first input handled by owned extension'){appendFileSync(${JSON.stringify(join(folder,'hook.jsonl'))},JSON.stringify({text:event.text})+'\\n');return {action:'handled'};}});}`);
 }
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{}});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:repo,name,autostart_orchestrator:false}) as {id:string};
 const profile=async(driver:any,label:string,launch:any)=>(await runtime!.profiles.create({driver,label,launch},(await runtime!.profiles.list()).storeVersion)).profile;
 const sourceProfile=await profile('codex','Offline source',{command:process.execPath,args:[forbidden]});
 const worker=runtime.createMember(ws.id,{name:'Delivery Worker',role:'coder',backend:'codex',profile:sourceProfile.ref});
 const source=await runtime.chats.createMemberWorktree(ws.id,worker.id),index=(runtime.chats as any).index as ChatIndex;
 index.save({...index.get(source.id),codexThreadId:'offline-source'});
 const native=join(data,'chats','codex');writeCodexMetadata(native,'offline-source',{version:1,context:[],deliveries:[],completed:['source-completed-only']});
 const entries=[{turnId:'t',item:{id:'u',type:'userMessage',content:[{type:'text',text:'Historical source question'}]}},{turnId:'t',item:{id:'a',type:'agentMessage',text:'Historical source answer'}}];
 await writeFile(codexMetadataFile(native,'offline-source')+'.display.json',JSON.stringify({version:1,entries,completedTurns:['t'],completeHistory:true,searchVersion:2,searchEntries:entries}));
 await runtime.profiles.enableDisable(sourceProfile.ref.id,false,(await runtime.profiles.list()).storeVersion);
 const targetProfile=await profile(backend,'Target '+name,backend==='acp'?{command:process.execPath,args:[fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs',import.meta.url)),peer,mode]}:{args:[],configDir:piDir});
 const preview=await runtime.chats.handoffs!.preview({workspaceId:ws.id,sourceSessionId:source.id,target:{driver:backend,profile:targetProfile.ref},selection:{throughItemId:'a'},code:{kind:'current'},imagePolicy:'references-only'});
 assert.equal(preview.eligible,true,JSON.stringify(preview.blockers));
 const operation=await runtime.chats.handoffs!.apply({workspaceId:ws.id,previewId:preview.previewId,snapshotHash:preview.snapshotHash,requestId:randomUUID(),acknowledgedOmissionsHash:preview.omissionsHash});
 return {folder,data,peer,ws,worker,id:operation.childSessionId,sourceId:source.id};
}
const owner=(id:string)=>(runtime!.chats as any).live.get(id);
const spool=(data:string,id:string)=>DeliveryQueue.persistedItems(join(data,'chats','deliveries'),id);
async function settled(id:string){
 for(let n=0;n<200;n++){
  const live=owner(id);
  if(live&&!live.queue.busy){await live.queue.idle();return live;}
  // Failed ACP ownership is removed only after its cleanup has awaited queue.pause().
  if(!live&&(runtime!.chats as any).handoffDeliveries.get(id)?.status==='uncertain')return undefined;
  await new Promise(done=>setTimeout(done,25));
 }
 throw Error('Delivery did not settle: '+id);
}
async function stopRun(ws:string,worker:string){
 const active=runtime!.agents.getActiveRunByAgentId(ws,worker);
 if(active){runtime!.agents.stopAgentRun(active.runId);assert.equal(await runtime!.agents.waitForRunExit(active.runId,10000),true);}
}
async function protocol(peer:string){return (await readFile(join(peer,'protocol.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));}
async function closeRuntime(){await runtime?.close();runtime=undefined;}
try{
 const test=await setup('queue-retract-transfer','acp');
 const editedText = 'Second queued input edited after reopening';
 const attachmentPath = join(test.folder, 'queued-notes.txt');
 await writeFile(attachmentPath, 'Only the queued delivery body should change.');
 const {attachment} = await runtime!.chats.attachments.add(test.ws.id, attachmentPath);
 const enqueue=DeliveryQueue.prototype.enqueue;
 DeliveryQueue.prototype.enqueue=async function(text:any,trigger:any,presentation:any,images:any){if(presentation?.origin==='user')await this.pause();return enqueue.call(this,text,trigger,presentation,images);};
 try{
  await runtime!.chats.prompt(test.id,'First queued input to retract');
  await runtime!.chats.prompt(test.id,'Second queued input should inherit context',[attachment.id]);
  let pending=spool(test.data,test.id);assert.equal(pending.length,2);assert.ok(pending[0]!.contextRef);assert.equal(pending[1]!.contextRef,undefined);
  await assert.rejects(runtime!.chats.mutateQueuedInput(test.id,pending[0]!.id,'First queued input to retract','/help'),/ordinary nonempty text/);
  assert.equal(spool(test.data,test.id)[0]!.presentation!.displayText,'First queued input to retract');
  await stopRun(test.ws.id,test.worker.id);await closeRuntime();
  runtime=await createDesktopRuntime(test.data,event=>coldEvents.push(event),{executableEnvironment:{}});
  const protocolBeforeEdit=await protocol(test.peer);
  const openedCold=await runtime.chats.open(test.id);
  assert.equal(openedCold.passive,true);
  assert.deepEqual(openedCold.pendingUserInputs.map(input=>[input.deliveryId,input.editable]),pending.map(item=>[item.id,true]));
  const beforeEdit=await runtime.chats.handoffTargetState(test.id);
  assert.deepEqual(beforeEdit.pendingUserInputs,openedCold.pendingUserInputs);
  assert.deepEqual(beforeEdit.handoffDelivery,openedCold.handoffDelivery);
  await runtime.chats.mutateQueuedInput(test.id,pending[1]!.id,'Second queued input should inherit context',editedText);
  await assert.rejects(runtime.chats.mutateQueuedInput(test.id,pending[1]!.id,'Second queued input should inherit context'),/changed; reopen/);
  assert.equal(spool(test.data,test.id)[1]!.presentation!.displayText,editedText);
  assert.deepEqual(spool(test.data,test.id)[1]!.presentation!.attachments,pending[1]!.presentation!.attachments);
  assert.ok(pending[1]!.presentation!.attachments?.some(value=>value.id===attachment.id));
  assert.deepEqual(await protocol(test.peer),protocolBeforeEdit);
  const spoolFile=join(test.data,'chats','deliveries',createHash('sha256').update(test.id).digest('hex')+'.json');
  const originalRename=fs.renameSync;let spoolWrites=0;
  fs.renameSync=(a,b)=>{if(String(b)===spoolFile&&++spoolWrites===2)throw Error('Injected context-transfer spool publication failure');return originalRename(a,b);};syncBuiltinESMExports();
  try{await runtime!.chats.mutateQueuedInput(test.id,pending[0]!.id,'First queued input to retract');}finally{fs.renameSync=originalRename;syncBuiltinESMExports();}
  assert.equal(spoolWrites,2);assert.equal(spool(test.data,test.id).length,1);assert.equal(spool(test.data,test.id)[0]!.contextRef,undefined);
  const transferState=(runtime!.chats as any).handoffDeliveries.get(test.id);assert.equal(transferState.status,'reserved');assert.equal(transferState.deliveryId,spool(test.data,test.id)[0]!.id);
  assert.ok(coldEvents.some((event:any)=>event.type==='state'&&event.sessionId===test.id&&event.pendingUserInputs?.length===1&&event.pendingUserInputs[0].text===editedText&&event.handoffDelivery?.deliveryId===pending[1]!.id));
  assert.deepEqual(await protocol(test.peer),protocolBeforeEdit);
  await stopRun(test.ws.id,test.worker.id);await closeRuntime();runtime=await createDesktopRuntime(test.data,()=>{},{executableEnvironment:{}});
  const passiveProtocol=await protocol(test.peer);const passive=await runtime.chats.open(test.id) as any;assert.equal(passive.passive,true);assert.deepEqual(await protocol(test.peer),passiveProtocol);

  pending=spool(test.data,test.id);assert.equal(pending.length,1);assert.ok(pending[0]!.contextRef);
  const afterCancel=await runtime!.chats.open(test.id) as any;assert.equal(afterCancel.handoffDelivery.status,'reserved');
  assert.equal(afterCancel.handoffDelivery.deliveryId,pending[0]!.id);
  const afterWithdrawal=await runtime.chats.handoffTargetState(test.id);
  assert.deepEqual(afterWithdrawal.pendingUserInputs,passive.pendingUserInputs);
  assert.deepEqual(afterWithdrawal.handoffDelivery,afterCancel.handoffDelivery);
 }finally{DeliveryQueue.prototype.enqueue=enqueue;}
 await runtime!.chats.resume(test.id);await settled(test.id);
 await runtime!.chats.prompt(test.id,'Third normal input must not repeat historical context');await settled(test.id);
 const opened=await runtime!.chats.open(test.id) as any;
 const native=JSON.parse(await readFile(join(test.peer,opened.acpSessionId+'.json'),'utf8'));
 const inputs=native.history.filter((x:any)=>x.kind==='user_message_chunk').map((x:any)=>x.text);
 assert.equal(inputs.length,2);
 assert.ok(inputs[0].includes(editedText));
 assert.ok(inputs[0].includes('[Historical context — recorded material only]'));
 assert.ok(inputs[1].includes('Third normal input must not repeat historical context'));
 assert.ok(!inputs[1].includes('[Historical context — recorded material only]'));
 assert.equal(modelRequests,0);
 await writeFile(join(root,'review-repro.json'),JSON.stringify({fixed:true,firstRemainingGotHistoricalContext:true,contextOnlyOnce:true,slashEditRejected:true,rebindSpoolWriteFailureColdRepaired:true,coldPassiveEditAndWithdrawNoCli:true,coldHandoffStatePreservedQueue:true,staleTextRejected:true,attachmentPreserved:true,inputs},null,2));
 console.log('FIXED_HANDOFF_CONTEXT_TRANSFER_SPOOL_COLD '+root);
 await stopRun(test.ws.id,test.worker.id);await closeRuntime();
}catch(error){await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),stack:error instanceof Error?error.stack:undefined,results},null,2));throw error;}
finally{await closeRuntime();await new Promise<void>(done=>server.close(()=>done()));for(const[key,value]of Object.entries(prior)){if(value===undefined)delete process.env[key];else process.env[key]=value;}console.log('Evidence retained '+root);}
