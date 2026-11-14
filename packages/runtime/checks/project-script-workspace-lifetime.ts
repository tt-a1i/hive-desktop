/** Actual Hive store + AgentManager PTYs: deletion and every shell start share one lifetime gate. */
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {existsSync} from 'node:fs';
import {mkdtemp,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createRuntimeStore} from '@tt-a1i/hive/dist/src/server/runtime-store.js';
import {createAgentManager} from '@tt-a1i/hive/dist/src/server/agent-manager.js';
import {Database} from '@tt-a1i/hive/dist/src/server/sqlite.js';
if(process.platform==='win32')throw new Error('POSIX native fixture only');
const directory=await mkdtemp(join(tmpdir(),'hive-script-workspace-lifetime-')),project=join(directory,'project'),other=join(directory,'other'),data=join(directory,'state');await mkdir(project);await mkdir(other);
const originalShell=process.env.SHELL;process.env.SHELL='/bin/sh'; // Fixture process only; no login/profile/global config.
const manager=createAgentManager(),store=createRuntimeStore({dataDir:data,agentManager:manager});
const originalStart=manager.startAgent.bind(manager);
const gone=async(pid:number)=>{const end=Date.now()+10000;for(;;){try{process.kill(pid,0);}catch(error){if((error as NodeJS.ErrnoException).code==='ESRCH')return;throw error;}if(Date.now()>end)throw Error('Owned native PID did not exit');await new Promise(done=>setTimeout(done,15));}};
const observer=()=>({onCreated:(_id:string)=>{},onOutput:(_chunk:string)=>{},onExit:(_code:number|null)=>{}});
const create=(path=project)=>store.createWorkspace(path,'lifetime-fixture');
const script=(workspaceId:string,cwd=project,command='sleep 30',scriptRunId=randomUUID())=>store.startSavedScript(workspaceId,{scriptRunId,cwd,command},observer());
try{
 const otherWs=create(other),protectedRun=await script(otherWs.id,other);const otherPid=protectedRun.pid!;process.kill(otherPid,0);
 const ws=create();
 const existing=await script(ws.id),interactive=await store.startWorkspaceShell(ws.id),session=await store.startSessionShell(ws.id,{sessionId:randomUUID(),cwd:project});
 const pids=[existing.pid!,interactive.pid!,session.pid!];for(const pid of pids)process.kill(pid,0);
 const marker=join(project,'forbidden-after-delete');
 const deleting=store.deleteWorkspace(ws.id);
 const refused=[script(ws.id,project,`printf forbidden > '${marker}'`),store.startWorkspaceShell(ws.id),store.startSessionShell(ws.id,{sessionId:randomUUID(),cwd:project})];
 for(const pending of refused){assert.ok(pending instanceof Promise,'closing starts retain the existing Promise rejection contract');await assert.rejects(pending,/SHELL_WORKSPACE_CLOSING|not found|不存在/i);}
 await deleting;for(const pid of pids)await gone(pid);assert.equal(existsSync(marker),false);assert.equal(store.listWorkspaces().some(w=>w.id===ws.id),false);process.kill(otherPid,0);
 await assert.rejects(async()=>script(ws.id),/not found|不存在/i);
 // A start that already entered the gate may be delayed in the real manager; deletion must await and reap it.
 const delayedWs=create(),scriptRunId=randomUUID();let release!:()=>void,entered!:()=>void;
 const blocked=new Promise<void>(done=>{release=done;}),inside=new Promise<void>(done=>{entered=done;});let delayedPid:number|undefined;
 manager.startAgent=async input=>{if(input.agentId===`${delayedWs.id}:script:${scriptRunId}:shell`){entered();await blocked;const run=await originalStart(input);delayedPid=run.pid!;return run;}return originalStart(input);};
 const entering=script(delayedWs.id,project,`(sleep 0.5; printf orphan > '${join(project,'late-child')}') & wait`,scriptRunId);await inside;
 let deleted=false;const delayedDelete=store.deleteWorkspace(delayedWs.id).then(()=>{deleted=true;});await new Promise(done=>setTimeout(done,30));assert.equal(deleted,false);
 await assert.rejects(async()=>store.startWorkspaceShell(delayedWs.id),/SHELL_WORKSPACE_CLOSING/);
 release();await entering;await delayedDelete;assert.ok(delayedPid);await gone(delayedPid);await new Promise(done=>setTimeout(done,650));assert.equal(existsSync(join(project,'late-child')),false);process.kill(otherPid,0);manager.startAgent=originalStart;
 // Real SQLite failure rolls deletion back after all known owned PTYs exit, then permits a fresh explicit start.
 const failedWs=create(),beforeFailure=await script(failedWs.id),failedPid=beforeFailure.pid!;
 const db=new Database(join(data,'runtime.sqlite'));
 const escaped=failedWs.id.replaceAll("'","''");db.exec(`CREATE TRIGGER fixture_delete_abort BEFORE DELETE ON workspaces WHEN OLD.id='${escaped}' BEGIN SELECT RAISE(ABORT,'fixture delete blocked'); END`);
 await assert.rejects(store.deleteWorkspace(failedWs.id),/fixture delete blocked/);await gone(failedPid);assert.ok(store.listWorkspaces().some(w=>w.id===failedWs.id));process.kill(otherPid,0);
 db.exec('DROP TRIGGER fixture_delete_abort');db.close();
 const fresh=await script(failedWs.id);assert.ok(fresh.pid);process.kill(fresh.pid!,0);await store.deleteWorkspace(failedWs.id);await gone(fresh.pid!);
 await store.deleteWorkspace(otherWs.id);await gone(otherPid);
 console.log('PROJECT_SCRIPT_WORKSPACE_LIFETIME_OK: synchronous deletion gate rejects script/workspace/session starts; actual prior+delayed PTYs and child trees exit before return; unrelated project survives; real SQLite failure releases gate after cleanup.');
}finally{manager.startAgent=originalStart;await store.close();if(originalShell===undefined)delete process.env.SHELL;else process.env.SHELL=originalShell;await rm(directory,{recursive:true,force:true});}
