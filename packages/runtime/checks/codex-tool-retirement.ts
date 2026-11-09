import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {Type} from 'typebox';
import {createCodexSession} from '../src/agents/codex.js';

// Real JSONL subprocess, synthetic native peer. The separate live workflow
// probe covers actual HTTP/SQLite, team CLI and original Codex/Terra reports.
const directory=await mkdtemp(join(tmpdir(),'hive-codex-tool-retirement-'));
const peer=String.raw`
const {createInterface}=require('node:readline');const fs=require('node:fs');
const thread={id:'retiring-worker',path:null,historyMode:'legacy',turns:[]};
let turn,item,mode;const trace=[];
const send=value=>process.stdout.write(JSON.stringify(value)+'\n');
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
const save=()=>fs.writeFileSync('trace.json',JSON.stringify(trace));
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line),answer=result=>send({id:q.id,result});
 if(!q.method){if(q.id===41){trace.push({response:q.result});save();if(mode==='disconnect')process.exit(0);if(mode!=='hold-ack'){item.status=q.result.success?'completed':'failed';item.success=q.result.success;item.contentItems=q.result.contentItems;notify('item/completed',{turnId:turn.id,item});}}continue;}
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start'||q.method==='thread/resume')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  mode=q.params.input.find(part=>part.type==='text').text;
  item={type:'dynamicToolCall',id:'report-call',tool:'team',arguments:{args:['report']},status:'inProgress',contentItems:[]};
  turn={id:'turn-1',status:'inProgress',items:[item]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});notify('item/started',{turnId:turn.id,item});
  send({id:41,method:'item/tool/call',params:{threadId:thread.id,turnId:turn.id,callId:item.id,tool:'team',arguments:{args:['report']}}});
 }else if(q.method==='turn/interrupt'){
  trace.push({interrupt:true});save();if(item.status==='inProgress'){item.status='failed';item.success=false;notify('item/completed',{turnId:turn.id,item});}
  turn.status='interrupted';answer({});notify('turn/completed',{turn});
 }
}})().catch(error=>{console.error(error);process.exit(1)});
`;
try {
 for(const mode of ['retire','explicit','hold-ack','disconnect'] as const){
  const cwd=join(directory,mode);await mkdir(cwd);await writeFile(join(cwd,'package.json'),'{"type":"commonjs"}');await writeFile(join(cwd,'app-server'),peer);
  let ready!:()=>void,release!:()=>void,toolSignal:AbortSignal|undefined,unexpectedExit=false;const started=new Promise<void>(resolve=>{ready=resolve;});
  const session=await createCodexSession({cwd,sessionDir:cwd,command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(){},onUnexpectedExit(){unexpectedExit=true;},customTools:[{
   name:'team',description:'Controlled host report response',parameters:Type.Object({args:Type.Array(Type.String())}),
   execute:async(_id,_args,signal)=>new Promise((resolve,reject)=>{
    toolSignal=signal;
    release=()=>resolve({content:[{type:'text',text:'REPORT_COMMITTED'}]});
    signal?.addEventListener('abort',()=>reject(new Error('Host tool aborted')),{once:true});ready();
   }),
  }]});
  try {
   const prompt=session.prompt(mode);void prompt.catch(()=>{});let timer:ReturnType<typeof setTimeout>|undefined;
   try{await Promise.race([started,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Host request not received')),5000);})]);}finally{clearTimeout(timer);}
   const before=Date.now();
   const stopping=mode==='explicit'?session.abort():session.abort({settleTools:true});
   assert.equal(toolSignal?.aborted,mode==='explicit');
   release();
   if(mode==='disconnect'){
    await assert.rejects(stopping,Error);await assert.rejects(prompt,Error);
    assert.equal(unexpectedExit,true);
    assert.notEqual((session.snapshot().history.find((message:any)=>message.toolCallId==='report-call') as any).toolStatus,'completed');
    continue;
   }
   await stopping;await prompt;
   const tool=session.snapshot().history.find((message:any)=>message.toolCallId==='report-call') as any;
   const trace=JSON.parse(await readFile(join(cwd,'trace.json'),'utf8'));
   if(mode==='retire'){
    assert.equal(tool.toolStatus,'completed');assert.equal(tool.isError,false);
    assert.deepEqual(tool.content,[{type:'text',text:'REPORT_COMMITTED'}]);
    assert.equal(trace[0].response.success,true);assert.equal(trace[1].interrupt,true);
   }else{
    assert.notEqual(tool.toolStatus,'completed');
    assert.ok(Date.now()-before<5000,'Stop must remain bounded when native acknowledgement is missing');
    if(mode==='explicit')assert.equal(trace[0].interrupt,true);
    else assert.equal(trace[0].response.success,true);
   }
  }finally{await session.abort();await session.dispose();}
 }
 console.log('Codex tool retirement passed: native success before retirement interrupt; explicit stop remains immediate; missing native acknowledgement stays bounded; disconnect rejects without fabricating success. Synthetic JSONL peer, no model requests.');
}finally{await rm(directory,{recursive:true,force:true});}
