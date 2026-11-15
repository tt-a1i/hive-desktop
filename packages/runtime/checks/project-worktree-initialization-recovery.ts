/** Real Git/Hive recovery before native setup exists. No account or model calls. */
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, readFile, realpath, rename, rm, writeFile} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {readWorktreeIntent} from '../src/git-worktrees.js';
import type {ChatIndex} from '../src/chat-index.js';
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-setup-recovery-'))), project=join(dir,'project'), data=join(dir,'state');
await mkdir(project);const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:project,stdio:'pipe'});
git('init','--template=');await writeFile(join(project,'base'),'base');git('add','.');git('commit','-m','fixture');
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});const rt=runtime;
 const ws=await rt.hive.request('POST','/api/workspaces',{path:project,name:'Recovery',autostart_orchestrator:false}) as {id:string};
 const worker=rt.createMember(ws.id,{name:'Worker',role:'coder',backend:'codex'});
 const saved=await rt.projectScripts.save(ws.id,'0',{name:'Original setup',command:'printf original > setup-result',runOnWorktreeCreate:true});
 await assert.rejects(rt.chats.createRecord(ws.id,'codex',{isolation:'worktree',expectedSetupRevision:'stale'}),/selection changed/);
 assert.equal((await rt.chats.pendingWorktrees(ws.id)).pending.length,0,'stale UI cannot create even a checkout');
 const index=(rt.chats as unknown as {index:ChatIndex}).index, save=index.save.bind(index);
 index.save=()=>{throw new Error('fixture index publication failed');};
 await assert.rejects(rt.chats.createMemberWorktree(ws.id,worker.id),/记录保存失败/);index.save=save;
 const pending=(await rt.chats.pendingWorktrees(ws.id)).pending;assert.equal(pending.length,1);const original=pending[0]!;
 const intent=await readWorktreeIntent(data,original.sessionId);assert.equal(intent.creation?.originAgentId,worker.id);assert.ok(intent.creation?.setup?.enabled);const plan=intent.creation.setup;
 assert.equal(plan.script.command,'printf original > setup-result');assert.equal(rt.scriptRuns.list(ws.id).runs.length,0);
 await rt.projectScripts.remove(ws.id,saved.revision,saved.scripts[0]!.id);await rt.close();runtime=undefined;
 runtime=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});const cold=runtime;
 const recovered=await cold.chats.recoverWorktreeRecord(ws.id,original.sessionId);assert.equal(recovered.originAgentId,worker.id);assert.equal(recovered.backend,'codex');
 const state=await cold.initialization.get(ws.id,recovered.id);assert.ok(state);assert.equal(state.status,'interrupted');assert.equal(state.requestId,plan.requestId);assert.equal(state.actions.retry,true);assert.equal(state.actions.skip,true);
 assert.equal(existsSync(join(original.path,'setup-result')),false);assert.equal(cold.scriptRuns.list(ws.id).runs.length,0,'pending recovery never auto-runs setup');
 const opened=await cold.chats.open(recovered.id);assert.ok('readOnly' in opened && opened.readOnly);assert.equal(cold.scriptRuns.list(ws.id).runs.length,0,'opening never replays');
 await cold.initialization.retry(ws.id,recovered.id,state.version);
 const until=Date.now()+10000;for(;;){const result=await cold.initialization.get(ws.id,recovered.id);if(result?.status==='succeeded')break;if(Date.now()>until)throw new Error('retry timeout');await new Promise(done=>setTimeout(done,20));}
 assert.equal(await readFile(join(original.path,'setup-result'),'utf8'),'original');assert.equal(cold.scriptRuns.list(ws.id).runs.length,1);
 // Repeated creation publication cannot start a second native command.
 await cold.initialization.created(ws.id,recovered.id);assert.equal(cold.scriptRuns.list(ws.id).runs.length,1);
 // A missing manifest cannot hide a still-owned live setup from safe cancellation.
 const currentSettings=await cold.projectScripts.list(ws.id);
 await cold.projectScripts.save(ws.id,currentSettings.revision,{name:'Owned silent',command:'sleep 60',runOnWorktreeCreate:true});
 const silent=await cold.chats.createRecord(ws.id,'codex',{isolation:'worktree'});
 let live=await cold.initialization.get(ws.id,silent.id);const liveUntil=Date.now()+10000;
 while(!live?.runId||!live.actions.skip){if(Date.now()>liveUntil)throw new Error('silent setup timeout');await new Promise(done=>setTimeout(done,20));live=await cold.initialization.get(ws.id,silent.id);}
 const manifest=join(data,'project-script-runs',createHash('sha256').update(JSON.stringify([ws.id,live.requestId])).digest('hex')+'.json');
 await rename(manifest,manifest+'.held');
 const stillLive=await cold.initialization.get(ws.id,silent.id);assert.equal(stillLive?.status,'running');assert.ok(stillLive?.actions.skip);
 const skipped=await cold.initialization.skip(ws.id,silent.id,stillLive!.version);assert.equal(skipped?.status,'skipped');
 assert.equal(cold.scriptRuns.get(ws.id,live.runId!).run.status,'cancelled');
 console.log('WORKTREE_INITIALIZATION_RECOVERY_OK: real index publication failure preserves Worker/backend/original request+snapshot, cold recovery/open never run, explicit retry works after saved script deletion; stale UI selection rejected before checkout.');
} finally {await runtime?.close();await rm(dir,{recursive:true,force:true});}
