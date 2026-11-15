/** Real Hive HTTP/SQLite and team/controller protocols with controlled Node peers.
 * SQL is read-only; no vendor CLI, accounts, model, manual drain or task repair. */
import assert from 'node:assert/strict';
import {createHash,randomUUID} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {chmod,mkdir,mkdtemp,readFile,realpath,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,delimiter} from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {createTeamTool} from '../src/team-tool.js';
import {callHiveMcpTool} from '@tt-a1i/hive/dist/src/cli/hive-mcp.js';
import {createDesktopConversationSwitchState} from '@tt-a1i/hive/dist/src/server/desktop-conversation-switch-state.js';
import SqliteDatabase from '@tt-a1i/hive/dist/src/server/sqlite.js';

const directory=await realpath(await mkdtemp(join(tmpdir(),'hive-conversation-switch-'))),repo=join(directory,'repo'),data=join(directory,'state');
const keys=['GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM','PI_CODING_AGENT_DIR','PI_OFFLINE','PATH'] as const;
const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
const originalFetch=globalThis.fetch;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined,db:DatabaseSync|undefined,release:(()=>void)|undefined;
const wait=async(check:()=>Promise<boolean>|boolean,label:string)=>{const deadline=Date.now()+15000;while(!await check()){if(Date.now()>deadline)throw Error('Timed out '+label);await new Promise(resolve=>setTimeout(resolve,15));}};
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd,stdio:'pipe'});
const sql=(statement:string,...values:any[])=>db!.prepare(statement).all(...values) as any[];
const databaseHash=()=>{const hash=createHash('sha256');for(const {name} of sql("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")){assert.match(name,/^[A-Za-z_][A-Za-z_0-9]*$/);hash.update(name);for(const row of sql(`SELECT * FROM ${name}`).map(value=>JSON.stringify(value)).sort())hash.update(row);}return hash.digest('hex');};
const fileHashes=async()=>{const result:Record<string,string>={};for(const name of ['runtime.sqlite','runtime.sqlite-wal']){try{result[name]=createHash('sha256').update(await readFile(join(data,'hive',name))).digest('hex');}catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;}}return result;};
const checked=(workspace:string,agent?:string)=>{
 const before=databaseHash(),ledger=runtime!.hive.executionLedgerRevision();
 const facts=runtime!.hive.desktopConversationSwitchState(workspace,agent);
 assert.equal(databaseHash(),before,'Snapshot must not change any database table');assert.equal(runtime!.hive.executionLedgerRevision(),ledger);
 assert.equal(facts.deliverySemantics,'enqueued-not-consumed');assert.equal(facts.readScope,'hive-ledger-only');return facts;
};
try{
 await mkdir(repo);await mkdir(join(directory,'pi'));await writeFile(join(directory,'gitconfig'),'');
 Object.assign(process.env,{GIT_CONFIG_GLOBAL:join(directory,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1',PI_CODING_AGENT_DIR:join(directory,'pi'),PI_OFFLINE:'1'});
 globalThis.fetch=(input,init)=>{const url=new URL(typeof input==='string'?input:input instanceof URL?input:input.url);if(!['127.0.0.1','localhost','::1'].includes(url.hostname))return Promise.reject(Error('Fixture forbids external fetch'));return originalFetch(input,init);};
 const peer=await readFile(new URL('./fixtures/codex-queue-peer.cjs',import.meta.url),'utf8');
 const delayed=peer.replace("{save();answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});}","{save();let c={};try{c=JSON.parse(fs.readFileSync('queue-peer-control.json','utf8'));}catch{};setTimeout(()=>answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'}),c.startupDelayMs||0);}");assert.notEqual(delayed,peer);
 const prepare=async(path:string)=>{await mkdir(path,{recursive:true});await writeFile(join(path,'package.json'),'{"type":"commonjs"}');await writeFile(join(path,'app-server'),delayed);await writeFile(join(path,'queue-peer-control.json'),'{"complete":true}');git(path,'init','--template=');git(path,'add','.');git(path,'commit','-qm','controlled fixture');};
 await prepare(repo);
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:repo,name:'Switch facts',autostart_orchestrator:false}) as {id:string};
 const otherRepo=join(directory,'other');await prepare(otherRepo);const other=await runtime.hive.request('POST','/api/workspaces',{path:otherRepo,name:'Other',autostart_orchestrator:false}) as {id:string};
 const lead=await runtime.chats.create(ws.id,'codex');await runtime.chats.resume(lead.id);
 const worker=runtime.createMember(ws.id,{name:'snapshot-worker',role:'coder',backend:'codex'}),unrelated=runtime.createMember(ws.id,{name:'unrelated',role:'coder',backend:'codex'}),foreign=runtime.createMember(other.id,{name:'foreign',role:'coder',backend:'codex'});
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});
 assert.equal(checked(ws.id).eligible,true);assert.equal(checked(ws.id,worker.id).eligible,true);
 assert.throws(()=>runtime!.hive.desktopConversationSwitchState('missing'),{code:'DESKTOP_SWITCH_WORKSPACE_UNKNOWN'});
 for(const agent of ['missing',foreign.id])assert.throws(()=>runtime!.hive.desktopConversationSwitchState(ws.id,agent),{code:'DESKTOP_SWITCH_AGENT_UNKNOWN'});
 for(const invalid of ['',null,{},42])assert.throws(()=>runtime!.hive.desktopConversationSwitchState(invalid as any),{code:'DESKTOP_SWITCH_INVALID_ARGUMENT'});
 const command=async(workspace:string,agent:string,args:string[])=>{const tool=createTeamTool({workspaceId:workspace,agentId:agent,token:runtime!.agents.peekAgentToken(agent)!,hivePort:String(runtime!.hive.port),cwd:repo});const answer=await tool.execute('switch-state-fixture',{args},undefined,undefined,undefined as never);assert.ok(!answer.isError,JSON.stringify(answer));const text=answer.content.map(item=>'text'in item?item.text:'').join('')||'null';try{return JSON.parse(text);}catch{if(args[0]==='report')return {notice:text};return JSON.parse(text.split('\n')[0]!);}};
 const orchestrator=`${ws.id}:orchestrator`;
 const sent=await command(ws.id,orchestrator,['send','snapshot-worker','real queued responsibility']);const dispatch=sent.dispatch_id as string;
 assert.equal(checked(ws.id,worker.id).openDispatches.queued,1);assert.equal(checked(ws.id).eligible,false);assert.equal(checked(ws.id,unrelated.id).eligible,true);assert.equal(checked(other.id).eligible,true);
 const workerRecord=await runtime.chats.createMemberWorktree(ws.id,worker.id);await runtime.chats.open(workerRecord.id);await runtime.chats.resume(workerRecord.id);
 const idle=async(id:string)=>{const snapshot=await runtime!.chats.open(id);return !snapshot.busy&&snapshot.pending===0;};
 await wait(()=>idle(workerRecord.id),'worker receives queued responsibility');assert.equal(checked(ws.id,worker.id).openDispatches.submitted,1);
 release=runtime.executionMutations.acquire(ws.id);await command(ws.id,orchestrator,['message','--dispatch',dispatch,'--kind','note','pending snapshot note']);
 const queued=checked(ws.id,worker.id);assert.equal(queued.pendingDispatchMessages.queued,1);const oldRevision=queued.revision;
 const filesBefore=await fileHashes();assert.equal(checked(ws.id,worker.id).revision,oldRevision);assert.deepEqual(await fileHashes(),filesBefore,'Snapshot must not write DB or WAL bytes');
 release();release=undefined;
 assert.equal(checked(ws.id,worker.id).pendingDispatchMessages.delivering,1,'Automatic drain claims while awaiting production runtime delivery');
 await wait(()=>sql("SELECT state FROM dispatch_message_outbox o JOIN dispatch_messages m ON m.id=o.message_id WHERE m.text='pending snapshot note'")[0]?.state==='delivered','automatic message delivery');await wait(()=>idle(workerRecord.id),'message native completion');assert.equal(checked(ws.id,worker.id).pendingDispatchMessages.queued,0);assert.notEqual(checked(ws.id,worker.id).revision,oldRevision);
 const question=await command(ws.id,worker.id,['message','--dispatch',dispatch,'--to','orchestrator','--kind','question','need a real answer']);
 const questionId=question.id??question.message?.id;assert.ok(questionId,JSON.stringify(question));
 await wait(()=>idle(lead.id),'question delivered');assert.equal(checked(ws.id,worker.id).pendingQuestions,1);assert.equal(checked(ws.id,orchestrator).pendingQuestions,1);
 await command(ws.id,orchestrator,['message','--dispatch',dispatch,'--kind','answer','--reply-to',questionId,'explicit answer']);await wait(()=>idle(workerRecord.id),'answer delivered');assert.equal(checked(ws.id,worker.id).pendingQuestions,0);
 const seen=await command(ws.id,worker.id,['messages','--dispatch',dispatch]);release=runtime.executionMutations.acquire(ws.id);
 await command(ws.id,worker.id,['report','real report not yet delivered','--dispatch',dispatch,'--seen',String(seen.required_seen_seq)]);
 const reported=checked(ws.id,worker.id);assert.deepEqual(reported.openDispatches,{queued:0,submitted:0});assert.equal(reported.pendingReports,1);assert.equal(reported.eligible,false);assert.ok(reported.blockers.some(item=>item.code==='PENDING_REPORTS'));
 // Retain a genuinely reported-but-undelivered responsibility across a full
 // runtime restart, without manufacturing or repairing an outbox row.
 const beforeCold=reported.revision;const closedHost=runtime.hive;await runtime.close();runtime=undefined;db.close();db=undefined;release();release=undefined;
 assert.throws(()=>closedHost.desktopConversationSwitchState(ws.id),{code:'DESKTOP_SWITCH_CLOSING'});
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}});db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});
 const coldReported=checked(ws.id,worker.id);assert.equal(coldReported.pendingReports,1);assert.equal(coldReported.eligible,false);assert.equal(coldReported.revision,beforeCold);
 await runtime.chats.open(lead.id);await runtime.chats.resume(lead.id);
 await wait(()=>sql('SELECT delivered_at FROM report_outbox WHERE dispatch_id=?',dispatch)[0]?.delivered_at!==null,'report enqueue acknowledgement');await wait(()=>idle(lead.id),'report native completion');assert.equal(checked(ws.id,worker.id).eligible,true);
 // A real external controller uses a controlled codex queue shim. It never
 // contacts the user's Codex App or a vendor account.
 if(process.platform==='win32')throw Error('Controller shim fixture requires POSIX; no Windows acceptance is claimed');
 const bin=join(directory,'bin');await mkdir(bin);const notifier=join(bin,'codex'),notificationLog=join(directory,'notifications');
 await writeFile(notifier,`#!${process.execPath}\nrequire('fs').appendFileSync(${JSON.stringify(notificationLog)},JSON.stringify(process.argv.slice(2))+'\\n');\n`);await chmod(notifier,0o700);process.env.PATH=bin+delimiter+(previous.PATH??'');
 const externalRepo=join(directory,'external');await prepare(externalRepo);const external=await runtime.hive.request('POST','/api/workspaces',{path:externalRepo,name:'Controller facts',controller_mode:'codex_app',autostart_orchestrator:false}) as {id:string};
 const threadId=randomUUID(),baseUrl=`http://127.0.0.1:${runtime.hive.port}`;
 const controller=async(input:Record<string,unknown>)=>callHiveMcpTool('hive.controller_action',{workspace_id:external.id,...input},{baseUrl,metadata:{threadId}}) as Promise<any>;
 const requested=await callHiveMcpTool('hive.controller_connect',{workspace_id:external.id},{baseUrl,metadata:{threadId}}) as any;
 assert.equal(checked(external.id).controller.pendingConnection,true);
 await runtime.hive.request('POST',`/api/workspaces/${external.id}/controller/confirm`,{request_id:requested.pending_request.id});assert.equal(checked(external.id).controller.pendingConnection,false);
 const externalWorker=runtime.createMember(external.id,{name:'external-worker',role:'coder',backend:'codex'});const externalRecord=await runtime.chats.createMemberWorktree(external.id,externalWorker.id);await runtime.chats.open(externalRecord.id);await runtime.chats.resume(externalRecord.id);
 const externalSent=await controller({action:'send',worker_name:'external-worker',text:'controller responsibility',operation_id:randomUUID()});const externalDispatch=externalSent.dispatch_id as string;await wait(()=>idle(externalRecord.id),'controller dispatch');
 const externalQuestion=await command(external.id,externalWorker.id,['message','--dispatch',externalDispatch,'--to','orchestrator','--kind','question','controller must answer']);const externalQuestionId=externalQuestion.id??externalQuestion.message?.id;
 assert.ok(externalQuestionId);const unread=checked(external.id,externalWorker.id);assert.equal(unread.pendingQuestions,1);assert.equal(unread.controllerReceipts,1);assert.equal(unread.controller.unreadReceipts,1);
 const received=await controller({action:'read_reports'});assert.equal(received.reports.length,1);const read=checked(external.id);assert.equal(read.controllerReceipts,1);assert.equal(read.controller.readUnacknowledgedReceipts,1);assert.equal(read.pendingDispatchMessages.queued,0);
 await controller({action:'ack_reports',report_ids:received.reports.map((item:any)=>item.id)});const acknowledged=checked(external.id);assert.equal(acknowledged.controllerReceipts,0);assert.equal(acknowledged.pendingQuestions,1,'Receipt acknowledgement does not answer the question');
 await controller({action:'message',dispatch_id:externalDispatch,kind:'answer',reply_to:externalQuestionId,text:'controller answer',operation_id:randomUUID()});await wait(()=>idle(externalRecord.id),'controller answer delivery');assert.equal(checked(external.id).pendingQuestions,0);
 const externalSeen=await command(external.id,externalWorker.id,['messages','--dispatch',externalDispatch]);await command(external.id,externalWorker.id,['report','controller report','--dispatch',externalDispatch,'--seen',String(externalSeen.required_seen_seq)]);
 assert.equal(checked(external.id).pendingReports,1);assert.equal(checked(external.id).controllerReceipts,1);assert.deepEqual(checked(external.id).openDispatches,{queued:0,submitted:0});
 const externalReports=await controller({action:'read_reports'});await controller({action:'ack_reports',report_ids:externalReports.reports.map((item:any)=>item.id)});
 await wait(()=>sql("SELECT count(*) n FROM report_outbox WHERE workspace_id=? AND notification_state='sending'",external.id)[0].n===0,'controlled notifier completion');assert.equal(checked(external.id,externalWorker.id).eligible,true);
 // Pending controller operations are observed through the real asynchronous
 // start API, before its controlled app-server handshake completes.
 await controller({action:'stop',worker_name:'external-worker',operation_id:randomUUID()});
 const externalPath=(await runtime.chats.sessionExecution(externalRecord.id)).path;await writeFile(join(externalPath,'queue-peer-control.json'),'{"complete":true,"startupDelayMs":350}');
 const starting=controller({action:'start',worker_name:'external-worker',operation_id:randomUUID()});await wait(()=>sql("SELECT count(*) n FROM controller_operations WHERE workspace_id=? AND state='pending'",external.id)[0].n===1,'pending controller start');assert.equal(checked(external.id,externalWorker.id).controller.pendingOperations,1);await starting;assert.equal(checked(external.id).controller.pendingOperations,0);
 assert.ok((await readFile(notificationLog,'utf8')).includes('queue'),'Only the controlled notification shim is used');
 // Inspect the same production SQL reader through a read-only connection to
 // verify transaction reuse, secret-table exclusion and explicit read failure.
 const connection=new SqliteDatabase(join(data,'hive/runtime.sqlite'),{readOnly:true});const statements:string[]=[];const prepareOriginal=connection.prepare.bind(connection);connection.prepare=(statement:string)=>{assert.equal(connection.connection.isTransaction,true,'Every read belongs to one SQLite snapshot');statements.push(statement);return prepareOriginal(statement);};
 const reader=createDesktopConversationSwitchState(connection,()=>false);connection.exec('BEGIN');let nested=false;const transaction=connection.transaction;connection.transaction=()=>{nested=true;throw Error('Must not nest a transaction');};
 try{assert.equal(reader(ws.id,worker.id).eligible,true);assert.equal(nested,false);}finally{connection.exec('ROLLBACK');connection.transaction=transaction;}
 assert.ok(!statements.some(statement=>/\b(?:settings|app_state|ui_tokens|auth)\b/i.test(statement)));
 connection.prepare=()=>{throw Error('Controlled SQLite read failure');};assert.throws(()=>reader(ws.id),{code:'DESKTOP_SWITCH_UNAVAILABLE'});connection.close();assert.throws(()=>reader(ws.id),{code:'DESKTOP_SWITCH_CLOSING'});
 const host=runtime.hive;await runtime.close();runtime=undefined;assert.throws(()=>host.desktopConversationSwitchState(ws.id),{code:'DESKTOP_SWITCH_CLOSING'});
 console.log('DESKTOP_CONVERSATION_SWITCH_STATE_OK: actual queued/submitted/reported responsibility, note/question/receipt/controller boundaries, immutable reads and explicit unknown/closing failures');
}finally{release?.();try{db?.close();await runtime?.close();}finally{globalThis.fetch=originalFetch;for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}await rm(directory,{recursive:true,force:true,maxRetries:5});}}
