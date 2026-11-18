/** Actual runtime JSONL + Hive + native saved script, no models. */
import assert from 'node:assert/strict';
import {execFileSync,spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createInterface} from 'node:readline';
if(process.platform==='win32')throw new Error('POSIX native JSONL fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-scripts-wire-')),project=join(directory,'project');await mkdir(project);
execFileSync('git',['init','--template='],{cwd:project,stdio:'pipe'});
await writeFile(join(project,'source'),'base');
execFileSync('git',['add','.'],{cwd:project});
execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false','commit','-m','fixture'],{cwd:project,stdio:'pipe'});
const child=spawn(process.execPath,[process.env.HIVE_DESKTOP_TEST_RUNTIME ?? resolve('dist/main.js')],{env:{...process.env,HIVE_DESKTOP_DATA_DIR:join(directory,'state')},stdio:['pipe','pipe','pipe']});
let sequence=0,stderr='';const pending=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void}>(),events:any[]=[];
child.stderr.on('data',bytes=>stderr+=bytes.toString());
createInterface({input:child.stdout}).on('line',line=>{const frame=JSON.parse(line);if(typeof frame.id==='number'){pending.get(frame.id)?.resolve(frame);pending.delete(frame.id);}else events.push(frame);});
child.on('exit',()=>{for(const request of pending.values())request.reject(new Error('Runtime exited: '+stderr));pending.clear();});
const rpc=(method:string,params:unknown={})=>new Promise<any>((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});child.stdin.write(JSON.stringify({id,method,params})+'\n');});
const until=async<T>(read:()=>T|Promise<T>,ready:(value:T)=>boolean)=>{const end=Date.now()+15000;for(;;){const value=await read();if(ready(value))return value;if(Date.now()>end)throw new Error('Wire timeout');await new Promise(done=>setTimeout(done,20));}};
const changes=()=>events.filter(frame=>frame.event==='worktreeInitializationChanged');
try{
 assert.ok((await rpc('initialize')).result.hivePort);
 const ws=(await rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:project,name:'Script wire',autostart_orchestrator:false}})).result;
 const saved=(await rpc('script.settings.save',{workspaceId:ws.id,expectedRevision:'0',script:{name:'Setup wire',command:'printf PRIVATE_ORIGINAL_SETUP; exit 8',runOnWorktreeCreate:true}})).result;
 const script=saved.scripts[0];
 const session=(await rpc('chat.createRecord',{workspaceId:ws.id,backend:'codex',isolation:'worktree'})).result;
 const params={workspaceId:ws.id,sessionId:session.id};
 const failed=await until(()=>rpc('worktree.initialization.get',params),value=>value.result?.status==='failed');
 const firstChanges=await until(changes,value=>value.length>=2);
 for(const frame of firstChanges) assert.deepEqual(frame.data,params);
 assert.equal(failed.result.script.name,'Setup wire');assert.ok(failed.result.runId);assert.equal('command' in failed.result.script,false);
 const opened=(await rpc('chat.open',{sessionId:session.id})).result;
 assert.equal(opened.readOnly,true);assert.equal(opened.identityError.code,'WORKTREE_INITIALIZATION_BLOCKED');assert.equal(opened.initialization.status,'failed');
 assert.ok(!JSON.stringify(opened.initialization).includes('PRIVATE_ORIGINAL_SETUP'));
 const stale=await rpc('worktree.initialization.retry',{...params,expectedVersion:randomUUID()});assert.equal(stale.error.code,'WORKTREE_INITIALIZATION_BLOCKED');
 for(const extra of [{command:'echo forbidden'},{cwd:project},{requestId:randomUUID()}]){
  const invalid=await rpc('worktree.initialization.retry',{...params,expectedVersion:failed.result.version,...extra});assert.equal(invalid.error.code,'WORKTREE_INITIALIZATION_BLOCKED');
 }
 await rpc('script.settings.remove',{workspaceId:ws.id,expectedRevision:saved.revision,scriptId:script.id});
 const retry=await rpc('worktree.initialization.retry',{...params,expectedVersion:failed.result.version});assert.ok(retry.result);
 const second=await until(()=>rpc('worktree.initialization.get',params),value=>value.result?.status==='failed' && value.result.runId!==failed.result.runId);
 const retryChanges=await until(changes,value=>value.length>=firstChanges.length+2);
 for(const frame of retryChanges) assert.deepEqual(frame.data,params);
 assert.notEqual(second.result.requestId,failed.result.requestId);assert.equal(second.result.script.version,script.version);
 const skipped=await rpc('worktree.initialization.skip',{...params,expectedVersion:second.result.version});assert.equal(skipped.result.status,'skipped');
 const runs=(await rpc('script.runs',{workspaceId:ws.id})).result.runs;assert.equal(runs.length,2);assert.equal(runs[0].script.command,script.command);
 const root=(await rpc('chat.createRecord',{workspaceId:ws.id,backend:'codex'})).result;
 assert.equal((await rpc('worktree.initialization.get',{workspaceId:ws.id,sessionId:root.id})).result,null);
 await rpc('shutdown');
 await new Promise<void>(done=>{if(child.exitCode!==null)done();else child.once('exit',()=>done());});
 assert.equal(child.exitCode,0);
 console.log('WORKTREE_INITIALIZATION_WIRE_OK: actual JSONL explicit role/captured redacted status/blocked open/stale state/no command or cwd RPC/retry deleted snapshot/skip/root/shutdown.');
}finally{if(child.exitCode===null)child.kill('SIGKILL');await rm(directory,{recursive:true,force:true});}
