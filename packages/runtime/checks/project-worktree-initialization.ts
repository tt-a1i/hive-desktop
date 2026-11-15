/** Real Git and native shell processes, without model or account calls. POSIX only. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync} from 'node:fs';
import {mkdtemp, mkdir, readFile, realpath, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {DatabaseSync} from 'node:sqlite';
import type {ChatIndex} from '../src/chat-index.js';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
if (process.platform === 'win32') throw new Error('POSIX check');
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-setup-'))), project=join(dir,'project'), data=join(dir,'data');
await mkdir(project); const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:project,stdio:'pipe'});
git('init','--template='); await writeFile(join(project,'source'),'root'); git('add','.'); git('commit','-m','fixture');
const wait=async <T>(read:()=>T|Promise<T>, ready:(v:T)=>boolean,label:string):Promise<T>=>{const until=Date.now()+15000; for(;;){const v=await read();if(ready(v))return v;if(Date.now()>until)throw new Error(`Timed out: ${label}`);await new Promise(done=>setTimeout(done,20));}};
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}}); const rt=runtime;
 const ws=await rt.hive.request('POST','/api/workspaces',{path:project,name:'Setup',autostart_orchestrator:false}) as {id:string};
 const save=async(name:string,command:string)=>{const before=await rt.projectScripts.list(ws.id);const after=await rt.projectScripts.save(ws.id,before.revision,{name,command,runOnWorktreeCreate:true});return after.scripts.find(s=>s.name===name)!;};
 const root=await rt.chats.createRecord(ws.id,'codex');assert.equal(await rt.initialization.get(ws.id,root.id),null);
 const disabled=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'});assert.equal((await rt.initialization.get(ws.id,disabled.id))?.status,'disabled');
 const setup=await save('Original','pwd > actual-cwd; printf original > result; while [ ! -f release ]; do sleep 0.05; done');
 const worker=rt.createMember(ws.id,{name:'worker-fixture',role:'coder',backend:'codex'});
 const index=(rt.chats as unknown as {index:ChatIndex}).index;const workerB=await rt.chats.createMemberWorktree(ws.id,worker.id,{skipSetup:true});index.bind(workerB.id,worker.id);
 const db=new DatabaseSync(join(data,'hive','runtime.sqlite'));const count=(table:string)=>(db.prepare('SELECT COUNT(*) AS n FROM '+table).get() as {n:number}).n;
 const beforeLedger=['messages','dispatches','agent_runs'].map(count);
 const workerA=await rt.chats.createMemberWorktree(ws.id,worker.id);const workerSetup=await wait(()=>rt.initialization.get(ws.id,workerA.id),s=>s?.status==='running' && !!s.runId,'worker setup');assert.ok(workerSetup);
 assert.equal(index.forAgent(ws.id,worker.id)?.id,workerB.id);assert.deepEqual(['messages','dispatches','agent_runs'].map(count),beforeLedger);
 index.bind(workerA.id,worker.id);
 const workspaces=await rt.hive.request('GET','/api/workspaces') as Parameters<typeof rt.chats.start>[0]['workspace'][];const workspace=workspaces.find(item=>item.id===ws.id)!;
 await assert.rejects(rt.chats.start({workspace,agentId:worker.id,runId:'fixture',token:'fixture',hivePort:String(rt.hive.port),config:{command:'hive:codex',args:[]},onExit(){}}),/WORKTREE_INITIALIZATION_BLOCKED|Worktree setup/);
 assert.deepEqual(['messages','dispatches','agent_runs'].map(count),beforeLedger);const workerCurrent=await rt.initialization.get(ws.id,workerA.id);assert.ok(workerCurrent);await rt.initialization.skip(ws.id,workerA.id,workerCurrent.version);db.close();
 const a=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'}); const first=await rt.initialization.get(ws.id,a.id); assert.ok(first);
 const b=await rt.chats.createRecord(ws.id,'pi',{isolation:'worktree'});const extra=await Promise.all((['pi-rpc','claude'] as const).map(backend=>rt.chats.createRecord(ws.id,backend,{isolation:'worktree'})));await Promise.all(extra.map(record=>runningExtra(record.id)));
 async function runningExtra(id:string){return wait(()=>rt.initialization.get(ws.id,id),s=>s?.status==='running' && !!s.runId,'extra running');}
 const ap=await rt.chats.executionDirectory(ws.id,a.id),bp=await rt.chats.executionDirectory(ws.id,b.id);
 const running=(id:string)=>wait(()=>rt.initialization.get(ws.id,id),s=>s?.status==='running' && !!s.runId,'running');
 const [ar,br]=await Promise.all([running(a.id),running(b.id)]);assert.ok(ar?.runId && br?.runId);
 await wait(()=>existsSync(join(ap,'result')),Boolean,'A command');await wait(()=>existsSync(join(bp,'result')),Boolean,'B command');
 assert.equal((await readFile(join(ap,'actual-cwd'),'utf8')).trim(),ap);assert.equal((await readFile(join(bp,'actual-cwd'),'utf8')).trim(),bp);assert.equal(existsSync(join(project,'result')),false);
 const before=await rt.projectScripts.list(ws.id);const edited=await rt.projectScripts.save(ws.id,before.revision,{id:setup.id,name:'Edited',command:'printf changed > result',runOnWorktreeCreate:true});await rt.projectScripts.remove(ws.id,edited.revision,setup.id);
 assert.equal(rt.scriptRuns.get(ws.id,ar.runId).run.script.command,setup.command);assert.deepEqual(ar.script,{id:setup.id,version:setup.version,name:setup.name});
 await Promise.all([a.id,b.id,...extra.map(record=>record.id)].map(async id=>{const [opened]=await Promise.all([rt.chats.open(id),assert.rejects(rt.chats.prompt(id,'blocked'),/WORKTREE_INITIALIZATION_BLOCKED|Worktree setup/),assert.rejects(rt.chats.resume(id),/WORKTREE_INITIALIZATION_BLOCKED|Worktree setup/)]);assert.ok('readOnly' in opened && opened.readOnly);assert.ok('identityError' in opened);assert.equal(opened.identityError.code,'WORKTREE_INITIALIZATION_BLOCKED');}));
 for(const item of extra){const state=await rt.initialization.get(ws.id,item.id);assert.ok(state);await rt.initialization.skip(ws.id,item.id,state.version);}
 await writeFile(join(ap,'release'),'go');await wait(()=>rt.initialization.get(ws.id,a.id),s=>s?.status==='succeeded','success');assert.equal(await readFile(join(ap,'result'),'utf8'),'original');
 const bs=await rt.initialization.get(ws.id,b.id);assert.ok(bs);assert.equal((await rt.initialization.skip(ws.id,b.id,bs.version))?.status,'skipped');assert.equal(rt.scriptRuns.get(ws.id,br.runId).run.status,'cancelled');await assert.rejects(rt.initialization.skip(ws.id,b.id,bs.version));
 const failure=await save('Failure','printf failed > failed-marker; exit 7');const c=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'});
 const failed=await wait(()=>rt.initialization.get(ws.id,c.id),s=>s?.status==='failed','nonzero');assert.ok(failed?.runId);assert.equal(rt.scriptRuns.get(ws.id,failed.runId).run.exitCode,7);
 // A changed ownership marker or malformed v3 setup provenance must prevent retry and skip.
 const cp=await rt.chats.executionDirectory(ws.id,c.id);const gitDir=execFileSync('git',['rev-parse','--absolute-git-dir'],{cwd:cp,encoding:'utf8'}).trim();const marker=join(gitDir,'hive-desktop-owner.json');const markerRaw=await readFile(marker,'utf8');await writeFile(marker,JSON.stringify({version:1,chatId:c.id,ownerToken:'wrong'}));
 await assert.rejects(rt.initialization.retry(ws.id,c.id,failed.version));await assert.rejects(rt.initialization.skip(ws.id,c.id,failed.version));await writeFile(marker,markerRaw);
 const branch=execFileSync('git',['branch','--show-current'],{cwd:cp,encoding:'utf8'}).trim();execFileSync('git',['checkout','-b','fixture-mismatch'],{cwd:cp,stdio:'pipe'});await assert.rejects(rt.initialization.retry(ws.id,c.id,failed.version));await assert.rejects(rt.initialization.skip(ws.id,c.id,failed.version));execFileSync('git',['checkout',branch],{cwd:cp,stdio:'pipe'});
 const intentPath=join(data,'worktrees',c.id,'intent.json'),intentRaw=await readFile(intentPath,'utf8');const intent=JSON.parse(intentRaw);assert.equal(intent.version,3);intent.creation.setup.script.command='';await writeFile(intentPath,JSON.stringify(intent));
 await assert.rejects(rt.initialization.retry(ws.id,c.id,failed.version));await assert.rejects(rt.initialization.skip(ws.id,c.id,failed.version));assert.equal(await readFile(intentPath,'utf8'),JSON.stringify(intent));await writeFile(intentPath,intentRaw);
 const after=await rt.projectScripts.list(ws.id);await rt.projectScripts.remove(ws.id,after.revision,failure.id);await rt.initialization.retry(ws.id,c.id,failed.version);
 const retried=await wait(()=>rt.initialization.get(ws.id,c.id),s=>s?.status==='failed' && s.runId!==failed.runId,'retry');assert.ok(retried?.runId);assert.equal(rt.scriptRuns.get(ws.id,retried.runId).run.script.command,failure.command);
 await save('Signal','kill -TERM $$');const signalled=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'});
 const signalState=await wait(()=>rt.initialization.get(ws.id,signalled.id),s=>s?.status==='failed','signal');assert.ok(signalState?.runId);assert.equal(rt.scriptRuns.get(ws.id,signalState.runId).run.signal,15);
 // Deleting a workspace must stop its real silent setup process and its descendants.
 const other=join(dir,'other');await mkdir(other);execFileSync('git',['init','--template='],{cwd:other,stdio:'pipe'});await writeFile(join(other,'source'),'other');execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false','add','.'],{cwd:other,stdio:'pipe'});execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false','commit','-m','fixture'],{cwd:other,stdio:'pipe'});
 const ws2=await rt.hive.request('POST','/api/workspaces',{path:other,name:'Deletion',autostart_orchestrator:false}) as {id:string};
 await rt.projectScripts.save(ws2.id,'0',{name:'Delete',command:'printf started > started; (sleep 2; printf orphan > orphan) & wait',runOnWorktreeCreate:true});
 const deleting=await rt.chats.createRecord(ws2.id,'codex',{isolation:'worktree'});const deletingPath=await rt.chats.executionDirectory(ws2.id,deleting.id);await wait(()=>existsSync(join(deletingPath,'started')),Boolean,'delete setup started');
 await rt.hive.request('DELETE',`/api/workspaces/${ws2.id}`);await new Promise(done=>setTimeout(done,2200));assert.equal(existsSync(join(deletingPath,'orphan')),false);
 await save('Silent','(sleep 2; printf orphan > orphan) & wait');const d=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'});const dp=await rt.chats.executionDirectory(ws.id,d.id);const dr=await running(d.id);assert.ok(dr?.runId);
 await rt.close();runtime=undefined;await new Promise(done=>setTimeout(done,2200));assert.equal(existsSync(join(dp,'orphan')),false);
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});const cold=await runtime.initialization.get(ws.id,d.id);assert.equal(cold?.status,'cancelled');assert.equal(runtime.scriptRuns.get(ws.id,dr.runId).run.status,'cancelled');assert.equal((await runtime.initialization.get(ws.id,a.id))?.status,'succeeded');
 const statePath=join(data,'worktree-initialization',createHash('sha256').update(JSON.stringify([ws.id,d.id])).digest('hex')+'.json');await writeFile(statePath,'{corrupt');await assert.rejects(runtime.initialization.get(ws.id,d.id),/unreadable/);assert.equal(await readFile(statePath,'utf8'),'{corrupt');assert.equal(existsSync(join(dp,'orphan')),false);
 console.log('WORKTREE_INITIALIZATION_OK: real Git A/B, native setup, blocked agent calls, snapshot edit/delete/retry, nonzero/signal, skip/delete/close cleanup, cold recovery and corrupt state. POSIX only.');
} finally {await runtime?.close();await rm(dir,{recursive:true,force:true});}
