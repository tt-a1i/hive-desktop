/** Real PTY lifecycle + native filesystem failures; no mocked process success. */
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,rename,readFile,rm} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {ScriptEvent} from '../src/project-script-runs.js';
if(process.platform==='win32')throw new Error('POSIX native fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-script-storage-')),data=join(directory,'state'),project=join(directory,'project');await mkdir(project);
const runs=join(data,'project-script-runs'),backup=runs+'-unavailable';
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
let hook:((event:ScriptEvent)=>void)|undefined, moved=false;
const wait=async<T>(read:()=>T|Promise<T>,ready:(v:T)=>boolean)=>{const end=Date.now()+10000;for(;;){const v=await read();if(ready(v))return v;if(Date.now()>end)throw new Error('Timed out');await new Promise(done=>setTimeout(done,15));}};
try{
 runtime=await createDesktopRuntime(data,()=>{}, {onScriptEvent:event=>hook?.(event)});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Persistence failures',autostart_orchestrator:false}) as {id:string};
 const saved=await runtime.projectScripts.save(ws.id,'0',{name:'No spawn on failed claim',command:'printf should-not-run > forbidden'}),script=saved.scripts[0]!;
 // Moving the private directory after the claim but before native onCreated persistence prevents spawn.
 hook=event=>{if(!moved&&event.type==='status'&&event.run.status==='starting'&&!event.run.nativeRunId){moved=true;requireRename();}};
 function requireRename(){ // sync is intentional: inject precisely within the production lifecycle callback.
  const fs=process.getBuiltinModule('node:fs');fs.renameSync(runs,backup);
 }
 const request={workspaceId:ws.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID()};
 await assert.rejects(runtime.scriptRuns.run(request),/SCRIPT_RUN_WRITE_FAILED/);assert.equal(existsSync(join(project,'forbidden')),false);
 hook=undefined;await rename(backup,runs);moved=false;await runtime.close();runtime=undefined;
 runtime=await createDesktopRuntime(data,()=>{}, {onScriptEvent:event=>hook?.(event)});
 const recovered=await runtime.scriptRuns.run(request);assert.equal(recovered.status,'interrupted');assert.equal(existsSync(join(project,'forbidden')),false);
 const saved2=await runtime.projectScripts.save(ws.id,saved.revision,{name:'Actual running write failure',command:'printf "PID=%s\\n" "$$"; sleep 1.2; printf next; sleep 3; printf orphan > forbidden'}),script2=saved2.scripts.at(-1)!;
 let nativeId:string|undefined, nativeFinished=false;
 hook=event=>{
  if(event.type==='status'&&event.run.nativeRunId===nativeId&&event.run.status==='unknown'&&event.run.finishedAt)nativeFinished=true;
  if(!moved&&event.type==='status'&&event.run.status==='running'){moved=true;nativeId=event.run.nativeRunId;requireRename();}
 };
 const run=await runtime.scriptRuns.run({workspaceId:ws.id,scriptId:script2.id,expectedVersion:script2.version,requestId:randomUUID()});
 const pidtext=await wait(()=>readFile(join(backup,run.runId+'.log'),'utf8').catch(()=>''),v=>/PID=\d+/.test(v));const pid=Number(/PID=(\d+)/.exec(pidtext)?.[1]);assert.ok(nativeId);
 await wait(()=>{try{process.kill(pid,0);return false;}catch(error){if((error as NodeJS.ErrnoException).code==='ESRCH')return true;throw error;}},Boolean);
 await wait(()=>nativeFinished,Boolean); // PID reaping may precede delivery of node-pty's native exit callback.
 hook=undefined;await rename(backup,runs);moved=false;
 const observed=runtime.scriptRuns.get(ws.id,run.runId);assert.equal(observed.run.status,'unknown');assert.ok(observed.run.errorCode);assert.ok(Buffer.from(observed.output.bytes).toString().includes('PID='));
 await new Promise(done=>setTimeout(done,3200));assert.equal(existsSync(join(project,'forbidden')),false);
 await runtime.close();runtime=undefined;
 runtime=await createDesktopRuntime(data,()=>{});
 const cold=runtime.scriptRuns.get(ws.id,run.runId);assert.ok(['interrupted','unknown'].includes(cold.run.status));assert.ok(['SCRIPT_RUNTIME_INTERRUPTED','SCRIPT_LOG_WRITE_FAILED','SCRIPT_RUN_WRITE_FAILED'].includes(cold.run.errorCode ?? ''));assert.equal(cold.run.logBytes,Buffer.from(await readFile(join(runs,run.runId+'.log'))).length);
 console.log('PROJECT_SCRIPT_STORAGE_FAILURES_OK: claim persistence prevents native spawn; real running persistence failure terminates owned shell/tree and remains unknown; cold logs retained/interrupted, no retry.');
}finally{hook=undefined;if(moved)await rename(backup,runs);await runtime?.close();await rm(directory,{recursive:true,force:true});}
