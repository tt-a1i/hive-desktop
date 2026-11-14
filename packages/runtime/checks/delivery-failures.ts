/** Real Pi SDK + Hive HTTP/SQLite + durable queue; loopback Responses peer, no inference. */
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {mkdtemp,mkdir,readFile,writeFile,rename,rmdir,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {ChatIndex} from '../src/chat-index.js';
import {assertFileReference} from './helpers/file-references.js';

const dir=await mkdtemp(join(tmpdir(),'hive-delivery-failures-'));
const pi=join(dir,'pi');await mkdir(pi);
const data=join(dir,'desktop');const indexPath=join(data,'chats','index.json');
const events:any[]=[];const requests:any[]=[];
let mode:'reject'|'success'='reject';let duringRequest:(()=>Promise<void>)|undefined;
const server=createServer(async(req,res)=>{
 try {
  const chunks=[];for await(const chunk of req)chunks.push(chunk);
  requests.push(JSON.parse(Buffer.concat(chunks).toString()));
  await duringRequest?.();
  if(mode==='reject'){
   res.writeHead(400,{'content-type':'application/json'});
   res.end(JSON.stringify({error:{message:'Deliberate fixture rejection: 原始错误',type:'invalid_request_error'}}));return;
  }
  res.writeHead(200,{'content-type':'text/event-stream'});
  const item={id:'msg_fixture',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'Retry completed with original input.',annotations:[]}]};
  const stream=[{type:'response.created',response:{id:'resp_fixture'}},
   {type:'response.output_item.added',output_index:0,item:{...item,content:[]}},
   {type:'response.output_text.delta',output_index:0,content_index:0,delta:item.content[0]!.text},
   {type:'response.output_item.done',output_index:0,item},
   {type:'response.completed',response:{id:'resp_fixture',status:'completed',output:[item],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}];
  for(const event of stream)res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
  res.end();
 } catch(error){res.destroy(error as Error);}
});
server.listen(0,'127.0.0.1');await once(server,'listening');const address=server.address();assert.ok(address&&typeof address!=='string');
const previous=process.env.PI_CODING_AGENT_DIR;process.env.PI_CODING_AGENT_DIR=pi;
await writeFile(join(pi,'settings.json'),JSON.stringify({defaultProvider:'failure-test',defaultModel:'gpt-6.1-terra'}));
await writeFile(join(pi,'models.json'),JSON.stringify({providers:{'failure-test':{api:'openai-responses',apiKey:'loopback-only',baseUrl:`http://127.0.0.1:${address.port}/v1`,models:[{id:'gpt-6.1-terra',name:'Terra failure loopback',input:['text','image']}]}}}));
const {createDesktopRuntime}=await import('../src/desktop-runtime.js');let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
async function wait(check:()=>Promise<boolean>){const end=Date.now()+12000;while(!await check()){assert.ok(Date.now()<end,'Failure/retry flow timed out');await delay(15);}}
async function queued(){const files=await readdir(join(data,'chats','deliveries'));const queues=await Promise.all(files.filter(f=>f.endsWith('.json')).map(async f=>JSON.parse(await readFile(join(data,'chats','deliveries',f),'utf8'))));return queues.flat();}
try{
 runtime=await createDesktopRuntime(data,event=>events.push(event));
 const project=join(dir,'project');await mkdir(project);
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Failures',autostart_orchestrator:false}) as {id:string};
 const file=join(dir,'requirements.txt');await writeFile(file,'Original attachment bytes');const attachment=await runtime.chats.attachments.add(ws.id,file);
 const png=join(dir,'tiny.png');await writeFile(png,Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC','base64'));
 const image=await runtime.chats.attachments.add(ws.id,png);
 const chat=await runtime.chats.create(ws.id,'pi');const text='保留原文\nexact trailing newline\n';
 await runtime.chats.prompt(chat.id,text,[attachment.attachment.id,image.attachment.id]);
 await wait(async()=>requests.length===1&&!(await runtime!.chats.open(chat.id)).busy);
 const first=await runtime.chats.open(chat.id);assert.equal(first.pending,1);assert.ok(first.deliveryFailure?.message.includes('原始错误'));
 const originalQueue=await queued();assert.equal(originalQueue.length,1);assert.equal(first.deliveryFailure.deliveryId,originalQueue[0].id);
 assert.equal(originalQueue[0].presentation.displayText,text);assert.equal(originalQueue[0].images.length,1);assert.equal(originalQueue[0].presentation.attachments.length,2);
 assert.ok(first.history.some((m:any)=>m.stopReason==='error'));assert.equal(first.history.filter((m:any)=>m.presentation?.deliveryId===originalQueue[0].id).length,1);
 const persisted=JSON.parse(await readFile(indexPath,'utf8'));assert.deepEqual(persisted[0].lastDeliveryFailure,first.deliveryFailure);
 await runtime.close();runtime=await createDesktopRuntime(data,event=>events.push(event));
 const cold=await runtime.chats.open(chat.id);assert.deepEqual(cold.deliveryFailure,first.deliveryFailure);assert.equal(cold.pending,1);assert.equal(requests.length,1);assert.deepEqual(await queued(),originalQueue);
 mode='success';await runtime.chats.resume(chat.id);await wait(async()=>requests.length>=2&&!(await runtime!.chats.open(chat.id)).busy);
 const success=await runtime.chats.open(chat.id);assert.equal(success.pending,0);assert.equal(success.deliveryFailure,undefined);assert.deepEqual(await queued(),[]);
 assert.ok(success.history.some((m:any)=>m.role==='assistant'&&m.stopReason==='stop'&&JSON.stringify(m.content).includes('Retry completed with original input.')));
 const attempts=success.history.filter((m:any)=>m.presentation?.deliveryId===originalQueue[0].id) as any[];assert.equal(attempts.length,2);assert.ok(attempts.every(m=>m.presentation.displayText===text));assert.equal(new Set(attempts.map(m=>m.presentation.deliveryId)).size,1,'Native attempts retain the same logical delivery identity');
 assertFileReference(requests[1],attachment);assert.ok(JSON.stringify(requests[1]).includes('input_image'));
 assert.equal(await readFile(attachment.filePath!,'utf8'),'Original attachment bytes');
 // Persisting diagnostics fails at the actual atomic rename; the accepted queue must survive.
 const beforeSecond=requests.length;mode='reject';duringRequest=async()=>{duringRequest=undefined;await rename(indexPath,`${indexPath}.saved`);await mkdir(indexPath);};
 await runtime.chats.prompt(chat.id,'second delivery');await wait(async()=>requests.length>beforeSecond&&!(await runtime!.chats.open(chat.id)).busy);
 const warning=events.find(e=>e.type==='error'&&e.diagnosticWriteFailed);assert.ok(warning);assert.ok(warning.message.includes('原始错误'));
 const unsaved=await runtime.chats.open(chat.id);assert.equal(unsaved.pending,1);assert.equal(unsaved.deliveryFailure,undefined,'Stale prior diagnostic cannot attach to a new head');
 const secondQueue=await queued();assert.equal(secondQueue.length,1);assert.equal(secondQueue[0].presentation.displayText,'second delivery');
 await rmdir(indexPath);await rename(`${indexPath}.saved`,indexPath);
 await runtime.close();runtime=await createDesktopRuntime(data,event=>events.push(event));const restored=await runtime.chats.open(chat.id);assert.equal(restored.pending,1);assert.equal(restored.deliveryFailure,undefined);const beforeRestartRetry=requests.length;
 await runtime.chats.resume(chat.id);await wait(async()=>requests.length>beforeRestartRetry&&!(await runtime!.chats.open(chat.id)).busy);
 const retried=await runtime.chats.open(chat.id);assert.equal(retried.deliveryFailure?.deliveryId,secondQueue[0].id);assert.ok(retried.deliveryFailure?.message.includes('原始错误'));assert.deepEqual((await queued())[0],secondQueue[0]);
 const beforeFinal=requests.length;mode='success';await runtime.chats.resume(chat.id);await wait(async()=>requests.length>beforeFinal&&!(await runtime!.chats.open(chat.id)).busy);assert.equal((await runtime.chats.open(chat.id)).pending,0);assert.equal((await runtime.chats.open(chat.id)).deliveryFailure,undefined);
 const afterFinal=requests.length;await runtime.close();runtime=await createDesktopRuntime(data,event=>events.push(event));assert.equal((await runtime.chats.open(chat.id)).deliveryFailure,undefined);assert.equal(requests.length,afterFinal);
 const schema=join(dir,'schema');await mkdir(schema);const record={id:'one',workspaceId:'ws',title:'old',updatedAt:1,sessionFile:null};
 await writeFile(join(schema,'index.json'),JSON.stringify([record]));assert.equal(new ChatIndex(schema).get('one').lastDeliveryFailure,undefined);
 for(const invalid of [null,[],{}, {deliveryId:'',message:'bad'},{deliveryId:'x',message:2}]){await writeFile(join(schema,'index.json'),JSON.stringify([{...record,lastDeliveryFailure:invalid}]));assert.throws(()=>new ChatIndex(schema));}
 const output=resolve('../../.artifacts/delivery-failures');await mkdir(output,{recursive:true});await writeFile(join(output,'core-receipt.json'),JSON.stringify({pass:true,dir,loopbackRequests:requests.length,nativePiSdk:true,realHive:true,vendorInference:false,retryOriginalDelivery:true,imageAndFilePreserved:true,indexWriteFailureSurfaced:true},null,2));
 console.log('DELIVERY_FAILURES_OK: durable error/reopen, exact delivery+file+image retry, real SDK success, disk failure warning, stale-error isolation. Loopback peer only.');
}finally{await runtime?.close();server.close();server.closeAllConnections();if(previous===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=previous;}
