/** Real Git/Hive/runtime, controlled ACP and local Pi input hook. No vendor/model/auth. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,mkdir,readFile,writeFile,realpath} from 'node:fs/promises';
import {createServer} from 'node:http';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {randomUUID} from 'node:crypto';
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
 {
  const test=await setup('acp-uncertain','acp','uncertain'),text='Explicit first input whose ACP outcome is unknown';
  await runtime!.chats.prompt(test.id,text);await settled(test.id);
  let opened=await runtime!.chats.open(test.id) as any;
  await writeFile(join(test.folder,'after-send.json'),JSON.stringify(opened,null,2));
  assert.equal(opened.handoffDelivery.status,'uncertain');
  const queued=spool(test.data,test.id);assert.equal(queued.length,1);assert.equal(queued[0]!.presentation?.displayText,text);assert.ok(queued[0]!.contextRef);
  const native=JSON.parse(await readFile(join(test.peer,opened.acpSessionId+'.json'),'utf8'));
  assert.ok(native.history[0].text.includes(text));assert.equal(native.history[0].text.split('[Historical context — recorded material only]').length-1,1);
  assert.ok(native.history[0].text.indexOf(text)<native.history[0].text.indexOf('[Historical context — recorded material only]'));
  const prompt=native.history[0].text;
  await stopRun(test.ws.id,test.worker.id);await closeRuntime();
  runtime=await createDesktopRuntime(test.data,()=>{},{executableEnvironment:{}});
  const before=await protocol(test.peer);opened=await runtime.chats.open(test.id) as any;
  assert.equal(opened.handoffDelivery.status,'uncertain');assert.equal(opened.passive,true);assert.deepEqual(spool(test.data,test.id),queued);
  await assert.rejects(runtime.chats.resume(test.id),/uncertain|ordinary text/);
  await assert.rejects(runtime.chats.prompt(test.id,'New input must not retry historical context'),/may already have been delivered/);
  assert.deepEqual(await protocol(test.peer),before);assert.equal(before.filter(row=>row.method==='session/prompt').length,1);assert.equal(before.filter(row=>row.event==='start').length,1);
  assert.equal(JSON.parse(await readFile(join(test.peer,opened.acpSessionId+'.json'),'utf8')).history[0].text,prompt);
  results.push({name:'acp-uncertain',passed:true,sessionId:test.id,nativeSessionId:opened.acpSessionId,deliveryId:queued[0]!.id,promptCount:1});await closeRuntime();
 }
 {
  const test=await setup('acp-consumed-write-failure','acp');
  const deliveries=(runtime!.chats as any).handoffDeliveries;let injected=0;
  deliveries.consumed=()=>{injected++;throw Error('Injected persistent consumed marker failure');};
  await runtime!.chats.prompt(test.id,'Explicit first input completes once despite marker failure');const live=await settled(test.id);
  assert.equal(live.queue.isPaused,true);assert.equal(live.queue.pendingCount,1);assert.ok(injected>=2);
  const queued=spool(test.data,test.id);assert.equal(queued.length,1);
  const index=(runtime!.chats as any).index as ChatIndex,record=index.get(test.id);
  const history=JSON.parse(await readFile(record.sessionFile!,'utf8'));assert.equal(history.deliveries[queued[0]!.id],'completed');
  assert.equal(deliveries.get(test.id).status,'attempted');
  await stopRun(test.ws.id,test.worker.id);await closeRuntime();
  runtime=await createDesktopRuntime(test.data,()=>{},{executableEnvironment:{}});
  const before=await protocol(test.peer),opened=await runtime.chats.open(test.id) as any;
  assert.equal(opened.passive,true);assert.equal(opened.handoffDelivery.status,'consumed');assert.equal(opened.acpSessionId,record.acpSessionId);assert.deepEqual(spool(test.data,test.id),queued);assert.deepEqual(await protocol(test.peer),before);
  await runtime.chats.resume(test.id);const resumed=await settled(test.id);assert.equal(resumed.queue.pendingCount,0);assert.equal(spool(test.data,test.id).length,0);
  const after=await protocol(test.peer);assert.equal(after.filter(row=>row.method==='session/prompt').length,1);assert.equal(after.filter(row=>row.method==='session/new').length,1);
  results.push({name:'acp-consumed-write-failure',passed:true,sessionId:test.id,nativeSessionId:record.acpSessionId,deliveryId:queued[0]!.id,injectedFailures:injected,promptCount:1,coldPassiveRepaired:true});await stopRun(test.ws.id,test.worker.id);await closeRuntime();
 }
 {
  const test=await setup('pi-local-handled','pi'),text='Ordinary first input handled by owned extension';
  await runtime!.chats.prompt(test.id,text);const live=await settled(test.id);
  assert.equal(live.queue.isPaused,true);assert.equal(live.queue.pendingCount,1);
  const queued=spool(test.data,test.id),opened=await runtime!.chats.open(test.id) as any;
  assert.equal(opened.handoffDelivery.status,'uncertain');assert.ok(live.handle.completedDeliveryIds().includes(queued[0]!.id));
  const record=((runtime!.chats as any).index as ChatIndex).get(test.id),transcript=await readFile(record.sessionFile!,'utf8');
  assert.ok(transcript.includes('"disposition":"handled"'));assert.ok(transcript.includes('hive-delivery-completed'));
  assert.ok((await readFile(join(test.folder,'hook.jsonl'),'utf8')).includes(text));assert.equal(modelRequests,0);
  await stopRun(test.ws.id,test.worker.id);await closeRuntime();
  runtime=await createDesktopRuntime(test.data,()=>{},{executableEnvironment:{}});
  const cold=await runtime.chats.open(test.id) as any;assert.equal(cold.handoffDelivery.status,'uncertain');assert.equal(cold.passive,true);assert.deepEqual(spool(test.data,test.id),queued);
  await assert.rejects(runtime.chats.resume(test.id),/uncertain|ordinary text/);assert.equal(modelRequests,0);
  results.push({name:'pi-local-handled',passed:true,sessionId:test.id,deliveryId:queued[0]!.id,completedReceiptNotConsumption:true,modelRequests});await closeRuntime();
 }
 assert.equal(modelRequests,0);
 await writeFile(join(root,'receipt.json'),JSON.stringify({pass:true,realGit:true,realHive:true,noModelOrAuth:true,modelRequests,results},null,2));
 console.log('HANDOFF_RUNTIME_DELIVERY_OK '+root);
}catch(error){await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),stack:error instanceof Error?error.stack:undefined,results},null,2));throw error;}
finally{await closeRuntime();await new Promise<void>(done=>server.close(()=>done()));for(const[key,value]of Object.entries(prior)){if(value===undefined)delete process.env[key];else process.env[key]=value;}console.log('Evidence retained '+root);}
