/** Silent actual PTYs remain stoppable when all private run metadata disappears. */
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,readFile,rename,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
if(process.platform==='win32')throw new Error('POSIX native fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-script-stop-storage-')),project=join(directory,'project'),data=join(directory,'state');await mkdir(project);
const runtime=await createDesktopRuntime(data,()=>{});
let moved=false;
const gone=async(pid:number)=>{const until=Date.now()+10000;for(;;){try{process.kill(pid,0);}catch(error){if((error as NodeJS.ErrnoException).code==='ESRCH')return;throw error;}if(Date.now()>until)throw Error('Owned native PID survived');await new Promise(done=>setTimeout(done,20));}};
try{
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Silent stop',autostart_orchestrator:false}) as {id:string};
 const saved=await runtime.projectScripts.save(ws.id,'0',{name:'Silent owned process',command:'printf %s "$$" > owned.pid; sleep 30'}),script=saved.scripts[0]!;
 const start=()=>runtime.scriptRuns.run({workspaceId:ws.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID()});
 const run=await start();
 // Only query this exact owned process through the existing native terminal restore contract.
 const rows=await runtime.hive.request('GET',`/api/ui/workspaces/${ws.id}/runs`) as {run_id:string}[];assert.ok(rows.some(row=>row.run_id===run.nativeRunId));
 const ownedPid=async()=>{const until=Date.now()+10000;for(;;){const pid=Number(await readFile(join(project,'owned.pid'),'utf8').catch(()=>''));if(Number.isInteger(pid)&&pid>0){process.kill(pid,0);return pid;}if(Date.now()>until)throw Error('Owned PID was not written');await new Promise(done=>setTimeout(done,20));}};
 const pid=await ownedPid();
 await rename(join(data,'project-script-runs'),join(data,'run-records-unavailable'));moved=true;
 await assert.rejects(runtime.scriptRuns.stop('another-workspace',run.runId),/SCRIPT_RUN_NOT_FOUND/);
 await assert.rejects(runtime.scriptRuns.stop(ws.id,run.runId),/SCRIPT_RUN_WRITE_FAILED/);
 await gone(pid);
 assert.equal((await runtime.hive.request('GET',`/api/ui/workspaces/${ws.id}/runs`) as {run_id:string}[]).some(row=>row.run_id===run.nativeRunId),false);
 await rename(join(data,'run-records-unavailable'),join(data,'project-script-runs'));moved=false;
 assert.equal(runtime.scriptRuns.get(ws.id,run.runId).run.status,'unknown');
 // Workspace shutdown must also reap silent owned PTYs without reading their missing records.
 await rm(join(project,'owned.pid'));await start();const workspacePid=await ownedPid();
 await rename(join(data,'project-script-runs'),join(data,'run-records-unavailable'));moved=true;
 await assert.rejects(runtime.scriptRuns.stopWorkspace(ws.id),/shutdown/);await gone(workspacePid);
 await rename(join(data,'run-records-unavailable'),join(data,'project-script-runs'));moved=false;
 // close() uses the same trusted ownership path; write errors remain errors after exit confirmation.
 await rm(join(project,'owned.pid'));const closing=await start();const closePid=await ownedPid();
 await rename(join(data,'project-script-runs'),join(data,'run-records-unavailable'));moved=true;
 await assert.rejects(runtime.scriptRuns.close(),/shutdown/);await gone(closePid);
 assert.ok(closing.nativeRunId);
 console.log('PROJECT_SCRIPT_STOP_STORAGE_OK: silent real shell stopped by trusted ws/run ownership despite missing metadata; original write error retained, wrong workspace refused, stopWorkspace/close confirm actual PID exit.');
}finally{if(moved)await rename(join(data,'run-records-unavailable'),join(data,'project-script-runs'));await runtime.close();await rm(directory,{recursive:true,force:true});}
