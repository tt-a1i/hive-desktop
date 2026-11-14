/** Actual native shell output with byte-page boundaries and a bounded retained log. */
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,rm,stat} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
if(process.platform==='win32')throw new Error('POSIX shell fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-script-pages-')),project=join(directory,'project'),data=join(directory,'state');await mkdir(project);
const runtime=await createDesktopRuntime(data,()=>{});
const wait=async<T>(read:()=>T|Promise<T>,ready:(v:T)=>boolean)=>{const end=Date.now()+30000;for(;;){const v=await read();if(ready(v))return v;if(Date.now()>end)throw new Error('Timed out');await new Promise(done=>setTimeout(done,20));}};
try{
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Byte pages',autostart_orchestrator:false}) as {id:string};
 let snapshot=await runtime.projectScripts.save(ws.id,'0',{name:'UTF8',command:"printf '甲乙😀'"});const small=snapshot.scripts[0]!;
 const run=await runtime.scriptRuns.run({workspaceId:ws.id,scriptId:small.id,expectedVersion:small.version,requestId:randomUUID()});
 await wait(()=>runtime.scriptRuns.get(ws.id,run.runId).run,v=>!['starting','running'].includes(v.status));
 const bytes:number[]=[];let offset=0;for(;;){const page=runtime.scriptRuns.get(ws.id,run.runId,offset,2);bytes.push(...page.output.bytes);if(page.output.nextOffset===offset)break;offset=page.output.nextOffset;}
 assert.equal(Buffer.from(bytes).toString(),'甲乙😀');assert.equal(offset,Buffer.byteLength('甲乙😀'));
 assert.throws(()=>runtime.scriptRuns.get(ws.id,run.runId,-1),/SCRIPT_RUN_INPUT_INVALID/);assert.throws(()=>runtime.scriptRuns.get(ws.id,run.runId,0,65537),/SCRIPT_RUN_INPUT_INVALID/);
 snapshot=await runtime.projectScripts.save(ws.id,snapshot.revision,{name:'Bounded',command:"head -c 16777300 /dev/zero | tr '\\000' 'x'"});const large=snapshot.scripts.at(-1)!;
 const output=await runtime.scriptRuns.run({workspaceId:ws.id,scriptId:large.id,expectedVersion:large.version,requestId:randomUUID()});
 const done=await wait(()=>runtime.scriptRuns.get(ws.id,output.runId).run,v=>!['starting','running'].includes(v.status));
 assert.equal(done.status,'succeeded',JSON.stringify(done));assert.equal(done.exitCode,0);assert.equal(done.logBytes,16*1024*1024);assert.equal(done.logTruncated,true);
 assert.equal((await stat(join(data,'project-script-runs',output.runId+'.log'))).size,16*1024*1024);
 const last=runtime.scriptRuns.get(ws.id,output.runId,16*1024*1024-2);assert.deepEqual(last.output.bytes,[120,120]);assert.equal(last.output.truncated,true);
 console.log('PROJECT_SCRIPT_LOG_PAGES_OK: actual UTF8 output across 2-byte pages; real >16MiB shell output succeeds by native exit, retained bytes bounded with explicit truncation.');
}finally{await runtime.close();await rm(directory,{recursive:true,force:true});}
