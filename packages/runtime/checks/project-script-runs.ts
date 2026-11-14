/** Real Git/Hive/SQLite/PTY scripts. No agent/model/account invocations. POSIX native validation only. */
import assert from 'node:assert/strict';
import {execFileSync, fork} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {mkdtemp, mkdir, readFile, writeFile, rename, rm, realpath} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {Database} from '@tt-a1i/hive/dist/src/server/sqlite.js';
import {createSettingsStore} from '@tt-a1i/hive/dist/src/server/settings-store.js';
import {resolveSavedScriptLaunch} from '@tt-a1i/hive/dist/src/server/workspace-shell-runtime.js';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {ProjectScriptRun} from '../src/project-script-runs.js';
import {ProjectScriptSettings} from '../src/project-script-settings.js';
if (process.platform === 'win32') throw new Error('POSIX fixture is not Windows native evidence');
const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-script-runs-')));
const project = join(directory,'project'), data = join(directory,'state'); await mkdir(project);
const git = (...args: string[]) => execFileSync('git', ['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args], {cwd: project, stdio:'pipe'});
git('init','--template='); await writeFile(join(project,'source'),'root'); git('add','.'); git('commit','-m','fixture');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const wait = async <T>(read:()=>T|Promise<T>, ready:(value:T)=>boolean, label:string):Promise<T> => {
 const until=Date.now()+15000; for (;;) {const value=await read();if(ready(value))return value;if(Date.now()>until)throw new Error('Timed out: '+label);await new Promise(done=>setTimeout(done,15));}
};
const terminal = (run:ProjectScriptRun) => !['starting','running'].includes(run.status);
let child: ReturnType<typeof fork> | undefined;
let nativePid: number | undefined;
try {
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});
 const rt=runtime;
 const workspace=await rt.hive.request('POST','/api/workspaces',{path:project,name:'Scripts',autostart_orchestrator:false}) as {id:string};
 const a=await rt.chats.createRecord(workspace.id,'codex',{isolation:'worktree'}), b=await rt.chats.createRecord(workspace.id,'codex',{isolation:'worktree'});
 const ap=await rt.chats.executionDirectory(workspace.id,a.id), bp=await rt.chats.executionDirectory(workspace.id,b.id);
 await writeFile(join(ap,'source'),'A');await writeFile(join(bp,'source'),'B');
 const save = async (command:string) => {const before=await rt.projectScripts.list(workspace.id);return (await rt.projectScripts.save(workspace.id,before.revision,{name:'Fixture',command})).scripts.at(-1)!;};
 const script=await save('printf "STDOUT:"; cat source; printf "STDERR" >&2; cat source > relative-output; exit 0');
 const runs:ProjectScriptRun[]=[];
 for(const [sessionId,path,content] of [[undefined,project,'root'],[a.id,ap,'A'],[b.id,bp,'B']] as const) {
  const request={workspaceId:workspace.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID(),...(sessionId?{sessionId}:{})};
  const [one,two]=await Promise.all([rt.scriptRuns.run(request),rt.scriptRuns.run(request)]);assert.equal(one.runId,two.runId);
  const done=await wait(()=>rt.scriptRuns.get(workspace.id,one.runId),v=>terminal(v.run),'native exit');
  assert.equal(done.run.status,'succeeded');assert.equal(done.run.exitCode,0);assert.equal(done.run.cwd,path);
  const output=Buffer.from(done.output.bytes).toString();assert.ok(output.includes('STDOUT:'+content));assert.ok(output.includes('STDERR'));
  assert.equal(await readFile(join(path,'relative-output'),'utf8'),content);runs.push(done.run);
  assert.equal((await rt.scriptRuns.run(request)).runId,done.run.runId);
  await assert.rejects(rt.scriptRuns.run({...request,sessionId:sessionId?a.id===sessionId?b.id:a.id:a.id}),/SCRIPT_RUN_REQUEST_CONFLICT/);
 }
 assert.equal((await rt.hive.request('GET','/api/workspaces') as unknown[]).length,1);
 assert.equal((await rt.hive.request('GET',`/api/ui/workspaces/${workspace.id}/runs`) as {agent_id:string}[]).some(run=>!run.agent_id.endsWith(':shell')),false);
 const nonzero=await save('printf failure-output; printf failure-stderr >&2; exit 7');
 const nr=await rt.scriptRuns.run({workspaceId:workspace.id,scriptId:nonzero.id,expectedVersion:nonzero.version,requestId:randomUUID()});
 const nf=await wait(()=>rt.scriptRuns.get(workspace.id,nr.runId),v=>terminal(v.run),'nonzero');assert.equal(nf.run.status,'failed');assert.equal(nf.run.exitCode,7);assert.ok(Buffer.from(nf.output.bytes).toString().includes('failure-stderr'));
 const cancel=await save('printf running; (sleep 2; printf orphan > must-not-exist) & wait');
 const cr=await rt.scriptRuns.run({workspaceId:workspace.id,scriptId:cancel.id,expectedVersion:cancel.version,sessionId:a.id,requestId:randomUUID()});
 await wait(()=>rt.scriptRuns.get(workspace.id,cr.runId),v=>v.output.bytes.length>0,'cancel output');
 assert.equal((await rt.terminal.list(workspace.id,a.id)).runs.length,0,'script cannot be mistaken for an interactive terminal');
 await rename(ap,ap+'-missing');
 const stopped=await rt.scriptRuns.stop(workspace.id,cr.runId);assert.equal(stopped.status,'cancelled');assert.ok(stopped.finishedAt);assert.notEqual(stopped.exitCode,null);
 await assert.rejects(rt.scriptRuns.run({workspaceId:workspace.id,scriptId:script.id,expectedVersion:script.version,sessionId:a.id,requestId:randomUUID()}));
 await new Promise(done=>setTimeout(done,2200));assert.equal(existsSync(join(ap+'-missing','must-not-exist')),false);await rename(ap+'-missing',ap);
 const before=await rt.projectScripts.list(workspace.id);
 const edited=await rt.projectScripts.save(workspace.id,before.revision,{id:script.id,name:'Changed',command:'printf changed'});
 await assert.rejects(rt.scriptRuns.run({workspaceId:workspace.id,scriptId:script.id,expectedVersion:script.version,requestId:randomUUID()}),/changed/);
 assert.equal(rt.scriptRuns.get(workspace.id,runs[0]!.runId).run.script.command,script.command,'captured command retained after settings edits');
 await assert.rejects(rt.scriptRuns.run({workspaceId:workspace.id,scriptId:script.id,expectedVersion:edited.scripts[0]!.version,requestId:randomUUID(),cwd:project}),/SCRIPT_RUN_INPUT_INVALID/);
 // Actual native spawn failure after directory resolution, without replacing process execution.
 const start=rt.hive.startSavedScript.bind(rt.hive);
 rt.hive.startSavedScript=async (ws,context,observer)=>{
  await rename(context.cwd,context.cwd+'-spawn-missing');
  let finished!:()=>void; const exited=new Promise<void>(done=>{finished=done;});
  try {const result=await start(ws,context,{...observer,onExit:code=>{observer.onExit(code);finished();}});await exited;return result;}
  finally{await rename(context.cwd+'-spawn-missing',context.cwd);}
 };
 const failed=await rt.scriptRuns.run({workspaceId:workspace.id,scriptId:nonzero.id,expectedVersion:nonzero.version,sessionId:a.id,requestId:randomUUID()});
 assert.equal(failed.status,'failed');assert.ok(failed.exitCode === null || failed.exitCode !== 0);rt.hive.startSavedScript=start;
 // Official second SQLite connection exercises the same production CAS implementation.
 const db=new Database(join(data,'hive','runtime.sqlite')), settings=createSettingsStore(db);
 const second=new ProjectScriptSettings({read:(key,ws)=>settings.readProjectScriptState(key,ws),compareAndSwap:(key,expected,next,ws)=>settings.compareAndSwapProjectScriptState(key,expected,next,ws)},()=>{});
 const other=join(directory,'other');await mkdir(other);const ws2=await rt.hive.request('POST','/api/workspaces',{path:other,name:'Other',autostart_orchestrator:false}) as {id:string};
 const winners=await Promise.allSettled([rt.projectScripts.save(ws2.id,'0',{name:'One',command:'true'}),second.save(ws2.id,'0',{name:'Two',command:'true'})]);assert.equal(winners.filter(v=>v.status==='fulfilled').length,1);
 const ws2saved=await rt.projectScripts.list(ws2.id);
 const key=`desktop_project_scripts_v1_${createHash('sha256').update(ws2.id).digest('hex')}`, raw=settings.readProjectScriptState(key,ws2.id);
 await rt.hive.request('DELETE',`/api/workspaces/${ws2.id}`);
 assert.equal(db.prepare('SELECT value FROM app_state WHERE key=?').get(key),undefined);
 assert.throws(()=>settings.compareAndSwapProjectScriptState(key,null,raw,ws2.id),/SCRIPT_WORKSPACE_MISSING/);
 assert.equal((await rt.projectScripts.list(workspace.id)).revision,edited.revision);
 db.close();assert.ok(ws2saved.scripts.length===1);
 const closing=await rt.scriptRuns.run({workspaceId:workspace.id,scriptId:cancel.id,expectedVersion:cancel.version,sessionId:b.id,requestId:randomUUID()});
 await wait(()=>rt.scriptRuns.get(workspace.id,closing.runId),v=>v.output.bytes.length>0,'closing output');
 await rt.close();runtime=undefined;
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});
 assert.equal(runtime.scriptRuns.get(workspace.id,closing.runId).run.status,'cancelled');
 assert.equal(runtime.scriptRuns.get(workspace.id,runs[1]!.runId).run.status,'succeeded');
 assert.equal((await runtime.scriptRuns.run({workspaceId:workspace.id,scriptId:runs[1]!.script.id,expectedVersion:runs[1]!.script.version,sessionId:a.id,requestId:runs[1]!.requestId})).runId,runs[1]!.runId,'cold duplicate survives edited script version');
 await runtime.close();runtime=undefined;
 // Kill a separate backend after the actual native command writes output; reopening marks interrupted.
 const crashData=join(directory,'crash-state'), crashProject=join(directory,'crash-project');await mkdir(crashProject);
 child=fork(resolve('checks/fixtures/project-script-crash.ts'),[crashData,crashProject],{execArgv:['--import','tsx'],stdio:['ignore','pipe','pipe','ipc']});
 let peerErrors='';child.stderr?.on('data',bytes=>peerErrors+=bytes.toString());
 const info=await new Promise<{workspaceId:string;run:ProjectScriptRun}>((done,reject)=>{const timer=setTimeout(()=>reject(new Error('crash peer timeout '+peerErrors)),15000);child!.once('message',value=>{clearTimeout(timer);done(value as {workspaceId:string;run:ProjectScriptRun});});child!.once('exit',code=>reject(new Error('crash peer early exit '+code+' '+peerErrors)));});
 await wait(()=>readFile(join(crashProject,'count'),'utf8').catch(()=>''),v=>v==='once','crash command ran');
 const log=await wait(()=>readFile(join(crashData,'project-script-runs',info.run.runId+'.log'),'utf8').catch(()=>''),v=>v.includes('NATIVE_PID='),'native pid');nativePid=Number(/NATIVE_PID=(\d+)/.exec(log)?.[1]);
 const exited=new Promise<void>(done=>child!.once('exit',()=>done()));child.kill('SIGKILL');await exited;child=undefined;
 runtime=await createDesktopRuntime(crashData,()=>{}, {executableEnvironment:{}});
 const interrupted=runtime.scriptRuns.get(info.workspaceId,info.run.runId);assert.equal(interrupted.run.status,'interrupted');assert.equal(interrupted.run.exitCode,null);assert.ok(Buffer.from(interrupted.output.bytes).toString().includes('NATIVE_PID='));
 assert.equal((await runtime.scriptRuns.run({workspaceId:info.workspaceId,scriptId:info.run.script.id,expectedVersion:info.run.script.version,requestId:info.run.requestId})).runId,info.run.runId);
 assert.equal(await readFile(join(crashProject,'count'),'utf8'),'once');
 assert.equal((await runtime.scriptRuns.stop(info.workspaceId,info.run.runId)).status,'interrupted');
 assert.deepEqual(resolveSavedScriptLaunch('echo x','C:\\repo', {ComSpec:'C:\\Windows\\System32\\cmd.exe'},'win32'),{command:'C:\\Windows\\System32\\cmd.exe',args:['/d','/s','/c','echo x'],cwd:'C:\\repo'});
 assert.throws(()=>resolveSavedScriptLaunch('echo x','\\\\server\\repo',{},'win32'),/SCRIPT_UNSUPPORTED_CWD/);
 console.log('PROJECT_SCRIPT_RUNS_OK: actual root/A/B PTYs, output+native exit, nonzero, cancel/process tree after source loss, saved version/duplicate/captured provenance, real spawn failure, production SQLite CAS/delete, close/cold/crash interruption. POSIX native only; zero agents/models.');
} finally {
 if(child)child.kill('SIGKILL');
 if(nativePid) {try{process.kill(-nativePid,'SIGKILL');}catch{try{process.kill(nativePid,'SIGKILL');}catch{}}}
 await runtime?.close();await rm(directory,{recursive:true,force:true});
}
