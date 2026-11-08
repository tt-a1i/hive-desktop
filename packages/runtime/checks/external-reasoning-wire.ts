/** Production JSONL from source or an extracted bundle. Real Codex, synthetic Claude CLI. No inference. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
const dir=await mkdtemp(join(tmpdir(),'hive-external-effort-wire-'));
const runtime=resolve(process.env.HIVE_REASONING_RUNTIME??'.');
const node=process.env.HIVE_REASONING_NODE??process.execPath;
const peer=fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url));
const config=join(dir,'claude');await mkdir(config);const wire=join(dir,'peer.jsonl');
const data=join(dir,'desktop');
function start(){
 const child=spawn(node,[join(runtime,'dist/main.js')],{cwd:dir,env:{...process.env,HIVE_DESKTOP_DATA_DIR:data,CLAUDE_CONFIG_DIR:config,HIVE_CLAUDE_COMMAND:peer,HIVE_CLAUDE_REASONING_WIRE:wire},stdio:['pipe','pipe','pipe']});
 const closed=once(child,'close');let errors='';child.stderr.on('data',chunk=>{errors+=chunk;process.stderr.write(chunk);});let sequence=0;
 const requests=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void}>();const agent:any[]=[];
 const lines=createInterface({input:child.stdout});lines.on('line',line=>{const value=JSON.parse(line);if(value.event==='agent')agent.push(value.data);requests.get(value.id)?.resolve(value);});
 child.on('close',()=>{for(const request of requests.values())request.reject(new Error('Wire runtime closed: '+errors));requests.clear();});
 async function rpc(method:string,params={}){
  console.log('WIRE '+method);const id=++sequence;let timer:ReturnType<typeof setTimeout>;
  try{return await new Promise<any>((yes,no)=>{timer=setTimeout(()=>no(new Error('Timed out: '+method)),20000);requests.set(id,{resolve:yes,reject:no});child.stdin.write(JSON.stringify({id,method,params})+'\n');});}
  finally{clearTimeout(timer!);requests.delete(id);}
 }
 async function stop(){if(child.exitCode!==null||child.signalCode!==null){await closed;lines.close();return;}try{assert.equal((await rpc('shutdown')).result.ok,true);assert.equal((await closed)[0],0);}finally{if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await closed;}lines.close();}}
 return {rpc,stop,agent};
}
let session= start();const ids:{id:string;backend:string}[]=[];
try{
 assert.ok((await session.rpc('initialize')).result.hivePort);
 const ws=(await session.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:dir,name:'external effort wire',autostart_orchestrator:false}})).result;
 for(const backend of ['codex','claude']){const result=await session.rpc('chat.createRecord',{workspaceId:ws.id,backend});assert.equal(result.error,undefined);ids.push({id:result.result.id,backend});}
}catch(error){console.error('WIRE_FAILURE',error);throw error;}finally{await session.stop();}
const file=join(data,'chats/index.json');const index=JSON.parse(await readFile(file,'utf8'));
for(const row of index)row.model={provider:row.backend==='codex'?'openai':'claude',id:'gpt-6.1-terra',name:'Terra wire control'};
await writeFile(file,JSON.stringify(index));
for(let boot=0;boot<2;boot++){
 session=start();try{
  assert.ok((await session.rpc('initialize')).result.hivePort);
  for(const {id} of ids){
   const state=await session.rpc('chat.reasoning',{sessionId:id});assert.equal(state.error,undefined);assert.equal(state.result.model.id,'gpt-6.1-terra');assert.ok(state.result.levels.includes('high'));
   if(boot===0){const changed=await session.rpc('chat.setReasoning',{sessionId:id,level:'high'});assert.equal(changed.error,undefined);assert.equal(changed.result.level,'high');assert.ok((await session.rpc('chat.setReasoning',{sessionId:id,level:null})).error);}
   else assert.equal(state.result.level,'high');
  }
  assert.ok(!session.agent.some(event=>event.type==='assistant_delta'||event.type==='tool_start'));
 }catch(error){console.error('WIRE_FAILURE',error);throw error;}finally{await session.stop();}
}
const peerLines=(await readFile(wire,'utf8')).trim().split('\n').map(line=>JSON.parse(line));assert.ok(peerLines.every(line=>line.event!=='query'));
const receipt={pass:true,dir,runtime,node,originalCodex:true,claudeSyntheticPeer:true,restarts:2,model:'gpt-6.1-terra',vendorInference:false};
await mkdir(resolve('../../.artifacts/external-reasoning'),{recursive:true});await writeFile(resolve('../../.artifacts/external-reasoning/wire-receipt.json'),JSON.stringify(receipt,null,2));console.log('EXTERNAL_REASONING_WIRE_OK '+JSON.stringify(receipt));
