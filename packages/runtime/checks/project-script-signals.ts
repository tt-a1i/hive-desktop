/** A real shell killed by a signal must never count as a successful exit. */
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
if(process.platform==='win32')throw new Error('POSIX signal fixture only');
const dir=await mkdtemp(join(tmpdir(),'hive-script-signals-')),project=join(dir,'project');await mkdir(project);
const runtime=await createDesktopRuntime(join(dir,'state'),()=>{});
try{
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Signals',autostart_orchestrator:false}) as {id:string};
 const saved=await runtime.projectScripts.save(ws.id,'0',{name:'Self termination',command:'printf before-signal; kill -TERM $$'}),script=saved.scripts[0]!;
 const run=await runtime.scriptRuns.run({workspaceId:ws.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID()});
 const end=Date.now()+10000;let done=runtime.scriptRuns.get(ws.id,run.runId);
 while(['starting','running'].includes(done.run.status)){if(Date.now()>end)throw Error('Native signal timeout');await new Promise(resolve=>setTimeout(resolve,20));done=runtime.scriptRuns.get(ws.id,run.runId);}
 assert.equal(done.run.status,'failed',JSON.stringify(done.run));assert.equal(done.run.signal,15);assert.equal(done.run.errorCode,'SCRIPT_SIGNALLED');
 console.log('PROJECT_SCRIPT_SIGNALS_OK: native self-SIGTERM retains signal and cannot become succeeded.');
}finally{await runtime.close();await rm(dir,{recursive:true,force:true});}
