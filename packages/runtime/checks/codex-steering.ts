// Real owned JSONL subprocess + durable delivery spool; synthetic native peer, no model.
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm,chmod} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {Type} from 'typebox';
import {createCodexSession} from '../src/agents/codex.js';
import {DeliveryQueue} from '../src/delivery-queue.js';
const root=await mkdtemp(join(tmpdir(),'hive-codex-steering-'));
const peer=String.raw`
const fs=require('node:fs');const {createInterface}=require('node:readline');
let thread=fs.existsSync('native.json')?JSON.parse(fs.readFileSync('native.json','utf8')):{id:'steering-thread',path:null,historyMode:'legacy',turns:[]};let mode,turn,tool;
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');const save=()=>fs.writeFileSync('native.json',JSON.stringify(thread));
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
const user=(text,id)=>{const item={id,type:'userMessage',content:[{type:'text',text}]};turn.items.push(item);notify('item/completed',{turnId:turn.id,item});save();};
const finish=status=>{turn.status=status;if(status==='completed'){const item={id:turn.id+'-final',type:'agentMessage',text:'verified final',phase:'final_answer'};turn.items.push(item);notify('item/completed',{turnId:turn.id,item});}save();notify('turn/completed',{turn});};
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line),answer=result=>send({id:q.id,result}),error=code=>send({id:q.id,error:{code,message:'Controlled native rejection'}});
 if(!q.method){if(q.id===41&&turn.status==='inProgress'){tool.status='completed';tool.success=true;notify('item/completed',{turnId:turn.id,item:tool});finish('completed');}continue;}
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start'||q.method==='thread/resume')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  mode=q.params.input[0].text;turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});user(mode,turn.id+'-input');
  if(mode.startsWith('notice')||mode==='queued user'){finish('completed');continue;}
  tool={id:'held-tool',type:'dynamicToolCall',tool:'hold',status:'inProgress',arguments:{},contentItems:[]};turn.items.push(tool);notify('item/started',{turnId:turn.id,item:tool});save();
  send({id:41,method:'item/tool/call',params:{threadId:thread.id,turnId:turn.id,callId:'held-tool',tool:'hold',arguments:{}}});
 }else if(q.method==='turn/steer'){
  fs.appendFileSync('steers.log',q.params.input[0].text+'\n');
  if(mode==='no-ack'){continue;}
  if(mode==='late'){finish('completed');error(-32600);}
  else if(mode==='reject'){error(-32603);}
  else if(mode==='disconnect'){process.exit(0);}
  else {if(mode==='before-ack')user(q.params.input[0].text,turn.id+'-notice-'+turn.items.length);answer({turnId:turn.id});if(mode==='unpersisted')notify('item/completed',{turnId:turn.id,item:{id:'unpersisted-notice',type:'userMessage',content:[{type:'text',text:q.params.input[0].text}]}});else if(mode!=='missing'&&mode!=='interrupted'&&mode!=='before-ack')user(q.params.input[0].text,turn.id+'-notice-'+turn.items.length);}
 }else if(q.method==='turn/interrupt'){answer({});finish('interrupted');}
}})().catch(e=>{console.error(e);process.exit(1)});
`;
const barrier=()=>{let release!:()=>void;return {promise:new Promise<void>(resolve=>{release=resolve;}),release:()=>release()};};
try{
 for(const origin of ['system','report'] as const) for(const mode of ['normal','before-ack','interrupted','consumed-interrupted','no-ack','missing','unpersisted','reject','late','disconnect','write-failure','user-order']){
  const cwd=join(root,`${origin}-${mode}`);await mkdir(cwd);await writeFile(join(cwd,'package.json'),'{"type":"commonjs"}');await writeFile(join(cwd,'app-server'),peer);
  const atTool=barrier(),releaseTool=barrier();const observed:any[]=[];let signal:AbortSignal|undefined;
  const options={cwd,sessionDir:join(cwd,'metadata'),command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(event:any){observed.push(event);},customTools:[{name:'hold',description:'held host tool',parameters:Type.Object({}),execute:async(_id:string,_args:unknown,input?:AbortSignal)=>{signal=input;atTool.release();await releaseTool.promise;return {content:[{type:'text' as const,text:'verified'}]};}}]};
  const session=await createCodexSession(options);const errors:Error[]=[];const completedReports:string[]=[];
  const notice=(index:number)=>({version:1 as const,origin,displayText:index===1?'first notice':'second notice',...(origin==='report'&&{dispatchId:`dispatch-${index}`,workerId:`worker-${index}`,workerName:`proof-${index}`,artifacts:[`proof-${index}.txt`]})});
  const queue=new DeliveryQueue({directory:join(cwd,'spool'),sessionId:'chat',completedIds:()=>session.completedDeliveryIds(),onError:error=>errors.push(error),onState:(_busy,id)=>{if(id){assert.ok(session.completedDeliveryIds().includes(id));completedReports.push(id);}},
   execute:item=>session.prompt(item.text,{deliveryId:item.id,presentation:item.presentation}),steer:item=>session.steer!(item.text,{deliveryId:item.id,presentation:item.presentation!})});
  try{
   await queue.enqueue(mode,true,{version:1,origin:'user',displayText:mode});await atTool.promise;
   if(mode==='user-order')await queue.enqueue('queued user',true,{version:1,origin:'user',displayText:'queued user'});
   if(mode==='write-failure')await chmod(join(cwd,'metadata'),0o500);
   await queue.enqueue('notice one',true,notice(1));
   if(mode==='normal'||mode==='before-ack')await queue.enqueue('notice two',true,notice(2));
   const accepted=queue.pending;assert.equal(DeliveryQueue.persistedCount(join(cwd,'spool'),'chat'),accepted.length);
   if(origin==='report'&&(mode==='normal'||mode==='before-ack')){assert.deepEqual(queue.pendingReports.map(r=>r.processing),[true,true],'Reports appended to an active turn must show processing');assert.deepEqual(completedReports,[],'Processing is not durable completion');}
   if(origin==='report'&&mode==='user-order')assert.deepEqual(queue.pendingReports.map(r=>r.processing),[false]);
   if(mode==='interrupted'||mode==='consumed-interrupted'||mode==='no-ack'){
    // Wait for actual native acceptance, without consuming the held tool.
    while(!(await readFile(join(cwd,'steers.log'),'utf8').catch(error=>{if(error.code==='ENOENT')return '';throw error;})))await new Promise(resolve=>setImmediate(resolve));
    const before=Date.now();const pausing=queue.pause();const aborting=session.abort();releaseTool.release();await aborting;await pausing;queue.discardCurrent();
    assert.ok(Date.now()-before<5000,'Stop must not wait for the 30-second steer RPC timeout');
    assert.equal(signal?.aborted,true);if(origin==='report')assert.deepEqual(queue.pendingReports.map(r=>r.processing),[false]);assert.equal(queue.pendingCount,1);assert.equal(queue.pending[0]?.text,'notice one');assert.deepEqual(session.completedDeliveryIds(),[]);
    if(mode==='consumed-interrupted'){
     queue.resume();await queue.idle();assert.equal(queue.pendingCount,0);
     const rows=session.snapshot().history.filter((x:any)=>x.customType==='hive-delivery') as any[];
     assert.deepEqual(rows.map(x=>x.presentation.origin),['user',origin,origin]);
     assert.equal(rows[1].presentation.deliveryId,accepted[1].id);assert.equal(rows[2].presentation.deliveryId,accepted[1].id);
    }
   }else{releaseTool.release();await queue.idle();}
   if(mode==='normal'||mode==='before-ack'){
    assert.equal(JSON.parse(await readFile(join(cwd,'native.json'),'utf8')).turns.length,1,`${origin}: notices must join the original turn`);
    assert.equal(queue.pendingCount,0);assert.deepEqual(errors,[]);assert.deepEqual(new Set(session.completedDeliveryIds()),new Set(accepted.map(x=>x.id)));
    const messages=session.snapshot().history.filter((x:any)=>x.customType==='hive-delivery') as any[];
    assert.deepEqual(messages.map(x=>x.presentation.displayText),[mode,'first notice','second notice']);
    assert.deepEqual(messages.map(x=>x.presentation.origin),['user',origin,origin]);
    assert.equal(new Set(messages.map(x=>x.presentation.deliveryId)).size,3);
    assert.deepEqual(messages.slice(1).map(({presentation:{deliveryId,...presentation}}:any)=>presentation),[notice(1),notice(2)]);
    assert.deepEqual(completedReports,origin==='report'?accepted.slice(1).map(x=>x.id):[]);
    assert.ok(!observed.some(event=>event.type==='message_end'&&event.message?.role==='user'&&JSON.stringify(event.message).includes('notice')),'Do not emit a raw user row before assigning its Hive delivery ID');
   }else if(mode==='missing'||mode==='unpersisted'||mode==='reject'){
    assert.equal(queue.pendingCount,1);assert.equal(errors.length,1);assert.equal(queue.pending[0]?.text,'notice one');
    assert.equal(session.completedDeliveryIds().includes(accepted.at(-1)!.id),false);
    queue.resume();await queue.idle();assert.equal(queue.pendingCount,0,'An explicit retry must not be stuck on the failed steering attempt');
   }else if(mode==='late'||mode==='user-order'){
    assert.equal(queue.pendingCount,0);assert.deepEqual(errors,[]);
    const native=JSON.parse(await readFile(join(cwd,'native.json'),'utf8'));
    assert.deepEqual(native.turns.map((x:any)=>x.items[0].content[0].text),mode==='late'?['late','notice one']:['user-order','queued user','notice one']);
    if(mode==='user-order')await assert.rejects(readFile(join(cwd,'steers.log')),{code:'ENOENT'});
   }else if(mode==='disconnect'||mode==='write-failure'){
    assert.equal(queue.pendingCount,2);assert.equal(errors.length,1);assert.equal(session.completedDeliveryIds().length,0);
   }
  }finally{releaseTool.release();if(mode==='write-failure')await chmod(join(cwd,'metadata'),0o700);await session.abort();await queue.pause();await session.dispose();}
  if(mode==='normal'||mode==='before-ack'||mode==='interrupted'||mode==='no-ack'){
   const restored=await createCodexSession({...options,threadId:session.threadId});
   try{
    const cold=new DeliveryQueue({directory:join(cwd,'spool'),sessionId:'chat',startPaused:true,completedIds:()=>restored.completedDeliveryIds(),onError:error=>errors.push(error),execute:async()=>{throw new Error('Cold check must not invoke a model');}});
    assert.equal(cold.pendingCount,(mode==='normal'||mode==='before-ack')?0:1);
    if(origin==='report')assert.ok(cold.pendingReports.every(r=>!r.processing));
    const restoredMessages=restored.snapshot().history.filter((x:any)=>x.customType==='hive-delivery') as any[];
    const origins=restoredMessages.map(x=>x.presentation.origin);
    if(mode==='normal'||mode==='before-ack')assert.deepEqual(restoredMessages.slice(1).map(({presentation:{deliveryId,...presentation}}:any)=>presentation),[notice(1),notice(2)]);
    assert.deepEqual(origins,(mode==='normal'||mode==='before-ack')?['user',origin,origin]:['user']);
   }finally{await restored.dispose();}
  }
 }
 console.log('Codex steering passed: native-item provenance, durable system/report completion/cold recovery and report metadata/acknowledgement, interrupt retention, missing consumption/rejection retry, terminal race, disconnect, metadata write failure and queued user ordering. Synthetic JSONL peer; no model.');
}finally{await rm(root,{recursive:true,force:true});}
