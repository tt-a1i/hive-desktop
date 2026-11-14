/** Real Hive/Git/Pi SDK, deliberately paused delivery and one controlled queue executor. No model calls. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdir,mkdtemp,readFile,realpath,rename,rm,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {createTeamTool} from '../src/team-tool.js';
import {DispatchExecutionOrigins} from '../src/dispatch-execution-origins.js';
import type {Chats} from '../src/chats.js';
import type {ChatIndex} from '../src/chat-index.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';

const directory=await realpath(await mkdtemp(join(tmpdir(),'hive-historical-identity-'))),repo=join(directory,'repo'),data=join(directory,'state'),pi=join(directory,'pi');
const keys=['PI_CODING_AGENT_DIR','PI_OFFLINE','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM'] as const;
const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
let modelRequests=0;
const server=createServer((_,response)=>{modelRequests++;response.writeHead(500).end('Unexpected inference');});
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const internals=(chats:Chats)=>chats as unknown as {index:ChatIndex;live:Map<string,{queue:DeliveryQueue}>};
try {
 await mkdir(repo);await mkdir(pi);await writeFile(join(directory,'gitconfig'),'');
 process.env.PI_CODING_AGENT_DIR=pi;process.env.PI_OFFLINE='1';process.env.GIT_CONFIG_GLOBAL=join(directory,'gitconfig');process.env.GIT_CONFIG_NOSYSTEM='1';
 server.listen(0,'127.0.0.1');await once(server,'listening');const address=server.address();assert.ok(address&&typeof address!=='string');
 await writeFile(join(pi,'models.json'),JSON.stringify({providers:{openai:{api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'fixture-only',models:[{id:'gpt-6.1-terra',name:'No-inference metadata',input:['text']}]}}}));
 await writeFile(join(pi,'settings.json'),JSON.stringify({defaultProvider:'openai',defaultModel:'gpt-6.1-terra'}));
 const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd:repo,stdio:'pipe'});
 git('init','--template=');await writeFile(join(repo,'same.txt'),'base');git('add','.');git('commit','-qm','base');
 runtime=await createDesktopRuntime(data,()=>{});const first=runtime;
 const ws=await first.hive.request('POST','/api/workspaces',{path:repo,name:'Identity fixture',autostart_orchestrator:false}) as {id:string};
 const lead=await first.chats.create(ws.id);const leadQueue=internals(first.chats).live.get(lead.id)!.queue;leadQueue.resume=()=>{};await leadQueue.pause();await first.chats.resume(lead.id);
 const leadRun=first.agents.getActiveRunByAgentId(ws.id,`${ws.id}:orchestrator`)!.runId;
 const worker=first.createMember(ws.id,{name:'identity-worker',role:'coder',backend:'pi'});
 const a=await first.chats.createRecord(ws.id,'pi',{isolation:'worktree'});internals(first.chats).index.bind(a.id,worker.id);await first.chats.open(a.id);
 let qa=internals(first.chats).live.get(a.id)!.queue;qa.resume=()=>{};await qa.pause();
 await first.hive.request('POST',`/api/workspaces/${ws.id}/agents/${encodeURIComponent(worker.id)}/start`);
 const command=async(agentId:string,args:string[])=>JSON.parse((await createTeamTool({workspaceId:ws.id,agentId,token:first.agents.peekAgentToken(agentId)!,hivePort:String(first.hive.port),cwd:repo}).execute('identity-check',{args},undefined,undefined,undefined as never)).content.map(item=>'text'in item?item.text:'').join('')||'null');
 const sent=await command(`${ws.id}:orchestrator`,['send','identity-worker','original A responsibility']);const dispatchId=sent.dispatch_id as string;
 const inbox=await command(worker.id,['messages','--dispatch',dispatchId]);await command(worker.id,['report','A complete','--dispatch',dispatchId,'--seen',String(inbox.required_seen_seq),'--artifact','same.txt']);
 const aRoot=(await first.chats.sessionExecution(a.id)).path;await writeFile(join(aRoot,'same.txt'),'A result');
 const aRun=first.agents.getActiveRunByAgentId(ws.id,worker.id)!;first.agents.stopAgentRun(aRun.runId);assert.equal(await first.agents.waitForRunExit(aRun.runId,5000),true);
 const b=await first.chats.createRecord(ws.id,'pi',{isolation:'worktree'});internals(first.chats).index.bind(b.id,worker.id);await first.chats.open(b.id);
 const qb=internals(first.chats).live.get(b.id)!.queue, actualResume=qb.resume.bind(qb);qb.resume=()=>{};await qb.pause();
 await first.hive.request('POST',`/api/workspaces/${ws.id}/agents/${encodeURIComponent(worker.id)}/start`);
 const bRun=first.agents.getActiveRunByAgentId(ws.id,worker.id)!.runId;
 const source=await first.chats.dispatchSession(ws.id,dispatchId,worker.id);
 assert.equal(source.sourceSessionId,a.id);assert.equal(source.session.id,a.id);assert.equal(source.session.originAgentId,worker.id);assert.equal(source.session.agentId,undefined);
 assert.equal(source.session.executionPath,aRoot);assert.equal('sessionFile'in source.session,false);assert.equal('piSessionId'in source.session,false);
 await assert.rejects(first.chats.dispatchSession(ws.id,dispatchId,'wrong-worker'),/DISPATCH_SOURCE_MISMATCH/);
 await assert.rejects(first.chats.dispatchSession(ws.id,'not-recorded',worker.id),/DISPATCH_SOURCE_UNKNOWN/);
 await qb.enqueue('B queued user input',true,{version:1,origin:'user',displayText:'B queued user input'});
 await assert.rejects(first.chats.resume(a.id),/SESSION_ORIGIN_BUSY/);await assert.rejects(first.chats.prompt(a.id,'must stay worker'),/SESSION_ORIGIN_BUSY/);
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,b.id);assert.equal(first.agents.getActiveRunByAgentId(ws.id,worker.id)!.runId,bRun);assert.equal(internals(first.chats).live.has(a.id),false);
 await qb.pause();qb.discardPending(new Set(qb.pending.map(item=>item.id)));
 // Exercise a truly busy transport without allowing its SDK executor to infer.
 let release!:()=>void,entered!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;}),atHead=new Promise<void>(resolve=>{entered=resolve;});
 const options=(qb as unknown as {options:{execute:()=>Promise<void>}}).options,execute=options.execute;
 options.execute=async()=>{entered();await gate;};
 await qb.enqueue('controlled busy head',true,{version:1,origin:'user',displayText:'controlled busy head'});actualResume();await atHead;
 await assert.rejects(first.chats.resume(a.id),/SESSION_ORIGIN_BUSY/);assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,b.id);
 const paused=qb.pause();release();await paused;options.execute=execute;
 await first.chats.resume(a.id);
 assert.equal(internals(first.chats).index.forAgent(ws.id,worker.id)!.id,a.id);assert.equal(internals(first.chats).index.get(a.id).originAgentId,worker.id);
 assert.equal(internals(first.chats).index.get(b.id).agentId,undefined);assert.equal(internals(first.chats).index.get(b.id).originAgentId,worker.id);
 qa=internals(first.chats).live.get(a.id)!.queue;qa.resume=()=>{};await qa.pause();await first.chats.prompt(a.id,'Continue A as its original worker');
 assert.ok(qa.pending.some(item=>item.presentation?.displayText==='Continue A as its original worker'));
 assert.equal(internals(first.chats).index.get(a.id).agentId,worker.id);assert.equal(first.agents.getActiveRunByAgentId(ws.id,`${ws.id}:orchestrator`)!.runId,leadRun);
 await first.close();runtime=undefined;
 const file=join(data,'chats/index.json');const rows=JSON.parse(await readFile(file,'utf8')) as Record<string,unknown>[];
 const oldA=rows.find(row=>row.id===a.id)!;delete oldA.agentId;delete oldA.originAgentId;delete oldA.identityState;
 const oldB=rows.find(row=>row.id===b.id)!;oldB.agentId=worker.id;
 const unknown={...oldA,id:'legacy-unknown',title:'主会话'};delete unknown.execution;
 rows.push(unknown,{...unknown,id:'legacy-unreadable',sessionFile:join(directory,'missing-transcript')},
   {...unknown,id:'legacy-conflicting',agentId:`${ws.id}:orchestrator`});await writeFile(file,JSON.stringify(rows));
 new DispatchExecutionOrigins(join(data,'chats/dispatch-execution-origins')).put({workspaceId:ws.id,dispatchId:'legacy-conflict',sourceSessionId:'legacy-conflicting',sourceAgentId:worker.id});
 runtime=await createDesktopRuntime(data,()=>{});
 const resolved=await runtime.chats.dispatchSession(ws.id,dispatchId,worker.id);assert.equal(resolved.sourceSessionId,a.id);assert.equal(resolved.session.identityState,'known');assert.equal(resolved.session.originAgentId,worker.id);
 assert.equal(internals(runtime.chats).index.forAgent(ws.id,worker.id)!.id,b.id,'Source lookup never rebinds the current worker');
 const legacy=await runtime.chats.open('legacy-unknown');assert.equal(legacy.readOnly,true);assert.equal(legacy.identityError.code,'SESSION_IDENTITY_UNKNOWN');assert.ok(legacy.history.length>0);assert.equal(internals(runtime.chats).live.has('legacy-unknown'),false);
 await assert.rejects(runtime.chats.prompt('legacy-unknown','Do not become orchestrator'),/SESSION_IDENTITY_UNKNOWN/);await assert.rejects(runtime.chats.resume('legacy-unknown'),/SESSION_IDENTITY_UNKNOWN/);await assert.rejects(runtime.chats.setModel('legacy-unknown','openai','gpt-6.1-terra'),/SESSION_IDENTITY_UNKNOWN/);
 const unreadable=await runtime.chats.open('legacy-unreadable');assert.equal(unreadable.readOnly,true);assert.equal(unreadable.historyIncomplete,true);assert.ok(unreadable.historyWarning);
 const conflicting=await runtime.chats.open('legacy-conflicting');assert.equal(conflicting.readOnly,true);assert.equal(conflicting.identityError.code,'SESSION_IDENTITY_CONFLICT');
 await assert.rejects(runtime.chats.resume('legacy-conflicting'),/SESSION_IDENTITY_CONFLICT/);
 const otherRoot=join(directory,'other');await mkdir(otherRoot);const other=await runtime.hive.request('POST','/api/workspaces',{path:otherRoot,name:'Other',autostart_orchestrator:false}) as {id:string};
 await assert.rejects(runtime.chats.dispatchSession(other.id,dispatchId,worker.id),/DISPATCH_SOURCE_UNKNOWN/);
 new DispatchExecutionOrigins(join(data,'chats/dispatch-execution-origins')).put({workspaceId:other.id,dispatchId:'wrong-workspace-source',sourceSessionId:a.id,sourceAgentId:worker.id});
 await assert.rejects(runtime.chats.dispatchSession(other.id,'wrong-workspace-source',worker.id),/DISPATCH_SOURCE_MISMATCH/);
 await rename(aRoot,aRoot+'.missing');try{await assert.rejects(runtime.chats.dispatchSession(ws.id,dispatchId,worker.id),/DISPATCH_SOURCE_UNAVAILABLE/);}finally{await rename(aRoot+'.missing',aRoot);}
 await runtime.hive.request('DELETE',`/api/workspaces/${ws.id}/workers/${worker.id}`);
 const deleted=await runtime.chats.open(a.id);assert.equal(deleted.readOnly,true);assert.equal(deleted.identityError.code,'SESSION_ORIGIN_DELETED');assert.ok(deleted.history.length>0);assert.equal(internals(runtime.chats).live.has(a.id),false);
 await assert.rejects(runtime.chats.resume(a.id),/SESSION_ORIGIN_DELETED/);assert.equal(modelRequests,0);
 console.log('HISTORICAL_SESSION_IDENTITY_OK: real A→B worker rebinding, queue/busy guards, explicit A continuation stays worker, immutable dispatch navigation, exact legacy recovery, unknown/deleted read-only history and unavailable source refusal. Zero model requests.');
}finally{
 await runtime?.close();if(server.listening){server.close();await once(server,'close');}
 for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}await rm(directory,{recursive:true,force:true,maxRetries:5});
}
