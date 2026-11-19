/** SIGKILL of an isolated runtime, with native ownership checks when using the macOS desktop helper. */
import assert from 'node:assert/strict';
import {execFileSync,fork,type ChildProcess} from 'node:child_process';
import {mkdtemp,mkdir,readFile,realpath,rm,unlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-setup-crash-'))),project=join(dir,'project'),data=join(dir,'state');await mkdir(project);
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:project,stdio:'pipe'});
git('init','--template=');await writeFile(join(project,'base'),'base');git('add','.');git('commit','-m','fixture');
const guarded = process.platform === 'darwin' && !!process.env.HIVE_DESKTOP_SHELL_OWNER;
const missingLog = process.argv.includes('--missing-log');
function processState(pid: number) {
 const result = execFileSync('ps', ['-o','ppid=,pgid=,stat=', '-p', String(pid)], {encoding: 'utf8', stdio: ['ignore','pipe','ignore']}).trim();
 const [parent, group, state] = result.split(/\s+/);
 return {parent: Number(parent), group: Number(group), state};
}
function running(pid: number) {
 try {return !processState(pid).state?.startsWith('Z');}
 catch (error) {if ((error as {status?: number}).status !== 1) throw error; return false;}
}
let child:ChildProcess|undefined,nativePid:number|undefined,group:number|undefined,rt:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 child=fork(resolve('checks/fixtures/worktree-setup-crash.ts'),[data,project],{execArgv:['--import','tsx'],stdio:['ignore','pipe','pipe','ipc']});let errors='';child.stderr?.on('data',v=>errors+=v.toString());
 const info=await new Promise<{workspaceId:string;sessionId:string;path:string;runId:string}>((done,reject)=>{const timer=setTimeout(()=>reject(new Error('crash peer timeout '+errors)),15000);child!.once('message',v=>{clearTimeout(timer);done(v as any);});child!.once('exit',code=>reject(new Error('crash peer exit '+code+' '+errors)));});
 let descendant: number;
 const until = Date.now() + 10000;
 for (;;) {
  const output = await readFile(join(data,'project-script-runs',info.runId+'.log'),'utf8').catch(()=>'');
  const match = /NATIVE_PID=(\d+) CHILD_PID=(\d+)/.exec(output);
  if (match) {nativePid = Number(match[1]); descendant = Number(match[2]); break;}
  if (Date.now() > until) throw new Error('native pid timeout');
  await new Promise(done => setTimeout(done,20));
 }
 group = nativePid;
 if (guarded) {
  const ownedGroup = processState(nativePid).group;
  assert.equal(processState(ownedGroup).parent, child.pid, 'PTY group leader belongs to the running Node fixture');
  assert.notEqual(ownedGroup, nativePid, 'the owner helper leads the PTY group');
  group = ownedGroup;
  assert.equal(processState(descendant).group, group, 'child inherits the script process group');
 }
 const exit=new Promise<void>(done=>child!.once('exit',()=>done()));child.kill('SIGKILL');await exit;child=undefined;
 if (guarded) {
  const deadline = Date.now() + 5000;
  while ([group, nativePid, descendant].some(running)) {
   assert.ok(Date.now() < deadline, 'PTY helper, shell and HUP-ignoring child must exit after Node SIGKILL');
   await new Promise(done => setTimeout(done,20));
  }
  group = undefined;
  console.log('WORKTREE_INITIALIZATION_NATIVE_OWNER_OK: PTY helper, shell and child exited without fixture cleanup');
 }
 if (missingLog) await unlink(join(data,'project-script-runs',info.runId+'.log'));
 rt=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});const state=await rt.initialization.get(info.workspaceId,info.sessionId);assert.ok(state);assert.equal(state.status,'interrupted');assert.equal(state.actions.retry,false);assert.equal(state.actions.skip,false);assert.match(state.actions.reason!,/unconfirmed/);
 if (missingLog) {
  assert.equal(state.errorCode, 'SCRIPT_LOG_MISSING');
  assert.equal(rt.scriptRuns.list(info.workspaceId).runs[0]?.logBytes, 0);
  assert.throws(() => rt!.scriptRuns.get(info.workspaceId, info.runId), /SCRIPT_RUN_LOG_MISSING/);
  const projects = await rt.hive.request('GET','/api/workspaces') as {id:string}[];
  assert.ok(projects.some(project => project.id === info.workspaceId));
  console.log('WORKTREE_INITIALIZATION_MISSING_LOG_OK: runtime opens with interrupted setup and an explicit missing-log error');
 }
 const count=await readFile(join(info.path,'count'),'utf8');assert.equal(count,'once');
 for(let i=0;i<3;i++){const opened=await rt.chats.open(info.sessionId);assert.ok('readOnly' in opened&&opened.readOnly);assert.equal((await rt.initialization.get(info.workspaceId,info.sessionId))?.version,state.version);}
 await assert.rejects(rt.initialization.retry(info.workspaceId,info.sessionId,state.version),/uncertain/);await assert.rejects(rt.initialization.skip(info.workspaceId,info.sessionId,state.version),/uncertain/);
 assert.equal(rt.scriptRuns.list(info.workspaceId).runs.length,1);assert.equal(await readFile(join(info.path,'count'),'utf8'),'once');
 console.log('WORKTREE_INITIALIZATION_CRASH_OK: actual runtime SIGKILL, retained request, interrupted status, no replay/open/model, unavailable unsafe retry/skip with explicit reason.');
} finally {
 if(child)child.kill('SIGKILL');
 if(group){try{process.kill(-group,'SIGKILL');}catch{}}
 await rt?.close();await rm(dir,{recursive:true,force:true});
}
