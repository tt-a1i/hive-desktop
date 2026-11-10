/** Real Hive HTTP/SQLite + Pi SDK history; no inference or mocked storage. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {DatabaseSync} from 'node:sqlite';
const root=await mkdtemp(join(tmpdir(),'hive-deleted-member-'));
const agent=join(root,'pi');await mkdir(agent);
process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
let requests=0;
const server=createServer((_,res)=>{requests++;res.writeHead(503);res.end('Unexpected inference');});
server.listen(0,'127.0.0.1');await once(server,'listening');
const address=server.address();assert.ok(address&&typeof address!=='string');
const provider={api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'owned-no-inference',models:[{id:'gpt-6.1-terra',name:'Terra no-inference fixture',input:['text']}]};
await writeFile(join(agent,'models.json'),JSON.stringify({providers:{openai:provider,'owned-openai':provider}}));
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
type Runtime=Awaited<ReturnType<typeof createDesktopRuntime>>;
let runtime:Runtime|undefined;
const state=join(root,'state');
try {
 runtime=await createDesktopRuntime(state,()=>{});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:root,name:'Deleted member history',autostart_orchestrator:false}) as {id:string};
 async function member(name:string) {
  const worker=await runtime!.createMember(ws.id,{name,backend:'pi',role:'reviewer',description:'No inference history fixture'});
  await runtime!.hive.request('POST',`/api/workspaces/${ws.id}/agents/${worker.id}/config`,{command:'hive:pi',args:['--provider','openai','--model','gpt-6.1-terra']});
  await runtime!.hive.request('POST',`/api/workspaces/${ws.id}/agents/${worker.id}/start`);
  const chat=runtime!.chats.list(ws.id).find(c=>c.agentId===worker.id);assert.ok(chat);
  assert.equal((await runtime!.chats.open(chat.id)).model?.id,'gpt-6.1-terra');
  return {worker,chat};
 }
 const removed=await member('Removed history owner');
 const kept=await member('Preserved history owner');
 await runtime.hive.request('DELETE',`/api/workspaces/${ws.id}/workers/${removed.worker.id}`);
 const saved=await runtime.chats.open(removed.chat.id);assert.ok(saved.history.length>0);
 const absent=()=>{
  assert.equal(runtime!.agents.peekAgentLaunchConfig(ws.id,removed.worker.id),undefined);
  const db=new DatabaseSync(join(state,'hive/runtime.sqlite'),{readOnly:true});
  try {assert.equal((db.prepare('SELECT COUNT(*) n FROM agent_launch_configs WHERE workspace_id=? AND agent_id=?').get(ws.id,removed.worker.id) as {n:number}).n,0);}
  finally {db.close();}
 };
 absent();
 await assert.rejects(runtime.chats.prompt(removed.chat.id,'Must not revive deleted member'));absent();
 await assert.rejects(runtime.chats.resume(removed.chat.id));absent();
 await assert.rejects(runtime.chats.setModel(removed.chat.id,'owned-openai','gpt-6.1-terra'));absent();
 assert.deepEqual((await runtime.chats.open(removed.chat.id)).history,saved.history);
 // The same validated store path must still configure existing members.
 await runtime.chats.setModel(kept.chat.id,'owned-openai','gpt-6.1-terra');
 assert.deepEqual(runtime.agents.peekAgentLaunchConfig(ws.id,kept.worker.id)?.args,['--provider','owned-openai','--model','gpt-6.1-terra']);
 assert.equal(requests,0);
 await runtime.close();runtime=undefined;
 runtime=await createDesktopRuntime(state,()=>{});absent();
 assert.deepEqual((await runtime.chats.open(removed.chat.id)).history,saved.history);
 const members=await runtime.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as {id:string}[];
 assert.deepEqual(members.map(m=>m.id),[kept.worker.id]);
 assert.equal((await runtime.chats.open(kept.chat.id)).model?.provider,'owned-openai');
 assert.equal(requests,0);
 console.log('DELETED_MEMBER_SESSION_OK: rejected prompt/resume/model changes leave no orphan launch config; real history, peer config and cold recovery preserved; zero inference.');
}finally {
 await runtime?.close();await new Promise<void>((resolve,reject)=>server.close(error=>error?reject(error):resolve()));await rm(root,{recursive:true,force:true});
}
