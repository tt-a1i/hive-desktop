/** Real Pi SDK + desktop HTTP/SQLite, owned config and a rejecting loopback endpoint. */
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {mkdtemp,mkdir,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-pi-default-model-')),agent=join(directory,'agent'),project=join(directory,'project'),data=join(directory,'state');
await mkdir(agent);await mkdir(project);let requests=0;
const server=createServer((_req,res)=>{requests++;res.writeHead(500);res.end('Owned check forbids model requests');});await new Promise<void>(resolve=>server.listen(0,'127.0.0.1',resolve));const address=server.address();assert.ok(address&&typeof address==='object');
const before={agent:process.env.PI_CODING_AGENT_DIR,offline:process.env.PI_OFFLINE};process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
const model='gpt-6.1-terra',globalProvider='owned-global-default',projectProvider='owned-project-default';
const provider=(name:string)=>({api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'owned-local-placeholder',models:[{id:model,name,reasoning:true,input:['text'],contextWindow:32768,maxTokens:4096}]});
const definitions={providers:{[globalProvider]:provider('Owned global Terra'),[projectProvider]:provider('Owned project Terra')}};
const settings=JSON.stringify({defaultProvider:globalProvider,defaultModel:model});await writeFile(join(agent,'settings.json'),settings);await writeFile(join(agent,'models.json'),JSON.stringify(definitions));
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try{
 runtime=await createDesktopRuntime(data,()=>{});const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Pi defaults',autostart_orchestrator:false}) as {id:string};
 const first=await runtime.chats.create(ws.id);let state=await runtime.chats.open(first.id);assert.equal(state.model?.provider,globalProvider);assert.equal(state.model?.id,model);
 await mkdir(join(project,'.pi'));const projectSettings=JSON.stringify({defaultProvider:projectProvider,defaultModel:model});await writeFile(join(project,'.pi/settings.json'),projectSettings);
 const second=await runtime.chats.create(ws.id);state=await runtime.chats.open(second.id);assert.equal(state.model?.provider,projectProvider);assert.equal(state.model?.id,model);
 // An explicit per-conversation selection must not rewrite either default.
 await runtime.chats.setModel(second.id,globalProvider,model);assert.equal((await runtime.chats.open(second.id)).model?.provider,globalProvider);
 const third=await runtime.chats.create(ws.id);assert.equal((await runtime.chats.open(third.id)).model?.provider,projectProvider);
 assert.equal(await readFile(join(agent,'settings.json'),'utf8'),settings);assert.equal(await readFile(join(project,'.pi/settings.json'),'utf8'),projectSettings);
 await runtime.close();runtime=await createDesktopRuntime(data,()=>{});
 for(const chat of [first,second]){const restored=await runtime.chats.open(chat.id);assert.equal(restored.model?.provider,globalProvider);assert.equal(restored.model?.id,model);assert.equal(restored.pending,0);}
 assert.equal((await runtime.chats.open(third.id)).model?.provider,projectProvider);
 // Removed saved providers leave a visible unavailable choice; project fallback cannot replace it.
 await runtime.close();runtime=undefined;await writeFile(join(agent,'models.json'),JSON.stringify({providers:{[projectProvider]:definitions.providers[projectProvider]}}));runtime=await createDesktopRuntime(data,()=>{});
 const missing=await runtime.chats.open(first.id);assert.equal(missing.model,undefined);assert.equal(missing.unavailableModel?.provider,globalProvider);assert.equal(missing.unavailableModel?.id,model);assert.equal(missing.pending,0);
 assert.equal((await runtime.chats.open(third.id)).model?.provider,projectProvider);assert.equal(requests,0);
 console.log('PI_DEFAULT_MODEL_OK: global/project defaults, independent conversation selection, cold restore and unavailable saved choice; real SDK/HTTP/SQLite, zero model requests.');
}finally{await runtime?.close();await new Promise<void>(resolve=>server.close(()=>resolve()));if(before.agent===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=before.agent;if(before.offline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=before.offline;await rm(directory,{recursive:true,force:true});}
