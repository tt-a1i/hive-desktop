/** Production Hive HTTP/SQLite/team CLI, automatic release drains and cold recovery.
 * The executable is Node with a controlled JSONL peer, never a vendor CLI or model.
 * SQLite is read-only throughout; input and reports use authenticated production team tools. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,mkdir,readFile,writeFile,rm,realpath} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {createTeamTool} from '../src/team-tool.js';
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-review-sqlite-outbox-'))),repo=join(dir,'repo'),data=join(dir,'state');
const envKeys=['GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM','PI_CODING_AGENT_DIR','PI_OFFLINE'] as const;
const old=Object.fromEntries(envKeys.map(k=>[k,process.env[k]]));
const fetchOriginal=globalThis.fetch;globalThis.fetch=(input,init)=>{const u=new URL(typeof input==='string'?input:input instanceof URL?input:input.url);if(!['localhost','127.0.0.1','::1'].includes(u.hostname))throw Error('Review forbids external fetch');return fetchOriginal(input,init);};
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Review','-c','user.email=review@example.invalid','-c','commit.gpgSign=false',...args],{cwd:repo,encoding:'utf8',stdio:'pipe'}).trim();

let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;let db:DatabaseSync|undefined;let release:(()=>void)|undefined;
const boot=()=>createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:process.execPath}});
const queries=async(path:string)=>{try{return(await readFile(join(path,'queue-peer-queries.jsonl'),'utf8')).trim().split('\n').filter(Boolean).map(x=>JSON.parse(x));}catch(e){if((e as any).code==='ENOENT')return [];throw e;}};
const wait=async(check:()=>Promise<boolean>,label:string)=>{const end=Date.now()+12000;while(!await check()){if(Date.now()>end)throw Error('Timed out '+label);await new Promise(r=>setTimeout(r,15));}};
try{
 await mkdir(repo);await mkdir(join(dir,'pi'));await writeFile(join(dir,'gitconfig'),'');
 Object.assign(process.env,{GIT_CONFIG_GLOBAL:join(dir,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1',PI_CODING_AGENT_DIR:join(dir,'pi'),PI_OFFLINE:'1'});
 await writeFile(join(repo,'package.json'),'{"type":"commonjs"}');await writeFile(join(repo,'app-server'),await readFile(new URL('./fixtures/codex-queue-peer.cjs',import.meta.url)));await writeFile(join(repo,'queue-peer-control.json'),'{"complete":true}');git('init','--template=');git('add','.');git('commit','-qm','review fixture');
 runtime=await boot();const ws=await runtime.hive.request('POST','/api/workspaces',{path:repo,name:'outbox-review',autostart_orchestrator:false}) as {id:string};
 const lead=await runtime.chats.create(ws.id,'codex');await runtime.chats.resume(lead.id);
 const worker=runtime.createMember(ws.id,{name:'review-worker',role:'coder',backend:'codex'});
 const workerRecord=await runtime.chats.createMemberWorktree(ws.id,worker.id);await runtime.chats.open(workerRecord.id);await runtime.chats.resume(workerRecord.id);
 const workerPath=(await runtime.chats.sessionExecution(workerRecord.id)).path;
 const command=async(agent:string,args:string[])=>{const t=createTeamTool({workspaceId:ws.id,agentId:agent,token:runtime!.agents.peekAgentToken(agent)!,hivePort:String(runtime!.hive.port),cwd:repo});const r=await t.execute('review-command',{args},undefined,undefined,undefined as never);assert.ok(!r.isError,JSON.stringify(r));return JSON.parse(r.content.map(x=>'text'in x?x.text:'').join('')||'null');};
 const sent=await command(`${ws.id}:orchestrator`,['send','review-worker','explicit-review-dispatch']);const dispatchId=sent.dispatch_id;assert.ok(dispatchId);
 await wait(async()=>{const s=await runtime!.chats.open(workerRecord.id);return !s.busy&&s.pending===0;},'worker completion');
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});const sql=(q:string,...a:any[])=>db!.prepare(q).all(...a) as any[];
 assert.equal(sql('SELECT status FROM dispatches WHERE id=?',dispatchId)[0].status,'submitted','Native turn completion cannot report the dispatch');
 const oldInbox=await command(worker.id,['messages','--dispatch',dispatchId]);assert.ok(Number.isSafeInteger(oldInbox.required_seen_seq));
 const workerQueriesBefore=(await queries(workerPath)).length;release=runtime.executionMutations.acquire(ws.id);
 await command(`${ws.id}:orchestrator`,['message','--dispatch',dispatchId,'--kind','note','lease-protected-note']);
 const noteSql="SELECT o.state,o.delivered_at,m.id FROM dispatch_message_outbox o JOIN dispatch_messages m ON m.id=o.message_id WHERE m.dispatch_id=? AND m.text='lease-protected-note'";
 assert.equal(sql(noteSql,dispatchId).length,1);assert.equal(sql(noteSql,dispatchId)[0].state,'queued');
 await new Promise(r=>setTimeout(r,70));assert.equal(sql(noteSql,dispatchId)[0].state,'queued');assert.equal((await queries(workerPath)).length,workerQueriesBefore);
 release();release=undefined;
 // The release listener must perform the claim and delivery. No manual drain/list.
 await wait(async()=>sql(noteSql,dispatchId)[0].state==='delivered','automatic dispatch-message drain');
 await wait(async()=>(await queries(workerPath)).some(x=>x.input.some((p:any)=>p.text?.startsWith('<hive-message kind="note"')&&p.text.includes('lease-protected-note'))),'automatic note native execution');
 assert.equal((await queries(workerPath)).filter(x=>x.input.some((p:any)=>p.text?.startsWith('<hive-message kind="note"')&&p.text.includes('lease-protected-note'))).length,1);
 await wait(async()=>!(await runtime!.chats.open(workerRecord.id)).busy,'note completion');
 assert.equal(sql('SELECT status FROM dispatches WHERE id=?',dispatchId)[0].status,'submitted');
 await assert.rejects(command(worker.id,['report','stale report must fail','--dispatch',dispatchId,'--seen',String(oldInbox.required_seen_seq)]),/seen|unread|message/i);
 assert.equal(sql('SELECT status FROM dispatches WHERE id=?',dispatchId)[0].status,'submitted');assert.equal(sql('SELECT count(*) n FROM report_outbox WHERE dispatch_id=?',dispatchId)[0].n,0);
 console.log('DISPATCH_MESSAGE_AUTO_RELEASE_OK: actual SQLite queued row unclaimed under lease; release alone delivered and executed exactly once; stale --seen report rejected');
 // Exit cleanly with a real durable queued message while delivery remains gated.
 release=runtime.executionMutations.acquire(ws.id);await command(`${ws.id}:orchestrator`,['message','--dispatch',dispatchId,'--kind','note','cold-pending-note']);
 const coldSql="SELECT o.state,o.delivered_at,m.id FROM dispatch_message_outbox o JOIN dispatch_messages m ON m.id=o.message_id WHERE m.dispatch_id=? AND m.text='cold-pending-note'";
 assert.equal(sql(coldSql,dispatchId)[0].state,'queued');await runtime.close();runtime=undefined;db.close();db=undefined;release();release=undefined;
 runtime=await boot();db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});assert.equal(sql(coldSql,dispatchId)[0].state,'queued');
 await runtime.chats.open(lead.id);await runtime.chats.resume(lead.id);await runtime.chats.open(workerRecord.id);await runtime.chats.resume(workerRecord.id);
 await wait(async()=>sql(coldSql,dispatchId)[0].state==='delivered','cold pending automatic dispatch-message delivery');
 const coldNative=async()=>(await queries(workerPath)).filter(x=>x.input.some((p:any)=>p.text?.startsWith('<hive-message kind="note"')&&p.text.includes('cold-pending-note')));
 await wait(async()=>(await coldNative()).length===1,'cold pending native delivery');
 await wait(async()=>!(await runtime!.chats.open(workerRecord.id)).busy&&!(await runtime!.chats.open(lead.id)).busy,'cold recovery idle');
 assert.equal(sql('SELECT count(*) n FROM dispatches')[0].n,1);assert.equal(sql('SELECT status FROM dispatches WHERE id=?',dispatchId)[0].status,'submitted');
 console.log('SQLITE_PENDING_COLD_OK: production-enqueued queued note survives close/reopen; normal worker start redrains same message exactly once without new dispatch');

 const inbox=await command(worker.id,['messages','--dispatch',dispatchId]);assert.ok(inbox.required_seen_seq>oldInbox.required_seen_seq);

 const before=await queries(repo);release=runtime.executionMutations.acquire(ws.id);
 const reported=await command(worker.id,['report','explicit-review-report','--dispatch',dispatchId,'--seen',String(inbox.required_seen_seq),'--artifact','proof.txt']);
 assert.equal(sql('SELECT status FROM dispatches WHERE id=?',dispatchId)[0].status,'reported');
 let out=sql('SELECT * FROM report_outbox WHERE dispatch_id=?',dispatchId);assert.equal(out.length,1);assert.equal(out[0].delivered_at,null);
 await new Promise(r=>setTimeout(r,70));assert.equal((await queries(repo)).length,before.length);assert.equal(sql('SELECT delivered_at FROM report_outbox WHERE dispatch_id=?',dispatchId)[0].delivered_at,null);
 console.log('LEASE_HELD: production report enqueue durable; no native execution; no delivered acknowledgement');
 release();release=undefined;
 // Only read SQLite and peer evidence here. No list/messages/drain/manual wakeup.
 await wait(async()=>sql('SELECT delivered_at FROM report_outbox WHERE dispatch_id=?',dispatchId)[0].delivered_at!==null,'automatic outbox delivery');
 await wait(async()=>(await queries(repo)).some(x=>JSON.stringify(x).includes('explicit-review-report')),'automatic native report execution');
 const reports=(await queries(repo)).filter(x=>JSON.stringify(x).includes('explicit-review-report'));assert.equal(reports.length,1);assert.equal(sql('SELECT count(*) n FROM dispatches')[0].n,1);assert.equal(sql('SELECT count(*) n FROM report_outbox WHERE dispatch_id=?',dispatchId)[0].n,1);
 await wait(async()=>!(await runtime!.chats.open(lead.id)).busy,'report queue completion');
 const beforeCold=(await queries(repo)).length;await runtime.close();runtime=undefined;db.close();db=undefined;
 runtime=await boot();await runtime.chats.open(lead.id);await runtime.chats.resume(lead.id);
 await wait(async()=>!(await runtime!.chats.open(lead.id)).busy,'cold completion');
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'),{readOnly:true});assert.equal(sql('SELECT count(*) n FROM dispatches')[0].n,1);assert.equal(sql('SELECT count(*) n FROM report_outbox WHERE dispatch_id=? AND delivered_at IS NOT NULL',dispatchId)[0].n,1);
 assert.equal((await queries(repo)).filter(x=>x.input.some((p:any)=>p.text?.startsWith('<hive-message kind=\"report\"'))).length,1);const reopened=await runtime.chats.open(lead.id);assert.equal(sql(noteSql,dispatchId)[0].state,'delivered');assert.equal((await queries(workerPath)).filter(x=>x.input.some((p:any)=>p.text?.startsWith('<hive-message kind="note"')&&p.text.includes('lease-protected-note'))).length,1);assert.equal((await coldNative()).length,1);assert.equal(reopened.history.filter((m:any)=>m.presentation?.origin==='report'&&m.presentation?.dispatchId===dispatchId).length,1);
 // A cold recovery summary may mention a prior report as context. It is not a second
 // report delivery: exact report envelopes, report presentation cards and ledger rows
 // above each remain one, independently of summary text or total query count.
 assert.ok(reopened.history.some((m:any)=>m.presentation?.origin==='recovery'),'Normal cold-start recovery remains visible separately from the actual report');
 console.log('SQLITE_OUTBOX_AUTO_RELEASE_OK: real CLI seen/report contract; release alone delivers once; cold resume preserves one dispatch/report and no duplicate report execution');
 console.log(JSON.stringify({dir,dispatchId,leadQueriesBefore:before.length,leadQueriesAfterCold:(await queries(repo)).length,beforeCold,workerQueries:(await queries(workerPath)).length}));
}finally{release?.();try{db?.close();await runtime?.close();}finally{globalThis.fetch=fetchOriginal;for(const[k,v]of Object.entries(old)){if(v===undefined)delete process.env[k];else process.env[k]=v;}await rm(dir,{recursive:true,force:true,maxRetries:5});}}
