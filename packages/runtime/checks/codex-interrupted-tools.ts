import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createCodexSession} from '../src/agents/codex.js';
import type {PiEvent} from '../src/agents/types.js';

// Real JSONL subprocess, synthetic native peer. No vendor or model requests.
const directory=await mkdtemp(join(tmpdir(),'hive-codex-interrupted-tools-'));
await writeFile(join(directory,'package.json'),'{"type":"commonjs"}');
await writeFile(join(directory,'app-server'),String.raw`
const {createInterface}=require('node:readline');const fs=require('node:fs');
let thread=fs.existsSync('thread.json')?JSON.parse(fs.readFileSync('thread.json','utf8')):{id:'interrupted-tool-thread',path:null,historyMode:'legacy',turns:[]};
const save=()=>fs.writeFileSync('thread.json',JSON.stringify(thread));const send=v=>process.stdout.write(JSON.stringify(v)+'\n');const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line);if(!q.method)continue;const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start'||q.method==='thread/resume')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  const turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});
  const declined=q.params.input.some(i=>i.text==='decline');const failed=q.params.input.some(i=>i.text==='fail');
  const item={id:'command-'+turn.id,type:'commandExecution',status:declined?'declined':'inProgress',command:'fixture command',cwd:process.cwd(),aggregatedOutput:''};turn.items.push(item);notify('item/started',{turnId:turn.id,item});
  if(declined){notify('item/completed',{turnId:turn.id,item});turn.status='completed';save();notify('turn/completed',{turn});}
  else{for(const delta of ['FIRST_PARTIAL\n','SECOND_PARTIAL 🐝\n']){item.aggregatedOutput+=delta;notify('item/commandExecution/outputDelta',{turnId:turn.id,itemId:item.id,delta});}if(failed){turn.status='failed';turn.error={message:'Synthetic terminal failure'};}save();if(failed)notify('turn/completed',{turn});}
 }else if(q.method==='turn/interrupt'){const turn=thread.turns.at(-1);turn.status='interrupted';save();answer({});notify('turn/completed',{turn});}
}})().catch(e=>{console.error(e);process.exit(1)});
`);
const events:PiEvent[]=[];
let ready!:()=>void;
const outputReady=new Promise<void>(resolve=>{ready=resolve;});
const options={cwd:directory,sessionDir:directory,command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(event:PiEvent){events.push(event);if(event.type==='tool_update'&&JSON.stringify(event.result).includes('SECOND_PARTIAL'))ready();}};
let session:Awaited<ReturnType<typeof createCodexSession>>|undefined;
try {
 session=await createCodexSession(options);
 const run=session.prompt('start partial command',{deliveryId:'cancelled-tool-delivery',presentation:{version:1,origin:'user',displayText:'start partial command'}});void run.catch(()=>{});
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{await Promise.race([outputReady,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Partial output not received')),5000);})]);}finally{clearTimeout(timer);}
 const full=[{type:'text',text:'FIRST_PARTIAL\nSECOND_PARTIAL 🐝\n'}];
 const pending=session.snapshot().history.find((m:any)=>m.toolName==='commandExecution') as any;
 assert.equal(pending.toolStatus,'inProgress');assert.deepEqual(pending.content,full);
 await session.abort();await assert.rejects(run);
 assert.deepEqual(session.completedDeliveryIds(),[]);
 const stopped=session.snapshot().history.find((m:any)=>m.toolName==='commandExecution') as any;
 assert.equal(stopped.toolStatus,'interrupted');assert.deepEqual(stopped.content,full);
 const end=events.find(e=>e.type==='tool_end') as Extract<PiEvent,{type:'tool_end'}>;
 assert.equal((end.result as any).toolStatus,'interrupted');assert.deepEqual((end.result as any).content,full);
 const threadId=session.sessionId;await session.dispose();session=undefined;
 session=await createCodexSession({...options,threadId});
 assert.deepEqual(session.completedDeliveryIds(),[]);
 assert.deepEqual(session.snapshot().history.find((m:any)=>m.toolName==='commandExecution'),stopped);
 await session.prompt('decline');
 const refused=session.snapshot().history.find((m:any)=>m.toolStatus==='declined') as any;
 assert.ok(refused);assert.equal(refused.isError,false);
 await assert.rejects(session.prompt('fail'));
 const failedTool=session.snapshot().history.find((m:any)=>m.toolCallId==='command-turn-2') as any;
 assert.equal(failedTool.toolStatus,'interrupted');assert.deepEqual(failedTool.content,full);
 assert.deepEqual(session.completedDeliveryIds(),[]);
 const native=JSON.parse(await readFile(join(directory,'thread.json'),'utf8'));
 assert.equal(native.turns[0].items[0].status,'inProgress');assert.equal(native.turns[0].items[0].hiveInterrupted,undefined);
 console.log('CODEX_TOOL_STATUS_HISTORY '+JSON.stringify({pending,stopped,refused}));
 console.log('Codex interrupted tools passed: cumulative live output, interruption without item/completed, real display-journal restart, declined status, native payload preserved. Synthetic JSONL peer; no model requests.');
}finally{if(session){await session.abort();await session.dispose();}await rm(directory,{recursive:true,force:true});}
