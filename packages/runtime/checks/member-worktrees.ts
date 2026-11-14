/** Real Hive HTTP/SQLite + Git + paused Pi SDK. New production creation API only; no index.bind, no inference. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {chmod,mkdir,mkdtemp,readFile,realpath,rename,rm,stat,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {createTeamTool} from '../src/team-tool.js';
import {createManagedWorktree,readWorktreeIntent} from '../src/git-worktrees.js';
import type {Chats} from '../src/chats.js';
import type {ChatIndex} from '../src/chat-index.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';
const directory=await realpath(await mkdtemp(join(tmpdir(),'hive-member-worktrees-'))),repo=join(directory,'repo'),data=join(directory,'state'),pi=join(directory,'pi');
const keys=['PI_CODING_AGENT_DIR','PI_OFFLINE','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM','HIVE_INTENT_CAPTURE'] as const;
const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined,db:DatabaseSync|undefined,modelRequests=0;
const modelServer=createServer((_,response)=>{modelRequests++;response.writeHead(500).end('Unexpected inference');});
const originalFetch=globalThis.fetch;
globalThis.fetch=(input,init)=>{
 const url=new URL(typeof input==='string'?input:input instanceof URL?input:input.url);
 if(url.hostname!=='127.0.0.1'&&url.hostname!=='localhost'&&url.hostname!=='::1')return Promise.reject(new Error('Fixture disallows external fetch'));
 return originalFetch(input,init);
};
const internals=(chats:Chats)=>chats as unknown as {index:ChatIndex;live:Map<string,{queue:DeliveryQueue}>};
const exists=(path:string)=>stat(path).then(()=>true,()=>false);
const pause=async(chats:Chats,id:string)=>{const queue=internals(chats).live.get(id)!.queue;queue.resume=()=>{};await queue.pause();return queue;};
try{
 await mkdir(repo);await mkdir(pi);await writeFile(join(directory,'gitconfig'),'');
 process.env.PI_CODING_AGENT_DIR=pi;process.env.PI_OFFLINE='1';process.env.GIT_CONFIG_GLOBAL=join(directory,'gitconfig');process.env.GIT_CONFIG_NOSYSTEM='1';
 modelServer.listen(0,'127.0.0.1');await once(modelServer,'listening');const address=modelServer.address();assert.ok(address&&typeof address!=='string');
 await writeFile(join(pi,'models.json'),JSON.stringify({providers:{openai:{api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'fixture-only',models:[{id:'gpt-6.1-terra',name:'Fixture metadata only',input:['text']}]}}}));
 await writeFile(join(pi,'settings.json'),JSON.stringify({defaultProvider:'openai',defaultModel:'gpt-6.1-terra'}));
 const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd:repo,stdio:'pipe'});
 git('init','--template=');await writeFile(join(repo,'base.txt'),'base');git('add','.');git('commit','-qm','base');
 const backend=join(directory,'unexpected-backend'),backendMarker=join(directory,'backend-started');
 await writeFile(backend,`#!/bin/sh\nprintf unexpected > '${backendMarker.replace(/'/g,"'\\''")}'\nexit 1\n`);await chmod(backend,0o700);
 const launch=()=>createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:backend,HIVE_CLAUDE_COMMAND:backend,HIVE_PI_COMMAND:backend}});
 runtime=await launch();let first=runtime;
 const ws=await first.hive.request('POST','/api/workspaces',{path:repo,name:'Worker worktree fixture',autostart_orchestrator:false}) as {id:string};
 await mkdir(join(directory,'other'));const other=await first.hive.request('POST','/api/workspaces',{path:join(directory,'other'),name:'Other',autostart_orchestrator:false}) as {id:string};
 const worker=first.createMember(ws.id,{name:'isolated-worker',role:'coder',backend:'pi'});
 const foreign=first.createMember(other.id,{name:'foreign',role:'coder',backend:'pi'});
 first.hive.configureAgentLaunch(ws.id,worker.id,{command:'hive:pi',args:['--provider','openai','--model','gpt-6.1-terra']});
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'));
 const count=(table:string)=>(db!.prepare('SELECT COUNT(*) AS n FROM '+table).get() as {n:number}).n;
 for(const table of ['messages','dispatches','agent_runs'])assert.equal(count(table),0);
 const b=await first.chats.createMemberWorktree(ws.id,worker.id);
 assert.equal(b.backend,'pi');assert.equal(b.originAgentId,worker.id);assert.equal(b.identityState,'known');assert.equal(b.agentId,undefined);
 assert.equal('sessionFile'in b,false);assert.equal('piSessionId'in b,false);
 for(const table of ['messages','dispatches','agent_runs'])assert.equal(count(table),0,'creation never activates a backend');
 await first.chats.open(b.id);const qb=await pause(first.chats,b.id);
 await first.chats.prompt(b.id,'B explicitly activated as Worker');
 const bRun=first.agents.getActiveRunByAgentId(ws.id,worker.id)!.runId;
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,b.id);
 const bLaunch=first.agents.peekAgentLaunchConfig(ws.id,worker.id);
 const a=await first.chats.createMemberWorktree(ws.id,worker.id,{branch:'worker-A'});
 const beforeMessages=count('messages');await first.chats.open(a.id);let qa=await pause(first.chats,a.id);
 assert.equal(first.agents.getActiveRunByAgentId(ws.id,worker.id)!.runId,bRun);
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,b.id);assert.equal(count('messages'),beforeMessages);
 assert.deepEqual(first.agents.peekAgentLaunchConfig(ws.id,worker.id),bLaunch);
 assert.notEqual(a.execution!.path,b.execution!.path);assert.equal(a.originAgentId,worker.id);assert.equal(a.agentId,undefined);
 const aIntent=await readWorktreeIntent(data,a.id);assert.equal(aIntent.creation!.originAgentId,worker.id);assert.equal(aIntent.creation!.backend,'pi');assert.equal(aIntent.creation!.model!.id,'gpt-6.1-terra');
 await assert.rejects(first.chats.prompt(a.id,'not while B queued'),/SESSION_ORIGIN_BUSY/);
 await assert.rejects(first.chats.resume(a.id),/SESSION_ORIGIN_BUSY/);
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,b.id);
 qb.discardPending(new Set(qb.pending.map(item=>item.id)));
 // Real busy queue, controlled executor: no SDK inference is invoked.
 let entered!:()=>void,release!:()=>void;const ready=new Promise<void>(resolve=>{entered=resolve;}),gate=new Promise<void>(resolve=>{release=resolve;});
 const options=(qb as unknown as {options:{execute:()=>Promise<void>}}).options,execute=options.execute;
 options.execute=async()=>{entered();await gate;};
 await qb.enqueue('owned busy fixture',true,{version:1,origin:'user',displayText:'owned busy fixture'});DeliveryQueueResume(qb);await ready;
 await assert.rejects(first.chats.resume(a.id),/SESSION_ORIGIN_BUSY/);
 const paused=qb.pause();release();await paused;options.execute=execute;
 // A real Hive dispatch remains an activation guard even after its local queue item is retired.
 const lead=await first.chats.create(ws.id);await pause(first.chats,lead.id);await first.chats.resume(lead.id);
 const leadId=`${ws.id}:orchestrator`,leadRun=first.agents.getActiveRunByAgentId(ws.id,leadId)!.runId;
 const command=async(agentId:string,args:string[])=>JSON.parse((await createTeamTool({workspaceId:ws.id,agentId,token:first.agents.peekAgentToken(agentId)!,hivePort:String(first.hive.port),cwd:repo}).execute('worker-worktree-fixture',{args},undefined,undefined,undefined as never)).content.map(item=>'text'in item?item.text:'').join('')||'null');
 const sent=await command(leadId,['send','isolated-worker','Unsettled B responsibility']);const dispatchId=sent.dispatch_id as string;
 qb.discardPending(new Set(qb.pending.map(item=>item.id)));
 await assert.rejects(first.chats.prompt(a.id,'not while B dispatch unsettled'),/SESSION_ORIGIN_BUSY/);
 assert.equal(first.agents.getActiveRunByAgentId(ws.id,worker.id)!.runId,bRun);
 const inbox=await command(worker.id,['messages','--dispatch',dispatchId]);await command(worker.id,['report','B fixture done','--dispatch',dispatchId,'--seen',String(inbox.required_seen_seq)]);
 await first.chats.resume(a.id);qa=await pause(first.chats,a.id);
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,a.id);
 assert.equal(internals(first.chats).index.get(b.id).agentId,undefined);assert.equal(internals(first.chats).index.get(b.id).originAgentId,worker.id);
 await first.chats.prompt(a.id,'A must remain the original Worker');
 assert.ok(qa.pending.some(item=>item.presentation?.displayText==='A must remain the original Worker'));
 assert.equal(first.agents.getActiveRunByAgentId(ws.id,leadId)!.runId,leadRun);
 for(const id of [leadId,`${ws.id}:__workflow__`,'missing',foreign.id])await assert.rejects(first.chats.createMemberWorktree(ws.id,id),/MEMBER_INVALID|MEMBER_NOT_FOUND/);
 await assert.rejects(first.chats.createMemberWorktree(ws.id,worker.id,{backend:'codex'}),/MEMBER_BACKEND_MISMATCH/);
 assert.equal((await first.chats.memberWorktrees(other.id,worker.id)).sessions.length,0);
 const invalidConfigWorker=first.createMember(ws.id,{name:'invalid-config-worker',role:'coder',backend:'pi'});
 first.hive.configureAgentLaunch(ws.id,invalidConfigWorker.id,{command:'fixture:unsupported',args:[]});
 await assert.rejects(first.chats.createMemberWorktree(ws.id,invalidConfigWorker.id),/MEMBER_BACKEND_UNSUPPORTED/);
 first.hive.configureAgentLaunch(ws.id,invalidConfigWorker.id,{command:'hive:pi',args:['--model','gpt-6.1-terra']});
 await assert.rejects(first.chats.createMemberWorktree(ws.id,invalidConfigWorker.id),/MEMBER_MODEL_INVALID/);
 const codexWorker=first.createMember(ws.id,{name:'codex-not-started',role:'coder',backend:'codex'});
 const codex=await first.chats.createMemberWorktree(ws.id,codexWorker.id);assert.equal(codex.backend,'codex');assert.equal(await exists(backendMarker),false);
 const histories=await first.chats.memberWorktrees(ws.id,worker.id);assert.deepEqual(new Set(histories.sessions.map(item=>item.id)),new Set([a.id,b.id]));
 await rename(b.execution!.path,b.execution!.path+'.missing');try{
  const entries=(await first.chats.memberWorktrees(ws.id,worker.id)).sessions;assert.equal(entries.find(item=>item.id===b.id)!.executionAvailable,false);assert.equal(entries.find(item=>item.id===a.id)!.executionAvailable,true);
  await assert.rejects(first.chats.open(b.id),/worktree|工作树|ENOENT/i);
 }finally{await rename(b.execution!.path+'.missing',b.execution!.path);}
 // Observe complete private provenance inside the actual checkout hook, before index publication.
 if(process.platform!=='win32'){
  const hooks=join(directory,'hooks');await mkdir(hooks);process.env.HIVE_INTENT_CAPTURE=join(directory,'checkout-intent.json');
  await writeFile(join(hooks,'post-checkout'),'#!/bin/sh\ncp ../intent.json "$HIVE_INTENT_CAPTURE"\n');await chmod(join(hooks,'post-checkout'),0o700);git('config','core.hooksPath',hooks);
 }
 const index=join(data,'chats/index.json');await rename(index,index+'.held');await mkdir(index);
 try{await assert.rejects(first.chats.createMemberWorktree(ws.id,worker.id),/工作树已保留/);}finally{await rm(index,{recursive:true});await rename(index+'.held',index);if(process.platform!=='win32')git('config','--unset-all','core.hooksPath');}
 const pending=(await first.chats.pendingWorktrees(ws.id)).pending;assert.equal(pending.length,1);const orphan=pending[0]!;
 assert.equal(orphan.originAgentId,worker.id);assert.equal(orphan.backend,'pi');
 if(process.platform!=='win32'){const captured=JSON.parse(await readFile(process.env.HIVE_INTENT_CAPTURE!,'utf8'));assert.equal(captured.creation.originAgentId,worker.id);assert.equal(captured.creation.backend,'pi');assert.equal(captured.state,'creating');}
 await writeFile(join(orphan.path,'retain.txt'),'retained worker code');db.close();db=undefined;
 await first.close();runtime=undefined;runtime=await launch();first=runtime;
 await assert.rejects(first.chats.recoverWorktreeRecord(other.id,orphan.sessionId),/项目|SESSION_IDENTITY_CONFLICT/);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'codex'),/SESSION_IDENTITY_CONFLICT/);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'pi',codexWorker.id),/SESSION_IDENTITY_CONFLICT/);
 const intentPath=join(data,'worktrees',orphan.sessionId,'intent.json'),original=await readFile(intentPath,'utf8');
 for(const bad of [undefined,null,{version:2}, {...JSON.parse(original).creation,identityState:'new',originAgentId:worker.id}]){
  const broken=JSON.parse(original);broken.creation=bad;await writeFile(intentPath,JSON.stringify(broken));
  await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId),{code:'IDENTITY_MISMATCH'});
 }
 await writeFile(intentPath,original);
 await first.hive.request('DELETE',`/api/workspaces/${ws.id}/workers/${worker.id}`);
 const recovered=await first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId);
 const recoveredAgain=await first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'pi',worker.id);
 assert.deepEqual(recoveredAgain,recovered);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'codex'),/SESSION_IDENTITY_CONFLICT/);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'pi',codexWorker.id),/SESSION_IDENTITY_CONFLICT/);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId,'pi',`${ws.id}:orchestrator`),/SESSION_IDENTITY_CONFLICT/);
 const recoveredIntent=await readFile(intentPath,'utf8');
 await rename(intentPath,intentPath+'.held');
 try{await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId));}finally{await rename(intentPath+'.held',intentPath);}
 const invalidRetryIntent=JSON.parse(recoveredIntent);delete invalidRetryIntent.creation;await writeFile(intentPath,JSON.stringify(invalidRetryIntent));
 try{await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,orphan.sessionId),{code:'IDENTITY_MISMATCH'});}finally{await writeFile(intentPath,recoveredIntent);}
 assert.equal(recovered.originAgentId,worker.id);assert.equal(recovered.identityState,'known');assert.equal(recovered.agentId,undefined);assert.equal(recovered.backend,'pi');assert.equal(recovered.model!.id,'gpt-6.1-terra');
 assert.equal(await readFile(join(orphan.path,'retain.txt'),'utf8'),'retained worker code');assert.equal((await readWorktreeIntent(data,orphan.sessionId)).creation!.originAgentId,worker.id);
 // Explicitly legacy generic ownership: no inferred Worker, visible recovery semantics.
 const legacyId='old-generic-intent';await createManagedWorktree({projectRoot:repo,dataRoot:data,chatId:legacyId});
 const legacyPending=(await first.chats.pendingWorktrees(ws.id)).pending.find(item=>item.sessionId===legacyId)!;assert.equal(legacyPending.legacy,true);assert.equal(legacyPending.identityRecorded,false);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,legacyId,'pi',worker.id),/SESSION_IDENTITY_CONFLICT/);
 const legacy=await first.chats.recoverWorktreeRecord(ws.id,legacyId,'pi');assert.ok('legacy'in legacy && 'recoveryKind'in legacy);assert.equal(legacy.legacy,true);assert.equal(legacy.recoveryKind,'new-orchestrator');assert.equal(legacy.originAgentId,undefined);
 assert.equal(legacy.identityRecorded,false);
 const legacyAgain=await first.chats.recoverWorktreeRecord(ws.id,legacyId,'pi');
 assert.deepEqual(legacyAgain,legacy,'Retry keeps the original identity and every legacy recovery warning');
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,legacyId,'codex'),/SESSION_IDENTITY_CONFLICT/);
 await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,legacyId,'pi',worker.id),/SESSION_IDENTITY_CONFLICT/);
 const legacyIntentPath=join(data,'worktrees',legacyId,'intent.json'),legacyIntent=await readFile(legacyIntentPath,'utf8');
 await writeFile(legacyIntentPath,'{}');
 try{await assert.rejects(first.chats.recoverWorktreeRecord(ws.id,legacyId));}finally{await writeFile(legacyIntentPath,legacyIntent);}
 const generic=await first.chats.createRecord(ws.id);assert.equal(generic.identityState,'new');assert.equal(generic.originAgentId,undefined);assert.equal(generic.execution,undefined);

 const deleted=await first.chats.open(recovered.id);assert.ok('readOnly'in deleted && 'identityError'in deleted);assert.equal(deleted.readOnly,true);assert.equal(deleted.identityError.code,'SESSION_ORIGIN_DELETED');
 await assert.rejects(first.chats.prompt(recovered.id,'must not become orchestrator'),/SESSION_ORIGIN_DELETED/);await assert.rejects(first.chats.resume(recovered.id),/SESSION_ORIGIN_DELETED/);
 assert.ok((await first.chats.memberWorktrees(ws.id,worker.id)).sessions.some(item=>item.id===recovered.id));assert.equal(await readFile(join(orphan.path,'retain.txt'),'utf8'),'retained worker code');
 assert.equal(modelRequests,0);assert.equal(await exists(backendMarker),false);
 await first.close();runtime=undefined;
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,realHiveGit:true,noModelRequests:modelRequests,productionCreationOnly:true,currentBindingPreserved:true,workerActivation:true,queueBusyDispatchGuards:true,creationBeforeCheckout:true,coldRecoveryOriginModel:true,legacyExplicit:true,deletedReadOnly:true,recoveryRetryStable:true},null,2)+'\n');
 console.log(JSON.stringify({pass:true,directory,modelRequests}));
}finally{
 db?.close();await runtime?.close();if(modelServer.listening){modelServer.close();await once(modelServer,'close');}globalThis.fetch=originalFetch;
 for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}
}
// Invoke the real class implementation despite the fixture's per-instance pause override.
function DeliveryQueueResume(queue:DeliveryQueue){Object.getPrototypeOf(queue).resume.call(queue);}
