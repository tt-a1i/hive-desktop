/** Controlled ACP startup via production JSONL. No vendor CLI, account or model. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
const directory=await mkdtemp(join(tmpdir(),'hive-acp-shutdown-start-'));
await mkdir(join(directory,'pi'));
const dataDir=join(directory,'state');
function start() {
 const child=spawn(process.execPath,[process.env.HIVE_DESKTOP_TEST_RUNTIME ?? 'dist/main.js'],{cwd:process.cwd(),env:{...process.env,HIVE_DESKTOP_DATA_DIR:dataDir,PI_CODING_AGENT_DIR:join(directory,'pi'),PI_OFFLINE:'1'},stdio:['pipe','pipe','pipe']});
 const exited=once(child,'exit');let sequence=0,stderr='';
 child.stderr.on('data',chunk=>{stderr+=String(chunk);});
 const pending=new Map<number,{resolve(value:any):void;reject(error:Error):void}>();
 const lines=createInterface({input:child.stdout});
 lines.on('line',line=>{const frame=JSON.parse(line);const request=pending.get(frame.id);if(request){pending.delete(frame.id);request.resolve(frame);}});
 child.on('exit',()=>{for(const request of pending.values())request.reject(new Error('Runtime exited: '+stderr));pending.clear();});
 const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<any>((resolve,reject)=>{
  const id=++sequence,timer=setTimeout(()=>{pending.delete(id);reject(new Error('Member request timed out: '+method));},40000);
  pending.set(id,{resolve:value=>{clearTimeout(timer);resolve(value);},reject:error=>{clearTimeout(timer);reject(error);}});
  child.stdin.write(JSON.stringify({id,method,params})+'\n');
 });

 return {child,rpc,exited,close:async()=>{if(child.exitCode===null){await rpc('shutdown');await exited;}lines.close();assert.equal(child.exitCode,0);}};
}
let wire=start();
const api=async(method:string,params:Record<string,unknown>={})=>{const frame=await wire.rpc(method,params);assert.equal(frame.error,undefined,JSON.stringify(frame.error));return frame.result;};
const wait=async<T>(read:()=>Promise<T>,ok:(value:T)=>boolean)=>{const limit=Date.now()+15000;for(;;){const value=await read();if(ok(value))return value;assert.ok(Date.now()<limit,'Timed out waiting for production ACP workflow');await new Promise(resolve=>setTimeout(resolve,30));}};
const peer=fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs',import.meta.url)),nativeRoot=join(directory,'native');
try{
 await api('initialize');const workspace=await api('hive.request',{method:'POST',path:'/api/workspaces',body:{path:directory,name:'Shutdown while ACP waits for MCP',autostart_orchestrator:false}});
 const profile=(await api('agent.profiles.create',{driver:'acp',label:'No MCP startup',launch:{command:process.execPath,args:[peer,nativeRoot,'no-mcp']},expectedStoreVersion:(await api('agent.profiles.list')).storeVersion})).profile;
 const chat=await api('chat.createRecord',{workspaceId:workspace.id,backend:'acp',profile:profile.ref});const starting=wire.rpc('chat.resume',{sessionId:chat.id});
 const started=await wait(async()=>{try{return (await readFile(join(nativeRoot,'protocol.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));}catch{return []; }},(rows:any[])=>rows.some(row=>row.method==='session/new'));
 const pid=started.find((row:any)=>row.event==='start').pid;assert.ok(pid);
 await wait(()=>api('chat.list',{workspaceId:workspace.id}),(records:any[])=>records.some(record=>record.id===chat.id&&typeof record.acpSessionId==='string')); // Native ID is persisted immediately before awaiting bridge proof.
 const began=Date.now();const result=await wire.rpc('shutdown');assert.equal(result.error,undefined);await wire.exited;const elapsedMs=Date.now()-began;assert.ok(elapsedMs<4000,`Shutdown waited ${elapsedMs}ms for the 30-second MCP deadline`);assert.ok((await starting).error);
 assert.throws(()=>process.kill(pid,0),{code:'ESRCH'},'The owned controlled ACP child must exit before runtime shutdown completes');
 console.log(JSON.stringify({pass:true,directory,productionJsonl:true,shutdownDuringMcpStartup:true,elapsedMs,ownedChildExited:true,noVendorProcess:true,noInference:true}));
}finally{await wire.close();}
