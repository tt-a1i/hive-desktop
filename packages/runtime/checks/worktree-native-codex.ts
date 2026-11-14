/** Opt-in real Codex/Terra team worktrees. No inference unless --live is explicit. */
import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {randomUUID,createHash} from 'node:crypto';
import {mkdir,mkdtemp,readFile,writeFile,realpath,access} from 'node:fs/promises';
import {dirname,isAbsolute,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {DatabaseSync} from 'node:sqlite';
import {ChatIndex} from '../src/chat-index.js';
import {CodexTransport} from '../src/agents/codex-transport.js';
if(!process.argv.includes('--live')){console.log('WORKTREE_NATIVE_CODEX_SKIPPED: pass --live to request actual isolated openai/gpt-6.1-terra inference.');process.exit(0);}
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
const exec=promisify(execFile),command=process.env.HIVE_CODEX_COMMAND;
assert.ok(command&&isAbsolute(command),'Set HIVE_CODEX_COMMAND to the installed official Codex executable');
let packageName:string|undefined,search=dirname(await realpath(command));
for(let depth=0;depth<8;depth++){try{packageName=JSON.parse(await readFile(join(search,'package.json'),'utf8')).name;if(packageName==='@openai/codex')break;}catch{}const parent=dirname(search);if(parent===search)break;search=parent;}
assert.equal(packageName,'@openai/codex','The actual installed official @openai/codex CLI is required');
const version=(await exec(command,['--version'],{timeout:10000})).stdout.trim();
const repoRoot=fileURLToPath(new URL('../../../',import.meta.url)),evidence=join(repoRoot,'.artifacts/functional-parity/native-worktree');await mkdir(evidence,{recursive:true});
const directory=await realpath(await mkdtemp(join(evidence,'run-'))),data=join(directory,'state'),project=join(directory,'project');await mkdir(project);await mkdir(join(directory,'pi-empty'));
const priorPi={dir:process.env.PI_CODING_AGENT_DIR,offline:process.env.PI_OFFLINE};process.env.PI_CODING_AGENT_DIR=join(directory,'pi-empty');process.env.PI_OFFLINE='1';
const nonce=randomUUID(),workerName='worktree-proof-worker',workerNameFile=`worker-${nonce}.txt`,leadNameFile=`lead-${nonce}.txt`;
const workerBytes=`HIVE_WORKTREE_WORKER_${nonce}\n`,leadBytes=`HIVE_WORKTREE_LEAD_${nonce}\n`,finish=`HIVE_WORKTREE_TERRA_COMPLETE_${nonce}`;
const events:any[]=[],requests:any[]=[],threadSelections:any[]=[],transports=new Set<CodexTransport>(),selected=new Map<CodexTransport,{threadId:string;cwd:string;provider:string;model:string}>();
const original=CodexTransport.prototype.sendRequest;
CodexTransport.prototype.sendRequest=async function<T>(method:string,params:unknown):Promise<T>{
 transports.add(this);const input=params as Record<string,unknown>;requests.push({at:Date.now(),phase,method,...(typeof input.threadId==='string'&&{threadId:input.threadId}),...(typeof input.cwd==='string'&&{cwd:input.cwd}),...(typeof input.model==='string'&&{model:input.model}),...(typeof input.modelProvider==='string'&&{provider:input.modelProvider})});
 if(method==='turn/start'||method==='turn/steer'){const model=selected.get(this);assert.ok(model,'Model selection must precede every actual native turn');assert.equal(model.provider,'openai');assert.equal(model.model,'gpt-6.1-terra');}
 const response=await original.call(this,method,params) as T;
 if(method==='thread/start'||method==='thread/resume'){const value=response as any;assert.equal(value.modelProvider,'openai');assert.equal(value.model,'gpt-6.1-terra');selected.set(this,{threadId:value.thread.id,cwd:value.thread.cwd,provider:value.modelProvider,model:value.model});threadSelections.push({phase,threadId:value.thread.id,cwd:value.thread.cwd,provider:value.modelProvider,model:value.model});}
 return response;
};
let phase='setup',runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined,leadId='',workerId='',workspaceId='',workerAgent='';
const receipt:any={pass:false,directory,command,version,model:'openai/gpt-6.1-terra',actualCodex:true,vendorInference:false,guiValidated:false};
const boot=()=>createDesktopRuntime(data,event=>events.push({...event,observedAt:Date.now(),phase}),{executableEnvironment:{HIVE_CODEX_COMMAND:command}});
const save=(name:string,value:unknown)=>writeFile(join(directory,name),JSON.stringify(value,null,2),{mode:0o600});
const inferenceCount=()=>requests.filter(request=>request.method==='turn/start'||request.method==='turn/steer').length;
const sha=(bytes:Buffer)=>createHash('sha256').update(bytes).digest('hex');
const assistantHas=(snapshot:any,needle:string)=>snapshot.history.some((message:any)=>message.role==='assistant'&&message.content?.some((part:any)=>part.type==='text'&&part.text?.includes(needle)));
async function ledger(){const db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});try{return{workspaces:db.prepare('SELECT id,path FROM workspaces').all(),dispatches:db.prepare('SELECT id,workspace_id,from_agent_id,to_agent_id,status,seen_seq,reported_at,report_text,artifacts FROM dispatches WHERE workspace_id=?').all(workspaceId),outbox:db.prepare('SELECT id,workspace_id,target_agent_id,dispatch_id,delivered_at FROM report_outbox WHERE workspace_id=?').all(workspaceId),messages:db.prepare('SELECT dispatch_id,sequence,from_agent_id,recipient_agent_id,kind FROM dispatch_messages WHERE workspace_id=?').all(workspaceId)}}finally{db.close();}}
async function cleanup(){if(!runtime)return;const closing=runtime.close();let timer:ReturnType<typeof setTimeout>|undefined;const forced=new Promise<void>(resolve=>{timer=setTimeout(()=>{void Promise.allSettled([...transports].map(transport=>transport.close(200))).then(()=>resolve());},12000);});await Promise.race([closing,forced]);clearTimeout(timer);await closing;runtime=undefined;}
try{
 const gitEnv={...process.env,GIT_CONFIG_GLOBAL:process.platform==='win32'?'NUL':'/dev/null',GIT_CONFIG_NOSYSTEM:'1'};
 const git=(args:string[])=>exec('git',['-c','user.name=Worktree Fixture','-c','user.email=worktree-fixture@example.invalid','-c','commit.gpgSign=false','-c',`core.hooksPath=${process.platform==='win32'?'NUL':'/dev/null'}`,...args],{cwd:project,env:gitEnv,timeout:10000});
 await git(['init','--template=','--initial-branch=main']);await writeFile(join(project,'README.md'),'Isolated real Codex worktree team proof. Only the requested marker files may be created.\n');await git(['add','--','README.md']);await git(['commit','-m','isolated fixture']);
 runtime=await boot();const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Native worktree team proof',autostart_orchestrator:false}) as {id:string};workspaceId=workspace.id;
 const member=runtime.createMember(workspaceId,{name:workerName,role:'coder',backend:'codex',description:'Use only your assigned session worktree. Complete exactly one dispatch, explicitly acknowledge seen and report the absolute artifact path. Do not create more members.'});workerAgent=member.id;
 const lead=await runtime.chats.createRecord(workspaceId,'codex',{isolation:'worktree'}),worker=await runtime.chats.createRecord(workspaceId,'codex',{isolation:'worktree'});leadId=lead.id;workerId=worker.id;
 runtime.hive.configureAgentLaunch(workspaceId,workerAgent,{command:'hive:codex',args:['--provider','openai','--model','gpt-6.1-terra']});
 await cleanup();
 // Bind the new isolated records before any native session opens. Hive remains
 // the sole member/dispatch ledger; this is test setup, not a claim of GUI coverage.
 const index=new ChatIndex(join(data,'chats'));index.bind(leadId,`${workspaceId}:orchestrator`);index.bind(workerId,workerAgent);
 for(const id of [leadId,workerId])index.save({...index.get(id),model:{provider:'openai',id:'gpt-6.1-terra',name:'Terra worktree proof'}});
 runtime=await boot();const leadExecution=await runtime.chats.sessionExecution(leadId),workerExecution=await runtime.chats.sessionExecution(workerId);assert.equal(leadExecution.kind,'worktree');assert.equal(workerExecution.kind,'worktree');assert.notEqual(leadExecution.path,workerExecution.path);assert.notEqual(leadExecution.path,project);assert.notEqual(workerExecution.path,project);
 const workerFile=join(workerExecution.path,workerNameFile),leadFile=join(leadExecution.path,leadNameFile);
 Object.assign(receipt,{workspaceId,leadId,workerId,workerAgent,leadExecution,workerExecution,workerFile,leadFile,finish,setupUsesExplicitIndexBinding:true});
 const initial=await runtime.chats.open(leadId);assert.equal(initial.model?.provider,'openai');assert.equal(initial.model?.id,'gpt-6.1-terra');assert.equal(initial.executionPath,leadExecution.path);assert.equal(inferenceCount(),0);
 // A manually configured worker intentionally remains stopped until explicitly
 // started; team send is not a promise to auto-start user-managed members.
 phase='live';receipt.vendorInference=true;const deadline=Date.now()+180000;
 await runtime.chats.resume(workerId);
 for(;;){const ready=await runtime.chats.open(workerId);assert.equal(ready.model?.provider,'openai');assert.equal(ready.model?.id,'gpt-6.1-terra');assert.equal(ready.executionPath,workerExecution.path);assert.ok(runtime.agents.getActiveRunByAgentId(workspaceId,workerAgent),'Owned worker must have an active native run');if(!ready.busy&&ready.pending===0)break;assert.ok(Date.now()<deadline,'Owned worker startup exceeded this attempt deadline');assert.ok(!ready.interactions?.some(item=>item.state==='pending'),'Worker startup needs human interaction');await delay(100);}
 receipt.workerExplicitlyStartedBeforeDispatch=true;receipt.startupInferenceRequests=inferenceCount();

 const workerTask=`This is one actual Hive worktree proof. Your team workspace remains ${JSON.stringify(project)} but your execution directory is ${JSON.stringify(workerExecution.path)}. Use a real local shell tool to run pwd, then create ONLY the relative file ${JSON.stringify(workerNameFile)} in your current execution directory, with exact UTF-8 bytes ${JSON.stringify(workerBytes)}. Read that actual file back with a real tool and verify bytes. Do not write in the team root or orchestrator directory. Use the registered team tool: inspect team messages for the active dispatch and obtain required_seen_seq, then explicitly team report with --dispatch <actual id>, --seen <required_seen_seq>, --artifact ${JSON.stringify(workerFile)}. Include the absolute artifact path in report text. Do not call a provider/account CLI or create any members. Prefer one native turn. After reporting finish your turn.`;
 const prompt=`Actual two-worktree collaboration proof. You are the orchestrator at ${JSON.stringify(leadExecution.path)}; the shared Hive team workspace is ${JSON.stringify(project)}. An existing Codex Terra worker named ${workerName} is already assigned its own worktree ${JSON.stringify(workerExecution.path)}. Do NOT create another worker or perform its file task. Use the registered team tool (never team in bash) to send exactly one dispatch to ${workerName}, with this task text:\n${workerTask}\nAfter dispatching, stop your initial turn and wait for the normal explicit worker report; do not poll, sleep, resend, or claim completion early. When its report arrives, independently use an actual local tool to read the absolute artifact ${JSON.stringify(workerFile)}, print its bytes, and verify exact content ${JSON.stringify(workerBytes)}. Only after this actual read, run pwd and write ONLY your relative proof ${JSON.stringify(leadNameFile)} in your own current execution directory with exact UTF-8 bytes ${JSON.stringify(leadBytes)}. Use normal team report/ack protocol as required, preserving the one dispatch. Then reply exactly ${finish}. Do not run Git commands, create extra members, or run network/account commands or edits outside the two marker files. Your continuation after the worker report is expected; no new user prompt will be sent.`;
 await runtime.chats.prompt(leadId,prompt);receipt.explicitOrchestratorPrompts=1;
 let completed=false;
 while(Date.now()<deadline){const a=await runtime.chats.open(leadId);const b=runtime.chats.list(workspaceId).find(record=>record.id===workerId)?.codexThreadId?await runtime.chats.open(workerId):undefined;
  assert.ok(!a.interactions?.some(item=>item.state==='pending')&&!b?.interactions?.some(item=>item.state==='pending'),'Native interaction needs human input; the check does not grant permissions');
  if(assistantHas(a,finish)&&!a.busy&&!a.pending&&b&&!b.busy&&!b.pending){completed=true;break;}
  const errors=events.filter(event=>event.phase==='live'&&event.type==='error');if(errors.length&&!a.busy&&!b?.busy)throw new Error(`Native execution failed: ${errors.map(event=>event.message).join('; ')}`);
  await delay(200);
 }
 assert.ok(completed,'Two-role native worktree proof exceeded its single 180-second attempt');
 const beforeLead=await runtime.chats.open(leadId),beforeWorker=await runtime.chats.open(workerId);const actualWorker=await readFile(workerFile),actualLead=await readFile(leadFile);assert.equal(actualWorker.toString('utf8'),workerBytes);assert.equal(actualLead.toString('utf8'),leadBytes);
 for(const root of [project,leadExecution.path,workerExecution.path]){if(root!==workerExecution.path)await assert.rejects(access(join(root,workerNameFile)));if(root!==leadExecution.path)await assert.rejects(access(join(root,leadNameFile)));}
 assert.ok(beforeLead.history.some((message:any)=>message.role==='toolResult'&&JSON.stringify(message.toolInput).includes(workerFile)&&JSON.stringify(message.content).includes(workerBytes.trim())),'Orchestrator must actually read worker file with a native tool, not only repeat the report');
 const team=await runtime.hive.request('GET',`/api/ui/workspaces/${workspaceId}/team`),beforeLedger=await ledger();assert.equal(beforeLedger.workspaces.length,1);assert.equal(beforeLedger.dispatches.length,1);const dispatch=beforeLedger.dispatches[0] as any;assert.equal(dispatch.workspace_id,workspaceId);assert.equal(dispatch.from_agent_id,`${workspaceId}:orchestrator`);assert.equal(dispatch.to_agent_id,workerAgent);assert.equal(dispatch.status,'reported');assert.ok(dispatch.reported_at);assert.ok(dispatch.report_text.includes(workerFile));assert.ok(JSON.parse(dispatch.artifacts).includes(workerFile));assert.ok(dispatch.seen_seq>=0);
 const independentReads=events.filter(event=>event.sessionId===leadId&&event.type==='tool_start'&&event.toolName==='commandExecution'&&JSON.stringify(event.args).includes(workerFile));assert.ok(independentReads.length&&independentReads.every(event=>event.observedAt>=dispatch.reported_at),'Native orchestrator read must occur after the authoritative worker report');
 const outbox=beforeLedger.outbox.filter((item:any)=>item.dispatch_id===dispatch.id);assert.equal(outbox.length,1);assert.equal((outbox[0] as any).target_agent_id,`${workspaceId}:orchestrator`);assert.ok((outbox[0] as any).delivered_at);assert.equal(beforeLead.pendingReports.length,0);
 const reportMessages=beforeLead.history.filter((message:any)=>message.presentation?.origin==='report'&&message.presentation?.dispatchId===dispatch.id);assert.equal(reportMessages.length,1);assert.equal((reportMessages[0] as any).presentation.workerId,workerAgent);assert.ok((reportMessages[0] as any).presentation.artifacts.includes(workerFile));
 const messagesCall=events.find(event=>event.type==='tool_start'&&event.sessionId===workerId&&event.toolName==='team'&&event.args?.args?.[0]==='messages');assert.ok(messagesCall,'Worker must read actual dispatch messages before acknowledging seen');
 const reportCall=events.find(event=>event.type==='tool_start'&&event.sessionId===workerId&&event.toolName==='team'&&event.args?.args?.[0]==='report');assert.ok(reportCall?.args.args.includes('--seen'),'Worker must explicitly supply --seen to its actual team report call');
 for(const snapshot of [beforeLead,beforeWorker]){assert.equal(snapshot.model?.provider,'openai');assert.equal(snapshot.model?.id,'gpt-6.1-terra');assert.ok(snapshot.codexThreadId);assert.equal(snapshot.busy,false);assert.equal(snapshot.pending,0);}
 const selectedPaths=[...selected.values()].map(item=>item.cwd);assert.ok(selectedPaths.includes(leadExecution.path)&&selectedPaths.includes(workerExecution.path));
 await save('before.json',{team,ledger:beforeLedger,lead:beforeLead,worker:beforeWorker});const requestsBefore=inferenceCount();await cleanup();phase='cold';runtime=await boot();const restoredLead=await runtime.chats.open(leadId),restoredWorker=await runtime.chats.open(workerId);await delay(1000);
 assert.equal(inferenceCount(),requestsBefore,'Cold open must not start or steer a new model turn');
 for(const [old,current]of[[beforeLead,restoredLead],[beforeWorker,restoredWorker]]as const){assert.equal(current.codexThreadId,old.codexThreadId);assert.deepEqual(current.execution,old.execution);assert.equal(current.executionPath,old.executionPath);assert.deepEqual(current.historyMessageIds,old.historyMessageIds);assert.equal(current.pending,old.pending);assert.deepEqual(current.pendingReports,old.pendingReports);assert.equal(current.busy,false);}
 for(const [threadId,path]of[[beforeLead.codexThreadId,leadExecution.path],[beforeWorker.codexThreadId,workerExecution.path]])assert.ok(threadSelections.some(item=>item.phase==='cold'&&item.threadId===threadId&&item.cwd===path),'Native resumed thread must retain its original execution cwd');
 const coldReports=restoredLead.history.filter((message:any)=>message.presentation?.origin==='report'&&message.presentation?.dispatchId===dispatch.id);assert.equal(coldReports.length,1);assert.deepEqual((coldReports[0] as any).presentation,(reportMessages[0] as any).presentation);
 const afterLedger=await ledger();assert.deepEqual(afterLedger,beforeLedger);assert.equal(runtime.chats.list(workspaceId).length,2);assert.equal(await readFile(workerFile,'utf8'),workerBytes);assert.equal(await readFile(leadFile,'utf8'),leadBytes);await save('after.json',{ledger:afterLedger,lead:restoredLead,worker:restoredWorker});
 Object.assign(receipt,{pass:true,dispatchId:dispatch.id,leadThreadId:beforeLead.codexThreadId,workerThreadId:beforeWorker.codexThreadId,workerSha256:sha(actualWorker),leadSha256:sha(actualLead),teamRootUntouched:true,independentToolRead:true,reportIdentityPreserved:true,outboxDelivered:true,coldNewInferenceRequests:inferenceCount()-requestsBefore,nativeTurnStarts:requests.filter(request=>request.method==='turn/start').length,nativeSteers:requests.filter(request=>request.method==='turn/steer').length,nativeTurnsByRole:{lead:requests.filter(request=>request.method==='turn/start'&&request.threadId===beforeLead.codexThreadId).length,worker:requests.filter(request=>request.method==='turn/start'&&request.threadId===beforeWorker.codexThreadId).length},independentReadAfterReport:true});
}catch(error){receipt.error=error instanceof Error?error.message:String(error);receipt.stack=error instanceof Error?error.stack:undefined;process.exitCode=1;try{if(runtime&&workspaceId)await save('failure-ledger.json',await ledger());}catch{} }
finally{
 phase='cleanup';if(runtime){await Promise.allSettled([leadId,workerId].filter(Boolean).map(id=>runtime!.chats.abort(id)));await cleanup().catch(error=>{receipt.cleanupError=String(error);receipt.pass=false;process.exitCode=1;});}
 await Promise.allSettled([...transports].map(transport=>transport.close(200)));
 const lifecycle=[...transports].map(transport=>(transport as unknown as {process:{diagnostics:unknown}}).process.diagnostics);await save('native-process-lifecycle.json',lifecycle);
 receipt.nativeProcessesClosed=lifecycle.every((item:any)=>item.closed===true);
 if(!receipt.nativeProcessesClosed){receipt.pass=false;receipt.cleanupError='An owned native process did not close';process.exitCode=1;}
 if(requests.some(request=>['cold','cleanup'].includes(request.phase)&&['turn/start','turn/steer'].includes(request.method))){receipt.pass=false;receipt.coldOrCleanupUnexpectedInference=true;process.exitCode=1;}
 CodexTransport.prototype.sendRequest=original;
 if(priorPi.dir===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=priorPi.dir;if(priorPi.offline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=priorPi.offline;
 await save('events.json',events);await save('native-requests.json',requests);await save('native-thread-selections.json',threadSelections);await save('receipt.json',receipt);await writeFile(join(evidence,'latest.json'),JSON.stringify({directory,pass:receipt.pass,receipt:join(directory,'receipt.json')},null,2),{mode:0o600});console.log(JSON.stringify(receipt));
}
