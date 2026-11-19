/** Actual runtime JSONL + Hive + native saved script, no models. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createInterface} from 'node:readline';
if(process.platform==='win32')throw new Error('POSIX native JSONL fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-scripts-wire-')),project=join(directory,'project');await mkdir(project);
const child=spawn(process.execPath,[process.env.HIVE_DESKTOP_TEST_RUNTIME ?? resolve('dist/main.js')],{env:{...process.env,HIVE_DESKTOP_DATA_DIR:join(directory,'state')},stdio:['pipe','pipe','pipe']});
let sequence=0,stderr='';const pending=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void}>(),events:any[]=[];
child.stderr.on('data',bytes=>stderr+=bytes.toString());
createInterface({input:child.stdout}).on('line',line=>{const frame=JSON.parse(line);if(typeof frame.id==='number'){pending.get(frame.id)?.resolve(frame);pending.delete(frame.id);}else events.push(frame);});
child.on('exit',()=>{for(const request of pending.values())request.reject(new Error('Runtime exited: '+stderr));pending.clear();});
const rpc=(method:string,params:unknown={})=>new Promise<any>((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});child.stdin.write(JSON.stringify({id,method,params})+'\n');});
const until=async<T>(read:()=>T|Promise<T>,ready:(value:T)=>boolean)=>{const end=Date.now()+15000;for(;;){const value=await read();if(ready(value))return value;if(Date.now()>end)throw new Error('Wire timeout');await new Promise(done=>setTimeout(done,20));}};
try{
 assert.ok((await rpc('initialize')).result.hivePort);
 const ws=(await rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:project,name:'Script wire',autostart_orchestrator:false}})).result;
 assert.deepEqual((await rpc('script.settings.list',{workspaceId:ws.id})).result,{version:2,revision:'0',scripts:[]});
 const saved=(await rpc('script.settings.save',{workspaceId:ws.id,expectedRevision:'0',script:{name:'Native wire',command:'printf native-wire; exit 9'}})).result;
 const script=saved.scripts[0];
 const missing=await rpc('script.run',{workspaceId:ws.id,scriptId:randomUUID(),expectedVersion:randomUUID(),requestId:randomUUID()});
 assert.equal(missing.error.code,'SCRIPT_SETTINGS_NOT_FOUND');assert.ok(missing.error.message.startsWith('[SCRIPT_SETTINGS_NOT_FOUND] '));
 const staleRequestId=randomUUID();
 const stale=await rpc('script.run',{workspaceId:ws.id,scriptId:script.id,expectedVersion:randomUUID(),requestId:staleRequestId});
 assert.equal(stale.error.code,'SCRIPT_SETTINGS_CONFLICT');assert.ok(stale.error.message.startsWith('[SCRIPT_SETTINGS_CONFLICT] '));
 const invalid=await rpc('script.settings.save',{workspaceId:ws.id,expectedRevision:saved.revision,script:{name:'',command:'true'}});
 assert.equal(invalid.error.code,'SCRIPT_SETTINGS_INVALID');assert.ok(invalid.error.message.startsWith('[SCRIPT_SETTINGS_INVALID] '));
 const emptyHistory=(await rpc('script.runs',{workspaceId:ws.id})).result;
 assert.equal(emptyHistory.runs.length,0,'preflight errors did not claim a run');
 const request={workspaceId:ws.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID()};
 const started=await rpc('script.run',request);assert.ok(started.result.runId);const runId=started.result.runId;
 const done=await until(()=>rpc('script.get',{workspaceId:ws.id,runId,offset:0,limit:128}),value=>value.result&&!['starting','running'].includes(value.result.run.status));
 assert.equal(done.result.run.status,'failed');assert.equal(done.result.run.exitCode,9);assert.equal(Buffer.from(done.result.output.bytes).toString(),'native-wire');
 assert.ok(events.some(frame=>frame.event==='script'&&frame.data.type==='status'&&frame.data.run.runId===runId));
 assert.equal((await rpc('script.run',request)).result.runId,runId);assert.equal((await rpc('script.runs',{workspaceId:ws.id})).result.runs.length,1);
 const badInput=await rpc('script.run',{...request,command:'echo forbidden'});assert.equal(badInput.error.code,'SCRIPT_RUN_INPUT_INVALID');assert.ok(badInput.error.message.startsWith('[SCRIPT_RUN_INPUT_INVALID] '));
 assert.equal((await rpc('script.stop',{workspaceId:ws.id,runId})).result.status,'failed');

 const otherProject=join(directory,'other-project');await mkdir(otherProject);
 const other=(await rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:otherProject,name:'Corrupt script history',autostart_orchestrator:false}})).result;
 const otherScript=(await rpc('script.settings.save',{workspaceId:other.id,expectedRevision:'0',script:{name:'Run once',command:'printf once >> executions'}})).result.scripts[0];
 const otherRequest={workspaceId:other.id,scriptId:otherScript.id,expectedVersion:otherScript.version,requestId:randomUUID()};
 const otherRun=(await rpc('script.run',otherRequest)).result;
 await until(()=>rpc('script.get',{workspaceId:other.id,runId:otherRun.runId}),value=>value.result?.run.status==='succeeded');
 const receiptName=createHash('sha256').update(JSON.stringify([other.id,otherRequest.requestId])).digest('hex')+'.json';
 const receipt=join(directory,'state','project-script-runs',receiptName);
 const corrupt='{ incomplete';await writeFile(receipt,corrupt);
 const history=await rpc('script.runs',{workspaceId:ws.id});
 const healthyLog=await rpc('script.get',{workspaceId:ws.id,runId,offset:0,limit:128});
 const historicalStop=await rpc('script.stop',{workspaceId:ws.id,runId});
 assert.ok(history.result&&healthyLog.result&&historicalStop.result,
  JSON.stringify({history:history.error,log:healthyLog.error,stop:historicalStop.error}));
 assert.deepEqual(history.result,{runs:[done.result.run],unreadableRunCount:1});
 assert.equal(emptyHistory.unreadableRunCount,0);
 assert.deepEqual((await rpc('script.runs',{workspaceId:other.id})).result,{runs:[],unreadableRunCount:1});
 const checkRequest=(workspaceId:string,requestId:string)=>rpc('script.runs',{workspaceId,requestId});
 const absent=await checkRequest(ws.id,staleRequestId);
 assert.deepEqual(absent.result,{...history.result,requestState:'absent'});
 assert.deepEqual((await checkRequest(ws.id,request.requestId)).result,{...history.result,requestState:'present'});
 assert.deepEqual((await checkRequest(ws.id,otherRequest.requestId)).result,{...history.result,requestState:'absent'});
 assert.deepEqual((await checkRequest(other.id,otherRequest.requestId)).result,{runs:[],unreadableRunCount:1,requestState:'unreadable'});
 assert.equal((await checkRequest(ws.id,'bad-id')).error.code,'SCRIPT_RUN_INPUT_INVALID');
 assert.deepEqual(healthyLog.result,done.result);
 assert.equal(historicalStop.result.status,'failed');
 assert.equal((await rpc('script.get',{workspaceId:other.id,runId})).error.code,'SCRIPT_RUN_NOT_FOUND');
 assert.equal((await rpc('script.stop',{workspaceId:other.id,runId})).error.code,'SCRIPT_RUN_NOT_FOUND');
 assert.equal((await rpc('script.get',{workspaceId:other.id,runId:otherRun.runId})).error.code,'SCRIPT_RUN_NOT_FOUND');
 assert.equal((await rpc('script.run',otherRequest)).error.code,'SCRIPT_RUN_CORRUPT');
 assert.equal(await readFile(receipt,'utf8'),corrupt);
 assert.equal(await readFile(join(otherProject,'executions'),'utf8'),'once');

 assert.equal((await rpc('script.settings.remove',{workspaceId:ws.id,expectedRevision:saved.revision,scriptId:script.id})).result.scripts.length,0);
 await rename(project,project+'-missing');
 try {
  const original=(await rpc('script.run',request)).result;assert.equal(original.runId,runId);assert.equal(original.script.command,script.command);assert.equal(original.cwd,done.result.run.cwd);
  assert.equal((await rpc('script.runs',{workspaceId:ws.id})).result.runs[0].runId,runId);
 } finally {await rename(project+'-missing',project);}
 await rpc('shutdown');
 await new Promise<void>(done=>{if(child.exitCode!==null)done();else child.once('exit',()=>done());});
 assert.equal(child.exitCode,0);
 console.log('PROJECT_SCRIPTS_WIRE_OK: actual JSONL settings/run/log/events/exit/duplicate/validation/stop/remove/shutdown; unreadable receipt isolated, exact request blocked, no cross-project access or replay.');
}finally{if(child.exitCode===null)child.kill('SIGKILL');await rm(directory,{recursive:true,force:true});}
