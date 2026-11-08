/** Real built-in Pi SDK and optional native external Pi, loopback-only synthetic provider. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp,mkdir,readFile,writeFile,rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createPiSession } from '../src/agents/pi.js';
import { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';
const image={type:'image' as const,mimeType:'image/png',data:'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC'};
const directory=await mkdtemp(join(tmpdir(),'hive-images-pi-'));const agentDir=join(directory,'agent');await mkdir(agentDir);
const requests:any[]=[];
const server=createServer(async(request,response)=>{const parts:Buffer[]=[];for await(const chunk of request)parts.push(Buffer.from(chunk));requests.push(JSON.parse(Buffer.concat(parts).toString('utf8')));response.writeHead(400,{'content-type':'application/json'});response.end(JSON.stringify({error:{type:'invalid_request_error',message:'Intentional loopback fixture stop after image capture'}}));});
server.listen(0,'127.0.0.1');await once(server,'listening');const address=server.address();assert.ok(address&&typeof address!=='string');
const config={providers:{'hive-images':{baseUrl:`http://127.0.0.1:${address.port}/v1`,api:'openai-responses',apiKey:'local-fixture',models:[{id:'gpt-6.1-terra',name:'Terra loopback image test',input:['text','image']}]}}};
const settings=JSON.stringify({defaultProvider:'hive-images',defaultModel:'gpt-6.1-terra'});await writeFile(join(agentDir,'settings.json'),settings);await writeFile(join(agentDir,'models.json'),JSON.stringify(config));
let handle:PiSessionHandle|undefined;
const previousDir=process.env.PI_CODING_AGENT_DIR,previousOffline=process.env.PI_OFFLINE;
try{
 for(const backend of process.argv.includes('--external')?['pi','pi-rpc']:['pi']){
  const start=requests.length;const sessionDir=join(directory,backend);
  const options={cwd:directory,sessionDir,agentDir,model:{provider:'hive-images',id:'gpt-6.1-terra'},onEvent(){}};
  if(backend==='pi')handle=await createPiSession(options);
  else{process.env.PI_CODING_AGENT_DIR=agentDir;process.env.PI_OFFLINE='1';const {createExternalPiSession}=await import('../src/agents/external-pi.js');handle=await createExternalPiSession({...options,command:process.env.HIVE_PI_COMMAND??'/Users/tsk/Library/pnpm/pi'});}
  assert.equal(handle.currentModel()?.id,'gpt-6.1-terra');
  const before=handle.history().length;
  await assert.rejects(handle.prompt('invalid',{images:[{...image,data:'bad'}]}));assert.equal(requests.length,start);assert.equal(handle.history().length,before);
  let failed!: (error:Error)=>void;const failure=new Promise<Error>(resolve=>{failed=resolve;});
  const queue=new DeliveryQueue({directory:join(directory,'deliveries'),sessionId:backend,startPaused:true,completedIds:()=>handle!.completedDeliveryIds(),execute:item=>handle!.prompt(item.text,{...(item.images && {images:item.images}),automated:true,deliveryId:item.id,...(item.presentation && {presentation:item.presentation})}),onError:failed});
  await queue.enqueue('Describe original image',true,{version:1,origin:'user',displayText:'Describe original image'},[image]);
  const deliveryId=queue.pending[0]!.id;queue.resume();
  let timer:ReturnType<typeof setTimeout>|undefined;
  try{await Promise.race([failure,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Loopback image turn timed out')),10000);})]);}finally{if(timer)clearTimeout(timer);}
  await queue.idle();assert.equal(queue.pendingCount,1);assert.deepEqual(queue.pending[0]!.images,[image]);
  assert.equal(requests.length,start+1,'Use one synthetic HTTP request, never vendor inference');
  const blocks=requests[start].input.flatMap((item:any)=>item.content??[]);
  assert.ok(blocks.some((part:any)=>part.type==='input_image'&&part.image_url===`data:${image.mimeType};base64,${image.data}`),'Real Pi provider must receive original image bytes');
  const record=handle.snapshot().history.find((message:any)=>message.presentation?.deliveryId===deliveryId) as {content:unknown};
  assert.equal((record as any).role,'user');
  assert.deepEqual(record.content,[{type:'text',text:'Describe original image'},image]);
  assert.equal(handle.completedDeliveryIds().includes(deliveryId),false,'Rejected provider turn must stay retryable');
  const file=handle.sessionFile!;assert.ok(file);await handle.dispose();handle=undefined;
  if(backend==='pi'){
   handle=await createPiSession({...options,sessionFile:file});
   assert.deepEqual((handle.snapshot().history.find((message:any)=>message.presentation?.deliveryId===deliveryId) as {content:unknown}).content,record.content);
   config.providers['hive-images'].models[0]!.input=['text'];await writeFile(join(agentDir,'models.json'),JSON.stringify(config));await handle.refreshModels!();
   const count=handle.history().length;await assert.rejects(handle.prompt('No vision',{images:[image]}));assert.equal(handle.history().length,count);assert.equal(requests.length,start+1);
   config.providers['hive-images'].models[0]!.input=['text','image'];await writeFile(join(agentDir,'models.json'),JSON.stringify(config));
  }else{
   const {createExternalPiSession}=await import('../src/agents/external-pi.js');handle=await createExternalPiSession({...options,sessionFile:file,command:process.env.HIVE_PI_COMMAND??'/Users/tsk/Library/pnpm/pi'});
   assert.deepEqual((handle.snapshot().history.find((message:any)=>message.presentation?.deliveryId===deliveryId) as {content:unknown}).content,record.content);
  }
  await handle.dispose();handle=undefined;assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
 }
 // Exercise the actual Chats structured-port acceptance and durable queue.
 process.env.PI_CODING_AGENT_DIR=agentDir;process.env.PI_OFFLINE='1';
 const {createDesktopRuntime}=await import('../src/desktop-runtime.js');
 let failedPort!:()=>void;const portError=new Promise<void>(resolve=>{failedPort=resolve;});
 const runtime=await createDesktopRuntime(join(directory,'desktop'),event=>{if(event.type==='error')failedPort();});
 try{
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:directory,name:'Images port',autostart_orchestrator:false}) as import('../src/hive-runtime-contract.js').WorkspaceSummary;
  const member=await runtime.hive.request('POST',`/api/workspaces/${workspace.id}/workers`,{name:'Image worker',role:'reviewer',autostart:false}) as {id:string};
  const port=await runtime.chats.start({workspace,agentId:member.id,runId:'image-port-fixture',token:'local-fixture-only',hivePort:String(runtime.hive.port),config:{command:'hive:pi',args:['--provider','hive-images','--model','gpt-6.1-terra']},onExit(){}});
  try{
   const prior=requests.length;
   await port.deliver('Port image',{images:[image],presentation:{version:1,origin:'user',displayText:'Port image'}});
   port.ready!();
   let timer:ReturnType<typeof setTimeout>|undefined;
   try{await Promise.race([portError,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Image port timed out')),10000);})]);}finally{if(timer)clearTimeout(timer);}
   const record=runtime.chats.list(workspace.id).find(record=>record.agentId===member.id)!;
   const snapshot=await runtime.chats.open(record.id);
   assert.equal(snapshot.pending,1);assert.equal(requests.length,prior+1);
   const user=snapshot.history.find((message:any)=>message.role==='user'&&message.presentation?.origin==='user') as {content:unknown};
   assert.deepEqual(user.content,[{type:'text',text:'Port image'},image]);
   await assert.rejects(port.deliver('invalid context',{triggerTurn:false,images:[image]}));
  }finally{await port.stop();}
 }finally{await runtime.close();}
 assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
 console.log('PROMPT_IMAGES_PI_OK: '+(process.argv.includes('--external')?'real native Pi CLI bridge + ':'')+'real Pi SDK/provider encode exact image, durable failed-turn transcript/reopen, no completion acknowledgement on rejection, invalid/text-only model blocks before history/HTTP. Loopback synthetic provider only; no vendor inference.');
}finally{
 await handle?.abort();await handle?.dispose();server.closeAllConnections();await new Promise<void>(resolve=>server.close(()=>resolve()));
 if(previousDir===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=previousDir;
 if(previousOffline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=previousOffline;
 await rm(directory,{recursive:true,force:true});
}
