/** SIGKILL of isolated runtime; never infer ownership of the old native process after restart. */
import assert from 'node:assert/strict';
import {execFileSync,fork,type ChildProcess} from 'node:child_process';
import {mkdtemp,mkdir,readFile,realpath,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-setup-crash-'))),project=join(dir,'project'),data=join(dir,'state');await mkdir(project);
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:project,stdio:'pipe'});
git('init','--template=');await writeFile(join(project,'base'),'base');git('add','.');git('commit','-m','fixture');
let child:ChildProcess|undefined,nativePid:number|undefined,rt:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 child=fork(resolve('checks/fixtures/worktree-setup-crash.ts'),[data,project],{execArgv:['--import','tsx'],stdio:['ignore','pipe','pipe','ipc']});let errors='';child.stderr?.on('data',v=>errors+=v.toString());
 const info=await new Promise<{workspaceId:string;sessionId:string;path:string;runId:string}>((done,reject)=>{const timer=setTimeout(()=>reject(new Error('crash peer timeout '+errors)),15000);child!.once('message',v=>{clearTimeout(timer);done(v as any);});child!.once('exit',code=>reject(new Error('crash peer exit '+code+' '+errors)));});
 const until=Date.now()+10000;for(;;){const output=await readFile(join(data,'project-script-runs',info.runId+'.log'),'utf8').catch(()=>'');const match=/NATIVE_PID=(\d+)/.exec(output);if(match){nativePid=Number(match[1]);break;}if(Date.now()>until)throw new Error('native pid timeout');await new Promise(done=>setTimeout(done,20));}
 const exit=new Promise<void>(done=>child!.once('exit',()=>done()));child.kill('SIGKILL');await exit;child=undefined;
 rt=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});const state=await rt.initialization.get(info.workspaceId,info.sessionId);assert.ok(state);assert.equal(state.status,'interrupted');assert.equal(state.actions.retry,false);assert.equal(state.actions.skip,false);assert.match(state.actions.reason!,/unconfirmed/);
 const count=await readFile(join(info.path,'count'),'utf8');assert.equal(count,'once');
 for(let i=0;i<3;i++){const opened=await rt.chats.open(info.sessionId);assert.ok('readOnly' in opened&&opened.readOnly);assert.equal((await rt.initialization.get(info.workspaceId,info.sessionId))?.version,state.version);}
 await assert.rejects(rt.initialization.retry(info.workspaceId,info.sessionId,state.version),/uncertain/);await assert.rejects(rt.initialization.skip(info.workspaceId,info.sessionId,state.version),/uncertain/);
 assert.equal(rt.scriptRuns.list(info.workspaceId).runs.length,1);assert.equal(await readFile(join(info.path,'count'),'utf8'),'once');
 console.log('WORKTREE_INITIALIZATION_CRASH_OK: actual runtime SIGKILL, retained native output/request, interrupted status, no replay/open/model, unavailable unsafe retry/skip with explicit reason. Old process cleanup uses test-owned PID only; production never kills a guessed PID.');
} finally {
 if(child)child.kill('SIGKILL');if(nativePid){try{process.kill(-nativePid,'SIGKILL');}catch{try{process.kill(nativePid,'SIGKILL');}catch{}}}
 await rt?.close();await rm(dir,{recursive:true,force:true});
}
