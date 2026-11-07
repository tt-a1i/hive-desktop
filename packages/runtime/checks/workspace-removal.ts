/** Real Hive HTTP/SQLite and original external Pi process; no model turn. */
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, mkdir, writeFile, readFile, stat, rename, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash, randomUUID } from 'node:crypto';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';
const directory=await mkdtemp(join(tmpdir(),'hive-workspace-removal-'));const data=join(directory,'state');
const projectA=join(directory,'project-a'),projectB=join(directory,'project-b'),projectC=join(directory,'project-c'),projectD=join(directory,'project-d'),projectE=join(directory,'project-e'),projectF=join(directory,'project-f');
for(const p of [projectA,projectB,projectC,projectD,projectE,projectF])await mkdir(join(p,'.hive'),{recursive:true});
await writeFile(join(projectA,'owned-marker.txt'),'project files survive');await writeFile(join(projectA,'.hive','tasks.md'),'# Tasks\n');
const pidfile=join(directory,'pi.pid'),wrapper=join(directory,'pi.sh');const quote=(s:string)=>"'"+s.replaceAll("'","'\\''")+"'";
await writeFile(wrapper,`#!/bin/sh\nprintf '%s\\n' "$$" > ${quote(pidfile)}\nexec ${quote(process.env.HIVE_PI_COMMAND??'/Users/tsk/Library/pnpm/pi')} --provider openai --model gpt-6.1-terra "$@"\n`,{mode:0o700});
const events:unknown[]=[];const runtime=await createDesktopRuntime(data,event=>events.push(event),{executableEnvironment:{}});
const live=(runtime.chats as unknown as {live:Map<string,{handle:PiSessionHandle;queue:DeliveryQueue}>}).live;
const db=new DatabaseSync(join(data,'hive','runtime.sqlite'));
const create=async(path:string)=>runtime.hive.request('POST','/api/workspaces',{path,name:path.split('/').at(-1),autostart_orchestrator:false}) as Promise<{id:string}>;
const exists=async(id:string)=>(await runtime.hive.request('GET','/api/workspaces') as {id:string}[]).some(w=>w.id===id);
const removal=async(id:string)=>(runtime as unknown as {removeWorkspace(id:string):Promise<{ok:boolean;removedSessionIds:string[]}>}).removeWorkspace(id);
const error=(code:string,workspaceRemoved:boolean|null)=>(e:unknown)=>{const x=e as {code?:string;details?:{workspaceRemoved?:boolean|null}};return x.code===code&&x.details?.workspaceRemoved===workspaceRemoved;};
try {
 await runtime.settings.update({piCommand:wrapper,codexCommand:null,claudeCommand:null});
 const a=await create(projectA),b=await create(projectB),c=await create(projectC),d=await create(projectD),e=await create(projectE),f=await create(projectF);
 await runtime.hive.request('POST',`/api/workspaces/${a.id}/workers`,{name:'owned-worker',role:'coder',autostart:false});
 assert.equal((db.prepare('SELECT COUNT(*) AS n FROM workers WHERE workspace_id=?').get(a.id) as {n:number}).n,1);
 const pi=await runtime.chats.create(a.id,'pi');const unbound=await runtime.chats.create(a.id,'pi-rpc');const unboundPid=Number((await readFile(pidfile,'utf8')).trim());process.kill(unboundPid,0);const external=await runtime.chats.create(a.id,'pi-rpc');const other=await runtime.chats.create(b.id,'pi');
 await runtime.chats.setModel(external.id,'openai','gpt-6.1-terra');
 const pid=Number((await readFile(pidfile,'utf8')).trim());process.kill(pid,0);
 // Fresh first start has no prior run/recovery delivery; startup instructions are context-only.
 assert.equal(live.get(external.id)!.queue.pending.length,0);assert.equal(runtime.agents.getActiveRunByAgentId(a.id,`${a.id}:orchestrator`),undefined);
 await runtime.chats.resume(external.id);const run=runtime.agents.getActiveRunByAgentId(a.id,`${a.id}:orchestrator`);assert.ok(run);
 const queue=live.get(external.id)!.queue;await queue.pause();assert.equal(queue.pending.length,0);assert.equal(queue.busy,false);
 const nativeFile=(await runtime.chats.open(external.id)).sessionFile;assert.ok(nativeFile);const nativeBefore=await readFile(nativeFile!);
 await queue.enqueue('owned unsent delivery',true);assert.equal(queue.busy,false);assert.equal(queue.pending.length,1);
 const spool=join(data,'chats','deliveries',createHash('sha256').update(external.id).digest('hex')+'.json');const pending=await readFile(spool,'utf8');assert.match(pending,/owned unsent delivery/);
 // Start an actual pure workflow through the authenticated Hive team route; it never calls an agent.
 await runtime.hive.request('PUT','/api/settings/app-state/workflow.enabled',{value:'true'});
 const token=runtime.agents.peekAgentToken(`${a.id}:orchestrator`);assert.ok(token);
 const source="export const meta = { name: 'owned-workflow', description: 'No model', maxDurationMs: 30000 }; await phase('owned phase'); await log('owned workflow ready'); const end = Date.now() + 15000; while (Date.now() < end) {} return 'finished';";
 const startWorkflow=()=>runtime.hive.request('POST','/api/team/workflow/run',{project_id:a.id,from_agent_id:`${a.id}:orchestrator`,token,source}) as Promise<{run_id:string}>;
 const workflow=await startWorkflow();const runId=workflow.run_id;
 const waitFor=async<T>(fn:()=>Promise<T>,ready:(x:T)=>boolean)=>{const deadline=Date.now()+10000;for(;;){const value=await fn();if(ready(value))return value;if(Date.now()>deadline)throw Error('workflow wait timed out');await new Promise(r=>setTimeout(r,10));}};
 await waitFor(()=>runtime.hive.request('GET',`/api/workflows/runs/${runId}/logs`) as Promise<{logs:unknown[]}>,value=>value.logs.length>0);
 await assert.rejects(removal(a.id),error('WORKSPACE_REMOVAL_BLOCKED',false));assert.equal(await exists(a.id),true);assert.equal(runtime.chats.list(a.id).length,3);process.kill(pid,0);
 // Exercise the actual Hive workflow lease and HTTP entry point with a fresh start attempt.
 let releaseLease!:()=>void;let leaseEntered!:()=>void;const entered=new Promise<void>(r=>leaseEntered=r),release=new Promise<void>(r=>releaseLease=r);
 const lease=(runtime.hive as unknown as {withWorkspaceWorkflowIdle<T>(id:string,fn:()=>Promise<T>):Promise<T>}).withWorkspaceWorkflowIdle;
 // A busy workflow cannot acquire the lease; after stop the same seam is checked below.
 await assert.rejects(lease(a.id,async()=>{}));
 await runtime.hive.request('POST',`/api/workflows/runs/${runId}/stop`);
 await waitFor(()=>runtime.hive.request('GET',`/api/workflows/runs/${runId}`) as Promise<{run:{status:string}}>,value=>value.run.status==='stopped');
 // Wait for actual worker cleanup, not just the persisted stopped status.
 const leaseTask=async()=>lease(a.id,async()=>{leaseEntered();await release;});
 let holding:Promise<void>|undefined;let stoppedCleanupRejections=0;const deadline=Date.now()+10000;
 while(!holding){try{const attempt=leaseTask();await Promise.race([entered,attempt]);holding=attempt;}catch(e){stoppedCleanupRejections++;if(Date.now()>deadline)throw e;await new Promise(r=>setTimeout(r,10));}}
 try{await assert.rejects(startWorkflow());}finally{releaseLease();await holding;}
 // Historical statuses do not prove current ownership; these rows represent terminal history.
 const now=Date.now();for(const status of ['interrupted','stopped','completed','failed'])db.prepare('INSERT INTO workflow_runs (id,workspace_id,script_path,name,status,started_at,created_at) VALUES (?,?,?,?,?,?,?)').run(randomUUID(),a.id,'historical','historical',status,now,now);
 const pendingAtDeletion=await readFile(spool,'utf8');assert.equal(JSON.parse(pendingAtDeletion)[0].id,JSON.parse(pending)[0].id);
 await assert.rejects(removal('nonexistent-'+randomUUID()));
 const deleted=await removal(a.id);assert.equal(deleted.ok,true);assert.deepEqual(new Set(deleted.removedSessionIds),new Set([pi.id,unbound.id,external.id]));assert.equal(await exists(a.id),false);
 assert.throws(()=>process.kill(pid,0),(e:unknown)=>(e as NodeJS.ErrnoException).code==='ESRCH');assert.throws(()=>process.kill(unboundPid,0),(e:unknown)=>(e as NodeJS.ErrnoException).code==='ESRCH');assert.equal(runtime.agents.getActiveRunByAgentId(a.id,`${a.id}:orchestrator`),undefined);assert.equal(runtime.chats.list(a.id).length,0);
 await assert.rejects(runtime.chats.open(external.id));await assert.rejects(runtime.chats.respondInteraction(external.id,'stale',{decision:'cancel'}));
 assert.equal(await exists(b.id),true);assert.equal((await runtime.chats.open(other.id)).id,other.id);
 assert.equal(await readFile(join(projectA,'owned-marker.txt'),'utf8'),'project files survive');assert.equal(await readFile(join(projectA,'.hive','tasks.md'),'utf8'),'# Tasks\n');assert.deepEqual(await readFile(nativeFile!),nativeBefore);assert.equal(await readFile(spool,'utf8'),pendingAtDeletion);
 assert.equal((db.prepare('SELECT COUNT(*) AS n FROM agent_runs WHERE agent_id=?').get(`${a.id}:orchestrator`) as {n:number}).n,0);
 for(const table of ['workspaces','workflow_runs','workers'])assert.equal((db.prepare(`SELECT COUNT(*) AS n FROM ${table} WHERE ${table==='workspaces'?'id':'workspace_id'}=?`).get(a.id) as {n:number}).n,0);
 const cchat=await runtime.chats.create(c.id,'pi');
 db.exec(`CREATE TRIGGER removal_test_fail BEFORE DELETE ON workspaces WHEN OLD.id='${c.id}' BEGIN SELECT RAISE(ABORT,'owned fault'); END`);
 await assert.rejects(removal(c.id),error('WORKSPACE_REMOVAL_FAILED',false));assert.equal(await exists(c.id),true);assert.ok(runtime.chats.list(c.id).some(x=>x.id===cchat.id));assert.equal((await runtime.chats.open(cchat.id)).id,cchat.id);db.exec('DROP TRIGGER removal_test_fail');
 // A delayed executable gates an actual catalog process, without monkeypatching runtime code.
 const gate=join(directory,'catalog-gate.sh');
 await writeFile(gate,`#!/bin/sh\nprintf started > "$PWD/catalog-started"\nwhile [ ! -f "$PWD/catalog-release" ]; do sleep 0.05; done\nif [ -f "$PWD/catalog-fail" ]; then exit 73; fi\nexec ${quote(process.env.HIVE_PI_COMMAND??'/Users/tsk/Library/pnpm/pi')} --provider openai --model gpt-6.1-terra "$@"\n`,{mode:0o700});
 await runtime.settings.update({piCommand:gate,codexCommand:null,claudeCommand:null});
 const waitFile=async(path:string)=>{const until=Date.now()+5000;for(;;){try{await stat(path);return;}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}if(Date.now()>until)throw Error('catalog gate timed out');await new Promise(r=>setTimeout(r,20));}};
 const eChat=await runtime.chats.create(e.id,'pi');await runtime.chats.setModel(eChat.id,'openai','gpt-6.1-terra');await runtime.chats.resume(eChat.id);const eToken=runtime.agents.peekAgentToken(`${e.id}:orchestrator`);assert.ok(eToken);
 const catalog=runtime.agentModels.list(e.id,'pi-rpc');void catalog.catch(()=>{});await waitFile(join(projectE,'catalog-started'));
 const removing=removal(e.id);void removing.catch(()=>{});
 await assert.rejects(runtime.agentModels.list(e.id,'pi'),error('WORKSPACE_REMOVAL_BLOCKED',false));
 await assert.rejects(runtime.chats.create(e.id,'pi'),error('WORKSPACE_REMOVAL_BLOCKED',false));
 assert.throws(()=>runtime.requestHive('POST',`/api/workspaces/${e.id}/workers`,{name:'blocked-worker',role:'coder'}),error('WORKSPACE_REMOVAL_BLOCKED',false));
 await assert.rejects(runtime.hive.request('POST','/api/team/workflow/run',{project_id:e.id,from_agent_id:`${e.id}:orchestrator`,token:eToken,source}),(e:unknown)=>(e as {status?:number}).status===409);
 assert.ok((await runtime.agentModels.list(b.id,'pi')).models.length>0);assert.equal(await exists(e.id),true);
 await writeFile(join(projectE,'catalog-release'),'release');assert.ok((await catalog).models.length>0);assert.equal((await removing).ok,true);assert.equal(await exists(e.id),false);
 await writeFile(join(projectF,'catalog-fail'),'fail');const failed=runtime.agentModels.list(f.id,'pi-rpc');void failed.catch(()=>{});await waitFile(join(projectF,'catalog-started'));
 const failRemoval=removal(f.id);void failRemoval.catch(()=>{});await writeFile(join(projectF,'catalog-release'),'release');await assert.rejects(failed);await assert.rejects(failRemoval,error('WORKSPACE_REMOVAL_FAILED',false));assert.equal(await exists(f.id),true);
 const dchat=await runtime.chats.create(d.id,'pi');const index=join(data,'chats','index.json'),backup=index+'.owned-backup';await rename(index,backup);await mkdir(index);
 try { await assert.rejects(removal(d.id),error('WORKSPACE_REMOVAL_PARTIAL',true));assert.equal(await exists(d.id),false);assert.ok(runtime.chats.list(d.id).some(x=>x.id===dchat.id)); }
 finally { await rm(index,{recursive:true});await rename(backup,index); }
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,noInference:true,originalPi:true,model:'openai/gpt-6.1-terra',workspaceA:a.id,workspaceB:b.id,ownedPid:pid,realActiveWorkflowBlocked:true,stoppedCleanupRejections,historicalRowsInsertedByFixtureAllowed:true,workflowStartBlockedWhileLeaseHeld:true,workflowStartBlockedDuringActualRemoval:true,nonexistentRejected:true,deletedSessions:[pi.id,unbound.id,external.id],ownedProcessExited:true,unrelatedWorkspaceUsable:true,projectAndNativeTranscriptPreserved:true,pendingSpoolRetained:true,sqliteTriggerRollback:true,indexFaultPartial:true,catalogLeaseAndDrain:true,catalogInitializationFailurePreservesWorkspace:true,scope:'Fresh bound Pi run; startup context only, no model turn'},null,2));console.log('Evidence:',directory);
}catch(e){await writeFile(join(directory,'failure.json'),JSON.stringify({error:String(e),stack:(e as Error).stack,events},null,2));console.error('Evidence:',directory);throw e;}finally{await Promise.all([projectE,projectF].map(path=>writeFile(join(path,'catalog-release'),'cleanup')));db.close();await runtime.close();}
// Separate desktop process: assert the actual JSONL result envelope, not the direct API exception.
{
 const {spawn}=await import('node:child_process');const {createInterface}=await import('node:readline');
 const child=spawn(process.execPath,['--import','tsx','src/main.ts'],{cwd:process.cwd(),env:{...process.env,HIVE_DESKTOP_DATA_DIR:join(directory,'wire-state')},stdio:['pipe','pipe','pipe']});
 let stderr='';child.stderr.on('data',b=>{stderr+=String(b)});const pending=new Map<number,{resolve:(v:any)=>void;reject:(e:Error)=>void}>();let id=0;
 createInterface({input:child.stdout}).on('line',line=>{const m=JSON.parse(line);if(typeof m.id==='number'){const p=pending.get(m.id);if(p){pending.delete(m.id);p.resolve(m);}}});
 child.on('exit',()=>{for(const p of pending.values())p.reject(new Error('Desktop exited: '+stderr));pending.clear();});
 const rpc=(method:string,params:unknown={})=>new Promise<any>((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});child.stdin.write(JSON.stringify({id:n,method,params})+'\n');});
 try{
  const init=await rpc('initialize');assert.ok(init.result.hivePort);
  const missing=await rpc('workspace.remove',{workspaceId:'absent-'+randomUUID()});assert.equal(missing.error,undefined);assert.equal(missing.result.ok,false);assert.equal(missing.result.error.code,'WORKSPACE_REMOVAL_FAILED');assert.equal(missing.result.error.details.workspaceRemoved,false);assert.equal(missing.result.error.details.stage,'prepare');
  const stop=await rpc('shutdown');assert.equal(stop.result.ok,true);
  await writeFile(join(directory,'wire-receipt.json'),JSON.stringify({pass:true,actualJsonl:true,missingWorkspaceStructuredError:true,noInference:true},null,2));
 }catch(e){await writeFile(join(directory,'wire-failure.json'),JSON.stringify({error:String(e),stderr},null,2));throw e;}finally{child.stdin.end();child.kill();}
}
