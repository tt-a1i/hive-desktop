/** Real HTTP/SQLite, SDK/external RPC and transcript failures; no vendor inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rename, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createServer } from 'node:http';
import { once } from 'node:events';
const directory = await mkdtemp(join(tmpdir(), 'hive-reasoning-'));
const agentDir=join(directory,'credentials');await mkdir(agentDir);
const requests:Record<string,unknown>[]=[];let arrived:(()=>void)|undefined;
const server=createServer(async(request,_response)=>{let body='';for await(const chunk of request)body+=chunk;requests.push(JSON.parse(body));arrived?.();});
server.listen(0,'127.0.0.1');await once(server,'listening');const address=server.address();assert.ok(address&&typeof address!=='string');
const settings=JSON.stringify({defaultProvider:'reasoning-local',defaultModel:'gpt-6.1-terra',defaultThinkingLevel:'low'});
await writeFile(join(agentDir,'settings.json'),settings);
const provider={api:'openai-responses',apiKey:'fixture-only',baseUrl:`http://127.0.0.1:${address.port}/v1`,models:[{id:'gpt-6.1-terra',name:'Terra isolated reasoning',reasoning:true,input:['text']}]};
await writeFile(join(agentDir,'models.json'),JSON.stringify({providers:{'reasoning-local':provider,'reasoning-spare':provider}}));
const previousDir=process.env.PI_CODING_AGENT_DIR;const previousOffline=process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR=agentDir;process.env.PI_OFFLINE='1';
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 for(const backend of ['pi','pi-rpc'] as const) {
  const stateDir=join(directory,backend);runtime=await createDesktopRuntime(stateDir,()=>{});
  const path=join(directory,backend+'-project');await mkdir(path);
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path,name:backend,autostart_orchestrator:false})as{id:string};
  const chat=await runtime.chats.create(workspace.id,backend);const id=chat.id;
  assert.equal(chat.model?.id,'gpt-6.1-terra');assert.equal(chat.model?.provider,'reasoning-local');
  const before=await runtime.chats.reasoning(id);assert.ok(before.levels.includes('low'));assert.ok(before.levels.includes('high'));
  assert.equal((await runtime.chats.reasoning(id,'high')).level,'high');
  await assert.rejects(runtime.chats.reasoning(id,'not-a-level'),TypeError);
  await assert.rejects(runtime.chats.reasoning(id,true),TypeError);
  assert.equal((await runtime.chats.reasoning(id)).level,'high');
  assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
  const file=(await runtime.chats.open(id)).sessionFile;assert.ok(file);
  let durable=await readFile(file,'utf8');assert.ok(durable.includes('"thinkingLevel":"high"'));
  await rename(file,file+'.model-retained');await mkdir(file);
  try {await assert.rejects(runtime.chats.setModel(id,'reasoning-spare','gpt-6.1-terra'));}
  finally {await rm(file,{recursive:true});await rename(file+'.model-retained',file);}
  assert.equal(await readFile(file,'utf8'),durable,'Failed model append must not change the saved transcript');
  assert.equal((await runtime.chats.open(id)).model?.provider,'reasoning-local','Failed model persistence must not leave the live engine on an uncommitted provider');
  assert.equal((await runtime.chats.reasoning(id)).level,'high');
  // External Pi appends its current process tool binding when reopening.
  durable=await readFile(file,'utf8');
  await rename(file,file+'.retained');await mkdir(file);
  try { await assert.rejects(runtime.chats.reasoning(id,'medium')); }
  finally{await rm(file,{recursive:true});await rename(file+'.retained',file);}
  assert.equal(await readFile(file,'utf8'),durable,'Failed native append must leave the durable transcript unchanged');
  assert.equal((await runtime.chats.reasoning(id)).level,'high','Retry must reload a healthy native session, not reuse a failed transcript branch');
  assert.equal((await runtime.chats.reasoning(id,'medium')).level,'medium');
  await runtime.close();runtime=await createDesktopRuntime(stateDir,()=>{});
  assert.equal((await runtime.chats.reasoning(id)).level,'medium',`${backend}: native transcript must restore the selected level`);
  assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
  await assert.rejects(runtime.chats.reasoning('missing-session'));
  await runtime.chats.setModel(id,'reasoning-local','gpt-6.1-terra');
  assert.equal((await runtime.chats.reasoning(id)).level,'medium','Re-selecting the current Pi model must preserve its reasoning level');
  let ready!:()=>void;const called=new Promise<void>(resolve=>{ready=resolve;});arrived=ready;
  await runtime.chats.prompt(id,'仅用于本机繁忙状态验证，请勿调用工具');
  let timeout:ReturnType<typeof setTimeout>|undefined;
  try { await Promise.race([called,new Promise((_,reject)=>{timeout=setTimeout(()=>reject(new Error('Local provider was not reached')),10_000);})]); }
  finally {if(timeout)clearTimeout(timeout);}
  const busy=await runtime.chats.open(id);assert.equal(busy.busy,true);
  await assert.rejects(runtime.chats.reasoning(id,'high'));
  assert.equal((await runtime.chats.reasoning(id)).level,'medium');
  assert.equal((await runtime.chats.open(id)).runId,busy.runId,'Rejected busy mutation must not retire the running team session');
  await runtime.chats.abort(id);assert.equal((await runtime.chats.open(id)).busy,false);
  assert.equal(requests.at(-1)?.model,'gpt-6.1-terra');
  await runtime.close();runtime=undefined;
 }
 assert.equal(requests.length,2);
 console.log('PI_REASONING_OK: real Hive HTTP/SQLite, SDK/external Pi RPC, capabilities, transcript restart, disk-failure retirement/retry and unchanged global defaults. Two blocked loopback requests only; no vendor inference.');
}finally {
 await runtime?.close();
 server.closeAllConnections();await new Promise<void>((resolve,reject)=>server.close(error=>error?reject(error):resolve()));
 if(previousDir===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=previousDir;
 if(previousOffline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=previousOffline;
 await rm(directory,{recursive:true,force:true});
}
