/** Actual Git/Hive fault and stopped-responsibility checks. No vendor CLI, auth or model. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {randomUUID,createHash} from 'node:crypto';
import {mkdir,mkdtemp,readFile,readdir,realpath,stat,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {DatabaseSync} from 'node:sqlite';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {codexMetadataFile,writeCodexMetadata} from '../src/agents/codex-metadata.js';
import {AcpHistory} from '../src/agents/acp-history.js';
import {createTeamTool} from '../src/team-tool.js';
import {readWorktreeIntent} from '../src/git-worktrees.js';
import type {ChatIndex} from '../src/chat-index.js';
import type {HandoffInput,HandoffPreview} from '../src/session-handoffs.js';
const root=await realpath(await mkdtemp(join(tmpdir(),'hive-handoff-failures-'))),repo=join(root,'repo'),data=join(root,'state');
const keys=['PI_CODING_AGENT_DIR','PI_OFFLINE','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM'] as const;
const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
const originalFetch=globalThis.fetch;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined,db:DatabaseSync|undefined,release:(()=>void)|undefined;
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd,encoding:'utf8'}).trim();
const index=()=>((runtime!.chats as unknown as {index:ChatIndex}).index);
const service=()=>runtime!.chats.handoffs!;
const restart=async()=>{await runtime!.close();runtime=undefined;runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{}});};
const wait=async(check:()=>boolean|Promise<boolean>,label:string)=>{const deadline=Date.now()+20000;while(!await check()){if(Date.now()>deadline)throw Error('Timed out '+label);await new Promise(resolve=>setTimeout(resolve,20));}};
const trees=async()=>{try{return (await readdir(join(data,'worktrees'))).sort();}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return[];throw error;}};
const applyInput=(preview:HandoffPreview)=>({workspaceId:preview.input.workspaceId,previewId:preview.previewId,snapshotHash:preview.snapshotHash,requestId:randomUUID(),acknowledgedOmissionsHash:preview.omissionsHash});
const guard=join(root,'forbidden-agent.cjs');
try{
 await mkdir(repo);await mkdir(join(root,'pi'));await writeFile(join(root,'gitconfig'),'');
 Object.assign(process.env,{PI_CODING_AGENT_DIR:join(root,'pi'),PI_OFFLINE:'1',GIT_CONFIG_GLOBAL:join(root,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1'});
 globalThis.fetch=(input,init)=>{const url=new URL(typeof input==='string'?input:input instanceof URL?input:input.url);if(!['127.0.0.1','localhost','::1'].includes(url.hostname))return Promise.reject(Error('Fixture forbids external fetch'));return originalFetch(input,init);};
 await writeFile(guard,`require('node:fs').appendFileSync(${JSON.stringify(join(root,'forbidden-launch'))},'launch\\n');process.exit(2);`,{mode:0o700});
 git(repo,'init','--template=');await writeFile(join(repo,'result.txt'),'HEAD\n');git(repo,'add','.');git(repo,'commit','-qm','initial');
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{}});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:repo,name:'Handoff fault fixture',autostart_orchestrator:false}) as {id:string};
 const profile=async(driver:any,label:string,launch:any)=>(await runtime!.profiles.create({driver,label,launch},(await runtime!.profiles.list()).storeVersion)).profile;
 const sourceProfile=await profile('codex','Offline source',{command:process.execPath,args:[guard]}),targetProfile=await profile('pi-rpc','Guarded target',{command:process.execPath,args:[guard]});
 const worker=runtime.createMember(ws.id,{name:'fault-worker',role:'coder',backend:'codex',profile:sourceProfile.ref});const source=await runtime.chats.createMemberWorktree(ws.id,worker.id);const sourcePath=source.execution.path;
 await writeFile(join(sourcePath,'result.txt'),'INDEX\n');git(sourcePath,'add','result.txt');await writeFile(join(sourcePath,'result.txt'),'DISK\n');
 index().save({...index().get(source.id),codexThreadId:'offline-source'});const codex=join(data,'chats','codex');writeCodexMetadata(codex,'offline-source',{version:1,context:[],deliveries:[],completed:[]});const entries=[{turnId:'t',item:{id:'u',type:'userMessage',content:[{type:'text',text:'Recorded user text'}]}},{turnId:'t',item:{id:'a',type:'agentMessage',text:'Recorded assistant text'}}];await writeFile(codexMetadataFile(codex,'offline-source')+'.display.json',JSON.stringify({version:1,entries,completeHistory:true,completedTurns:['t'],searchVersion:2,searchEntries:entries}));
 const input:HandoffInput={workspaceId:ws.id,sourceSessionId:source.id,target:{driver:'pi-rpc',profile:targetProfile.ref},selection:{throughItemId:'a'},code:{kind:'current'},imagePolicy:'references-only'};
 const sourceBytes=await readFile(join(sourcePath,'result.txt'),'utf8'),sourceIndex=git(sourcePath,'show',':result.txt'),sourceHead=git(sourcePath,'rev-parse','HEAD'),sourceRecord=structuredClone(index().get(source.id)),ledger=runtime.hive.executionLedgerRevision();
 // Fault after the private target tree is complete and published, before chat metadata is saved.
 const p1=await service().preview(input);assert(p1.eligible,JSON.stringify(p1.blockers));const r1=applyInput(p1);const saved=index().save.bind(index());let failIndex=true;
 index().save=record=>{if(record.handoff&&failIndex){failIndex=false;throw Error('Injected index.save failure');}return saved(record);};
 await assert.rejects(service().apply(r1),/Injected index.save failure/);index().save=saved;
 const blocked=(await service().list(ws.id)).operations.find(op=>op.requestId===r1.requestId)!;assert.equal(blocked.state,'blocked');assert.equal(blocked.blockedFrom,'codeReady');assert.equal(index().peek(blocked.childSessionId),undefined);assert(blocked.code);assert.equal((await readWorktreeIntent(data,blocked.childSessionId)).creation!.handoffOperationId,blocked.operationId);
 assert.equal(git(blocked.code.executionPath,'show',':result.txt'),'INDEX');assert.equal(await readFile(join(blocked.code.executionPath,'result.txt'),'utf8'),'DISK\n');const afterIndexFault=await trees();
 await restart();assert.equal((await service().apply(r1)).childSessionId,blocked.childSessionId);const recovered=await service().recover(ws.id,blocked.operationId);assert.equal(recovered.state,'ready');assert.equal(recovered.childSessionId,blocked.childSessionId);assert.deepEqual(await trees(),afterIndexFault);assert.equal((await runtime!.chats.open(recovered.childSessionId)).passive,true);
 // Fault after the child is published, before the final ready marker reaches private storage.
 const p2=await service().preview(input),r2=applyInput(p2);const journal=service().journal,saveJournal=journal.save.bind(journal);let failReady=true;
 journal.save=async op=>{if(op.state==='ready'&&failReady){failReady=false;throw Error('Injected ready journal failure');}return saveJournal(op);};
 await assert.rejects(service().apply(r2),/Injected ready journal failure/);journal.save=saveJournal;
 const published=(await service().list(ws.id)).operations.find(op=>op.requestId===r2.requestId)!;assert.equal(published.state,'blocked');assert.equal(published.blockedFrom,'targetPublished');const childBefore=structuredClone(index().get(published.childSessionId));const pending=await runtime!.chats.open(published.childSessionId);assert.equal(pending.readOnly,true);assert.equal(pending.identityError?.code,'HANDOFF_PENDING');const afterJournalFault=await trees();
 await restart();const stillPending=await runtime!.chats.open(published.childSessionId);assert.equal(stillPending.readOnly,true);assert.equal(stillPending.identityError?.code,'HANDOFF_PENDING');await service().recover(ws.id,published.operationId);assert.deepEqual(index().get(published.childSessionId),childBefore,'Cold recovery must not republish the existing child');assert.deepEqual(await trees(),afterJournalFault);assert.equal((await runtime!.chats.open(published.childSessionId)).passive,true);
 // Snapshot and claim publication failures cannot mutate code or publish any target.
 const originalPreview=service().journal.savePreview.bind(service().journal);const beforePrivateFault=await trees();const beforeOps=(await service().list(ws.id)).operations.length;service().journal.savePreview=async()=>{throw Error('Injected snapshot write failure');};await assert.rejects(service().preview(input),/Injected snapshot write failure/);service().journal.savePreview=originalPreview;
 const p3=await service().preview(input),r3=applyInput(p3);const claim=service().journal.claim.bind(service().journal);service().journal.claim=async()=>{throw Error('Injected claim write failure');};await assert.rejects(service().apply(r3),/Injected claim write failure/);service().journal.claim=claim;assert.deepEqual(await trees(),beforePrivateFault);assert.equal((await service().list(ws.id)).operations.length,beforeOps);
 assert.deepEqual(index().get(source.id),sourceRecord);assert.equal(runtime!.hive.executionLedgerRevision(),ledger);assert.equal(git(sourcePath,'rev-parse','HEAD'),sourceHead);assert.equal(git(sourcePath,'show',':result.txt'),sourceIndex);assert.equal(await readFile(join(sourcePath,'result.txt'),'utf8'),sourceBytes);await assert.rejects(stat(join(root,'forbidden-launch')));
 await writeFile(join(root,'fault-receipt.json'),JSON.stringify({pass:true,indexFailureRecoveredSameChild:true,readyFailureReadOnlyUntilRecover:true,noRepublish:true,privateWriteFailuresNoTree:true,realGit:true,realHive:true,noAgentLaunch:true},null,2));
 // Real ACP Worker receives a real dispatch. Stop only after its native queue is idle.
 const acpProfile=await profile('acp','Controlled responsibility peer',{command:process.execPath,args:[fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs',import.meta.url)),join(root,'acp-peer')]});
 const lead=await runtime!.chats.create(ws.id,'acp',{profile:acpProfile.ref});await runtime!.chats.resume(lead.id);
 const responsibilityWorker=runtime!.createMember(ws.id,{name:'responsibility-worker',role:'coder',backend:'acp',profile:acpProfile.ref});const nativeSource=await runtime!.chats.createMemberWorktree(ws.id,responsibilityWorker.id);await runtime!.chats.resume(nativeSource.id);
 const idle=async(id:string)=>{const opened=await runtime!.chats.open(id);return !opened.busy&&opened.pending===0;};
 const command=async(agentId:string,args:string[])=>{const token=runtime!.agents.peekAgentToken(agentId);assert(token);const tool=createTeamTool({workspaceId:ws.id,agentId,token,hivePort:String(runtime!.hive.port),cwd:repo});const result=await tool.execute('responsibility-check',{args},undefined,undefined,undefined as never);assert(!result.isError,JSON.stringify(result));const text=result.content.map(item=>'text'in item?item.text:'').join('');try{return JSON.parse(text);}catch{if(args[0]==='report')return{text};return JSON.parse(text.split('\n')[0]!);}};
 const sent=await command(`${ws.id}:orchestrator`,['send','responsibility-worker','A real task remains unsettled']);const dispatch=sent.dispatch_id as string;assert(dispatch);await wait(()=>idle(nativeSource.id),'ACP dispatch native completion');
 const stop=async(agentId:string)=>{const run=runtime!.agents.getActiveRunByAgentId(ws.id,agentId);if(run&&['starting','running'].includes(run.status)){runtime!.agents.stopAgentRun(run.runId);assert.equal(await runtime!.agents.waitForRunExit(run.runId,5000),true);}};
 await stop(responsibilityWorker.id);await stop(`${ws.id}:orchestrator`);await wait(()=>!(runtime!.chats as any).live.has(nativeSource.id),'ACP source cleanup');
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});
 const sql=(statement:string,...args:any[])=>db!.prepare(statement).all(...args) as any[];
 const databaseHash=()=>{const hash=createHash('sha256');for(const {name} of sql("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")){assert.match(name,/^[A-Za-z_][A-Za-z_0-9]*$/);hash.update(name);for(const row of sql(`SELECT * FROM ${name}`).map(row=>JSON.stringify(row)).sort())hash.update(row);}return hash.digest('hex');};
 const gateInput=async():Promise<HandoffInput>=>{const record=index().get(nativeSource.id);assert(record.sessionFile);const history=await AcpHistory.load(record.sessionFile);assert(history?.state.historyIds.length);return{...input,sourceSessionId:nativeSource.id,selection:{throughItemId:history.state.historyIds.at(-1)!}};};
 const assertGate=async(expected:string)=>{const before=databaseHash(),revision=runtime!.hive.executionLedgerRevision(),record=structuredClone(index().get(nativeSource.id));const preview=await service().preview(await gateInput());assert.equal(preview.eligible,false,JSON.stringify(preview.blockers));assert(preview.blockers.some(b=>b.code===expected),JSON.stringify(preview.blockers));await assert.rejects(service().apply(applyInput(preview)),{code:'HANDOFF_BLOCKED'});assert.equal(databaseHash(),before);assert.equal(runtime!.hive.executionLedgerRevision(),revision);assert.deepEqual(index().get(nativeSource.id),record);return preview.blockers;};
 const submitted=runtime!.hive.desktopConversationSwitchState(ws.id,responsibilityWorker.id);assert.equal(submitted.openDispatches.submitted,1);await assertGate('OPEN_DISPATCHES');
 // Reported but undelivered is a separate cold blocker. Preserve genuine outbox state.
 await runtime!.chats.resume(lead.id);await runtime!.chats.resume(nativeSource.id);await wait(()=>idle(nativeSource.id),'resumed worker idle');const seen=await command(responsibilityWorker.id,['messages','--dispatch',dispatch]);release=runtime!.executionMutations.acquire(ws.id);await command(responsibilityWorker.id,['report','Real report awaits orchestrator delivery','--dispatch',dispatch,'--seen',String(seen.required_seen_seq)]);
 const reported=runtime!.hive.desktopConversationSwitchState(ws.id,responsibilityWorker.id);assert.equal(reported.pendingReports,1);assert.deepEqual(reported.openDispatches,{queued:0,submitted:0});await stop(responsibilityWorker.id);await stop(`${ws.id}:orchestrator`);await runtime!.close();runtime=undefined;db.close();db=undefined;release();release=undefined;
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{}});db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});const coldReported=runtime.hive.desktopConversationSwitchState(ws.id,responsibilityWorker.id);assert.equal(coldReported.pendingReports,1);assert.equal(coldReported.revision,reported.revision);const reportBlockers=await assertGate('PENDING_REPORTS');
 await writeFile(join(root,'receipt.json'),JSON.stringify({pass:true,root,realGit:true,realHive:true,realAcpMcp:true,indexFailureSameChild:true,readyFailurePendingColdRecovery:true,snapshotClaimNoCodeMutation:true,stoppedSubmittedBlocked:true,coldReportedOutboxBlocked:true,reportBlockers,noAbortOrManualOutboxRepair:true},null,2));console.log('HANDOFF_RUNTIME_FAILURES_OK '+root);
}catch(error){await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),stack:error instanceof Error?error.stack:undefined},null,2));throw error;}
finally{release?.();db?.close();await runtime?.close();globalThis.fetch=originalFetch;for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}console.log('Evidence retained '+root);}
