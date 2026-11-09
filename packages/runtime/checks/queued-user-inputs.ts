/** Real Pi SDK or original external Pi RPC, Hive HTTP/SQLite and held loopback HTTP. No vendor inference. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp,mkdir,writeFile,rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const backend=process.env.HIVE_QUEUE_BACKEND??'pi';assert.ok(backend==='pi'||backend==='pi-rpc');
const dir=await mkdtemp(join(tmpdir(),'hive-queued-inputs-')),agent=join(dir,'agent'),data=join(dir,'state');await mkdir(agent);
const requests:any[]=[],events:any[]=[];let release!:()=>void;const hold=new Promise<void>(r=>release=r);
const server=createServer(async(req,res)=>{
 try{
  const chunks=[];for await(const chunk of req)chunks.push(chunk);const body=JSON.parse(Buffer.concat(chunks).toString());requests.push(body);const n=requests.length;
  if(n===1)await hold;if(res.destroyed)return;
  const item={id:`msg_${n}`,type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:`Queued input response ${n}`,annotations:[]}]};
  res.writeHead(200,{'content-type':'text/event-stream'});
  for(const event of [{type:'response.created',response:{id:`resp_${n}`}},{type:'response.output_item.added',output_index:0,item:{...item,content:[]}},{type:'response.output_text.delta',output_index:0,content_index:0,delta:item.content[0]!.text},{type:'response.output_item.done',output_index:0,item},{type:'response.completed',response:{id:`resp_${n}`,status:'completed',output:[item],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}])res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);res.end();
 }catch(error){res.destroy(error as Error);}
});server.listen(0,'127.0.0.1');await once(server,'listening');const address=server.address();assert.ok(address&&typeof address!=='string');
const previous=process.env.PI_CODING_AGENT_DIR,offline=process.env.PI_OFFLINE;process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
await writeFile(join(agent,'settings.json'),JSON.stringify({defaultProvider:'queue-test',defaultModel:'gpt-6.1-terra'}));
await writeFile(join(agent,'models.json'),JSON.stringify({providers:{'queue-test':{api:'openai-responses',apiKey:'loopback',baseUrl:`http://127.0.0.1:${address.port}/v1`,models:[{id:'gpt-6.1-terra',input:['text','image']}]}}}));
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
async function wait(predicate:()=>Promise<boolean>){const end=Date.now()+12000;while(!await predicate()){assert.ok(Date.now()<end,'Queue contract timed out');await delay(10);}}
try{
 runtime=await createDesktopRuntime(data,e=>events.push(e));const workspace=await runtime.hive.request('POST','/api/workspaces',{path:dir,name:'Queued inputs',autostart_orchestrator:false})as{id:string};
 const chat=await runtime.chats.create(workspace.id,backend);await runtime.chats.prompt(chat.id,'First working message');await wait(async()=>requests.length===1&&(await runtime!.chats.open(chat.id)).busy);
 const file=join(dir,'details.txt');await writeFile(file,'Queued attachment content');const staged=await runtime.chats.attachments.add(workspace.id,file);
 const followup='补充路径 src/main.ts\n保留原始空行\n';await runtime.chats.prompt(chat.id,followup,[staged.attachment.id]);
 const queued=await runtime.chats.open(chat.id);assert.equal(requests.length,1,'Follow-up must not interrupt or start concurrently');assert.equal(queued.pendingUserInputs.length,2);
 await assert.rejects(runtime.chats.prompt(chat.id,'Rejected follow-up',['00000000-0000-4000-8000-000000000000']));
 const unchanged=await runtime.chats.open(chat.id);assert.equal(unchanged.busy,true);assert.equal(unchanged.pendingUserInputs.length,2);assert.equal(requests.length,1);
 const second=queued.pendingUserInputs[1]!;assert.equal(second.text,followup);assert.equal(second.processing,false);assert.equal(second.attachments[0]!.name,'details.txt');assert.ok(!second.text.includes('<!-- hive-desktop-images:'));assert.equal(queued.pendingUserInputs[0]!.processing,true);
 await runtime.agents.deliverSystemMessageToAgent(workspace.id,`${workspace.id}:orchestrator`,'PRIVATE AUTOMATED CONTEXT',{requireActiveRun:true,presentation:{version:1,origin:'system',displayText:'PRIVATE AUTOMATED CONTEXT'}});
 assert.equal((await runtime.chats.open(chat.id)).pendingUserInputs.length,2,'Automated context must not appear as explicit user input');
 assert.ok(events.some(e=>e.type==='state'&&e.pendingUserInputs?.some((i:any)=>i.deliveryId===second.deliveryId&&i.text===followup)));
 await runtime.chats.abort(chat.id);release();const paused=await runtime.chats.open(chat.id);assert.equal(paused.busy,false);assert.equal(paused.pendingUserInputs.length,1);assert.equal(paused.pendingUserInputs[0]!.deliveryId,second.deliveryId);
 await runtime.close();runtime=await createDesktopRuntime(data,e=>events.push(e));const cold=await runtime.chats.open(chat.id);assert.equal(cold.pendingUserInputs[0]!.deliveryId,second.deliveryId);assert.equal(cold.pendingUserInputs[0]!.text,followup);assert.equal(cold.pendingUserInputs[0]!.attachments[0]!.id,staged.attachment.id);assert.equal(requests.length,1,'Cold open must not resume paused input');
 await runtime.chats.resume(chat.id);await wait(async()=>!(await runtime!.chats.open(chat.id)).busy&&(await runtime!.chats.open(chat.id)).pending===0);
 const done=await runtime.chats.open(chat.id);assert.deepEqual(done.pendingUserInputs,[]);assert.equal(requests.length,3,'Recovery context must join queued work, not trigger a redundant model turn');assert.ok(JSON.stringify(requests[1]).includes('src/main.ts'));assert.ok(JSON.stringify(requests[1]).includes('Current member profile'),'The queued turn must still receive current recovery facts');
 assert.equal(done.history.filter((m:any)=>m.presentation?.deliveryId===second.deliveryId).length,1);
 await runtime.close();runtime=await createDesktopRuntime(data,e=>events.push(e));await runtime.chats.open(chat.id);assert.equal(requests.length,3);
 await runtime.chats.resume(chat.id);await wait(async()=>!(await runtime!.chats.open(chat.id)).busy&&(await runtime!.chats.open(chat.id)).pending===0);
 assert.equal(requests.length,4,'Recovery without queued work must still wake the model');assert.ok(JSON.stringify(requests[3]).includes('Current member profile'));
 console.log(backend,'QUEUED_USER_INPUTS_OK: real SDK/Hive/SQLite, held current request, durable follow-up+attachment, explicit-source filtering/state, stop preservation, paused cold restore, sequential unique consumption. Loopback only.');
}finally{release();await runtime?.close();server.close();server.closeAllConnections();if(previous===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=previous;if(offline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=offline;await rm(dir,{recursive:true,force:true});}
