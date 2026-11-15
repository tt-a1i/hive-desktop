/** Official SDK, synthetic local peer, real Hive persistence; no model inference. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {ChatIndex} from '../src/chat-index.js';
import {DeliveryQueue} from '../src/delivery-queue.js';
import {createClaudeSession} from '../src/agents/claude.js';
const dir=await mkdtemp(join(tmpdir(),'hive-session-modes-'));
const keys=['HIVE_CLAUDE_COMMAND','CLAUDE_CONFIG_DIR','HIVE_CLAUDE_MODES_WIRE','HIVE_CLAUDE_MODES_CONTROL'] as const;
const previous=Object.fromEntries(keys.map(k=>[k,process.env[k]]));
process.env.HIVE_CLAUDE_COMMAND=fileURLToPath(new URL('./fixtures/claude-modes-peer.cjs',import.meta.url));
process.env.CLAUDE_CONFIG_DIR=join(dir,'claude');process.env.HIVE_CLAUDE_MODES_WIRE=join(dir,'wire');process.env.HIVE_CLAUDE_MODES_CONTROL=join(dir,'control');
const control=(value:object)=>writeFile(process.env.HIVE_CLAUDE_MODES_CONTROL!,JSON.stringify(value));
const lines=async()=>(await readFile(process.env.HIVE_CLAUDE_MODES_WIRE!,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
await control({});const {createDesktopRuntime}=await import('../src/desktop-runtime.js');let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try{
 const project=join(dir,'project'),state=join(dir,'desktop');await mkdir(project);
 const direct=await createClaudeSession({cwd:project,sessionDir:join(dir,'direct'),mode:'acceptEdits',onEvent(){}});const sid=direct.sessionId;
 assert.equal((await direct.mode!()).applied,'acceptEdits');assert.equal((await direct.setMode!('plan')).applied,'plan');
 await control({rejectMode:'default'});await assert.rejects(direct.setMode!('default'),/Synthetic rejected mode/);assert.equal((await direct.mode!()).selected,'plan');await direct.dispose();await control({});
 const resumed=await createClaudeSession({cwd:project,sessionDir:join(dir,'direct'),sessionId:sid,mode:'plan',onEvent(){}});assert.equal((await resumed.mode!()).applied,'plan');await resumed.dispose();
 const peer=await lines(),starts=peer.filter(x=>x.event==='start');assert.equal(starts.length,2);assert.ok(starts[0].args.some((x:string)=>x.startsWith('--session-id=')));assert.ok(starts[1].args.some((x:string)=>x.startsWith('--resume=')));
 const arg=(args:string[],name:string)=>args.find(x=>x.startsWith(name+'='))?.slice(name.length+1);assert.equal(arg(starts[0].args,'--permission-mode'),'acceptEdits');assert.equal(arg(starts[1].args,'--permission-mode'),'plan');
 assert.deepEqual(peer.filter(x=>x.event==='set_permission_mode').map(x=>x.request.mode),['acceptEdits','plan','default','plan']);
 runtime=await createDesktopRuntime(state,()=>{});const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'modes',autostart_orchestrator:false}) as {id:string};const chat=await runtime.chats.createRecord(ws.id,'claude');
 assert.equal((await runtime.chats.mode(chat.id)).selected,'default');assert.equal((await runtime.chats.mode(chat.id,'acceptEdits')).applied,'acceptEdits');
 assert.deepEqual(new ChatIndex(join(state,'chats')).get(chat.id).nativeMode,{backend:'claude',mode:'acceptEdits'});await assert.rejects(runtime.chats.mode(chat.id,'bypassPermissions'),TypeError);
 // Inject only guard state into the real opened chat; never send a user prompt.
 const live=(runtime.chats as unknown as {live:Map<string,any>}).live;const opened=live.get(chat.id)!;
 const indexFile=join(state,'chats/index.json');
 const guard=async()=>{const before=await readFile(indexFile,'utf8'),count=(await lines()).filter(x=>x.event==='set_permission_mode').length;
  await assert.rejects(runtime!.chats.mode(chat.id,'plan'),/queued messages and interactions/);
  assert.equal(await readFile(indexFile,'utf8'),before);assert.equal((await lines()).filter(x=>x.event==='set_permission_mode').length,count);};
 const originalQueue=opened.queue;
 for(const property of ['busy','pendingCount']){opened.queue=new Proxy(originalQueue,{get(target,key){return key===property?1:Reflect.get(target,key,target);}});try{await guard();}finally{opened.queue=originalQueue;}}
 const originalInteractions=opened.handle.interactions;opened.handle.interactions=()=>[{id:'synthetic'}];try{await guard();}finally{opened.handle.interactions=originalInteractions;}
 // A report arriving during native acknowledgement must wait for the new mode.
 let entered!:()=>void,release!:()=>void;const setterEntered=new Promise<void>(resolve=>{entered=resolve;});const gate=new Promise<void>(resolve=>{release=resolve;});
 let executions=0;const completed:string[]=[];const queue=new DeliveryQueue({directory:join(dir,'race-queue'),sessionId:'race',completedIds:()=>completed,
  execute:async item=>{executions++;completed.push(item.id);},onError:error=>{throw error;}});
 const originalSetter=opened.handle.setMode;opened.queue=queue;opened.handle.setMode=async(mode:string)=>{entered();await gate;return originalSetter.call(opened.handle,mode);};
 try{const change=runtime.chats.mode(chat.id,'plan');await setterEntered;assert.equal(queue.isPaused,true);
  await queue.enqueue('controlled report',true,{version:1,origin:'report',displayText:'controlled report'});
  assert.equal(executions,0);release();assert.equal((await change).applied,'plan');await queue.idle();assert.equal(executions,1);
 }finally{release();opened.handle.setMode=originalSetter;opened.queue=originalQueue;}
 assert.equal((await runtime.chats.mode(chat.id,'acceptEdits')).applied,'acceptEdits');
 // Inject an unsupported handle into real records, avoiding Pi process or account access.
 for(const backend of ['pi','pi-rpc'] as const){const unsupported=await runtime.chats.createRecord(ws.id,backend);live.set(unsupported.id,{queue:originalQueue,handle:{}});
  try{const before=await readFile(indexFile,'utf8'),count=(await lines()).filter(x=>x.event==='set_permission_mode').length;
   const result=await runtime.chats.mode(unsupported.id);assert.equal(result.backend,backend);assert.equal(result.selected,null);assert.deepEqual(result.options,[]);
   await assert.rejects(runtime.chats.mode(unsupported.id,'plan'),/does not support/);assert.equal(await readFile(indexFile,'utf8'),before);assert.equal((await lines()).filter(x=>x.event==='set_permission_mode').length,count);
  }finally{live.delete(unsupported.id);}}

 await control({rejectMode:'plan'});await assert.rejects(runtime.chats.mode(chat.id,'plan'),/Synthetic rejected mode/);assert.equal((await runtime.chats.mode(chat.id)).selected,'acceptEdits');await control({});
 await runtime.close();runtime=await createDesktopRuntime(state,()=>{});assert.equal((await runtime.chats.mode(chat.id)).applied,'acceptEdits');
 const file=join(state,'chats/index.json'),before=await readFile(file,'utf8');await rename(file,file+'.retained');await mkdir(file);
 try{await assert.rejects(runtime.chats.mode(chat.id,'plan'));}finally{await rm(file,{recursive:true});await rename(file+'.retained',file);}
 assert.equal(await readFile(file,'utf8'),before);assert.equal((await runtime.chats.mode(chat.id)).applied,'acceptEdits','A failed write must retire the changed session');await runtime.close();runtime=undefined;
 for(const nativeMode of [null,[],{},true,{backend:'pi',mode:'plan'},{backend:'codex',mode:'plan'},{backend:'claude',mode:''},{backend:'claude',mode:1}]){const rows=JSON.parse(before);rows.find((row:{id:string})=>row.id===chat.id).nativeMode=nativeMode;await writeFile(file,JSON.stringify(rows));assert.throws(()=>new ChatIndex(join(state,'chats')));}
 const rows=JSON.parse(before);rows.find((row:{id:string})=>row.id===chat.id).nativeMode={backend:'claude',mode:'retired-mode'};await writeFile(file,JSON.stringify(rows));runtime=await createDesktopRuntime(state,()=>{});await assert.rejects(runtime.chats.mode(chat.id));await assert.rejects(runtime.chats.prompt(chat.id,'Must not reach peer'));await runtime.close();runtime=undefined;
 assert.equal((await lines()).filter(x=>x.event==='query').length,0);console.log('SESSION_MODES_OK: SDK startup/resume and acknowledgement, persistence, rejection, disk failure and invalid records; no inference.');
}finally{await runtime?.close();for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}}
