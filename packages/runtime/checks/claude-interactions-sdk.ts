/** Official SDK + synthetic CLI protocol peer. No original CLI or inference. */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { query } from '@anthropic-ai/claude-agent-sdk';
import { ClaudeInteractions } from '../src/agents/claude-interactions.js';
import type { PiEvent } from '../src/agents/types.js';
const dir=await mkdtemp(join(tmpdir(),'hive-claude-sdk-interactions-'));
const peer=join(dir,'peer.cjs');const wire=join(dir,'wire.jsonl');
const questions=[{question:'Choose one?',header:'One',multiSelect:false,options:[{label:'Alpha',description:'A'},{label:'Beta',description:'B'}]},{question:'Choose many?',header:'Many',multiSelect:true,options:[{label:'X',description:'x'},{label:'Y',description:'y'}]}];
const cases=[{name:'Bash',input:{command:'echo owned',nested:{a:1}}},{name:'Bash',input:{command:'decline'}},{name:'Bash',input:{command:'cancel'}},{name:'AskUserQuestion',input:{questions}},{name:'AskUserQuestion',input:{questions:[{question:'bad'}]}},{name:'Bash',input:{command:'next'}},{name:'AskUserQuestion',input:{questions}}];
await writeFile(peer,`#!${process.execPath}\nconst fs=require('node:fs');const readline=require('node:readline');const cases=${JSON.stringify(cases)};let i=0;const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');const next=()=>{if(i<cases.length){const c=cases[i];send({type:'control_request',request_id:'request-'+i,request:{subtype:'can_use_tool',tool_name:c.name,input:c.input,tool_use_id:'tool-'+i}})}};readline.createInterface({input:process.stdin}).on('line',l=>{const x=JSON.parse(l);fs.appendFileSync(${JSON.stringify(wire)},JSON.stringify(x)+'\\n');if(x.type==='control_request'&&x.request.subtype==='initialize'){send({type:'control_response',response:{subtype:'success',request_id:x.request_id,response:{commands:[],models:[],account:{}}}});setTimeout(next,20)}else if(x.type==='control_response'&&x.response.request_id==='request-'+i){i++;setTimeout(next,10)}});\n`,{mode:0o700});
let finish!:()=>void;const pending=new Promise<void>(r=>finish=r);async function* input(){await pending;}
const events:PiEvent[]=[];const run=new AbortController();const manager=new ClaudeInteractions('session',()=>({id:'turn',signal:run.signal}),event=>events.push(event));
const sdk=query({prompt:input(),options:{pathToClaudeCodeExecutable:peer,canUseTool:(name,data,options)=>manager.request(name,data,options)}});
const consume=(async()=>{for await(const _ of sdk){}})();void consume.catch(()=>{});
const wait=async<T>(f:()=>Promise<T>|T,p:(v:T)=>boolean)=>{const end=Date.now()+5000;for(;;){const v=await f();if(p(v))return v;if(Date.now()>end)throw Error('timeout');await new Promise(r=>setTimeout(r,10));}};
const invalid=(fn:()=>unknown)=>assert.throws(fn,(e:unknown)=>(e as {code?:string}).code==='INTERACTION_INVALID_RESPONSE');
try {
 await sdk.initializationResult();
 for(let n=0;n<cases.length;n++){
  if(n===4){await wait(async()=>{try{return (await readFile(wire,'utf8')).includes('request-4')}catch{return false}},Boolean);continue;}
  const cards=await wait(()=>manager.snapshot(),v=>v.length===1);const card=cards[0]!;
  assert.equal(card.nativeRequestId,'request-'+n);assert.equal(card.turnId,'turn');assert.equal(card.itemId,'tool-'+n);
  if(n===3){invalid(()=>manager.respond(card.id,{answers:{'Choose one?':[], 'Choose many?':['X']}}));invalid(()=>manager.respond(card.id,{answers:{'Choose one?':['Alpha'],'Choose many?':['X'],extra:['Y']}}));invalid(()=>manager.respond(card.id,{answers:{'Choose one?':['Alpha','Beta'],'Choose many?':['X']}}));invalid(()=>manager.respond(card.id,{answers:{'Choose one?':['Alpha'],'Choose many?':['X','X']}}));assert.equal(manager.snapshot()[0]?.id,card.id);manager.respond(card.id,{answers:{'Choose one?':['freeform'],'Choose many?':['X','Y','Other']}});}
  else manager.respond(card.id,{decision:n===1?'decline':n===2||n===6?'cancel':'accept'});
  assert.throws(()=>manager.respond(card.id,{decision:'accept'}),(e:unknown)=>(e as {code?:string}).code==='INTERACTION_STALE');
 }
 const lines=await wait(async()=>{try{return (await readFile(wire,'utf8')).trim().split('\n').map(x=>JSON.parse(x))}catch{return []}},v=>v.filter((x:any)=>x.type==='control_response').length===cases.length);
 const responses=lines.filter((x:any)=>x.type==='control_response').map((x:any)=>x.response.response);
 assert.deepEqual(responses[0],{behavior:'allow',updatedInput:cases[0]!.input,toolUseID:'tool-0'});
 assert.equal(responses[1].behavior,'deny');assert.equal(responses[1].interrupt,undefined);assert.equal(responses[2].interrupt,true);
 assert.deepEqual(responses[3].updatedInput,{questions,answers:{'Choose one?':'freeform','Choose many?':'X, Y, Other'}});
 assert.equal(responses[4].behavior,'deny');assert.equal(responses[4].interrupt,true);assert.equal(responses[5].behavior,'allow');assert.equal(responses[6].behavior,'deny');assert.equal(responses[6].interrupt,true);
 assert.equal(JSON.stringify(events).includes('freeform'),false);assert.equal(manager.snapshot().length,0);
 // Exercise native cancellation and lifecycle with real manager callbacks, separately from SDK wire cases.
 const signal=new AbortController();const options={signal:signal.signal,toolUseID:'life-tool',requestId:'life-request'};
 const aborting=manager.request('Bash',{command:'abort'},options);assert.equal(manager.snapshot().length,1);signal.abort();assert.equal((await aborting).behavior,'deny');assert.equal(manager.snapshot().length,0);
 const life=new AbortController();const opts={signal:life.signal,toolUseID:'life-tool',requestId:'duplicate'};
 const first=manager.request('Bash',{command:'one'},opts);const second=manager.request('Bash',{command:'two'},opts);assert.equal((await first).behavior,'deny');assert.equal((await second).behavior,'deny');assert.equal(manager.snapshot().length,0);
 const ended=manager.request('Bash',{command:'end'},{...opts,requestId:'end'});manager.clear('aborted','turn');assert.equal((await ended).behavior,'deny');assert.equal(manager.snapshot().length,0);
 const next=manager.request('Bash',{command:'next'},{...opts,requestId:'continued'});manager.respond(manager.snapshot()[0]!.id,{decision:'accept'});assert.deepEqual(await next,{behavior:'allow',updatedInput:{command:'next'}});
 await writeFile(join(dir,'receipt.json'),JSON.stringify({pass:true,synthetic:true,noInference:true,officialSdk:true,scope:'SDK canUseTool + manager; not Hive HTTP or original CLI',cases:cases.length},null,2));console.log('Evidence:',dir);
} catch(e){await writeFile(join(dir,'failure.json'),JSON.stringify({error:String(e),events},null,2));console.error('Evidence:',dir);throw e;}finally{sdk.close();finish();await consume.catch(()=>{});}
