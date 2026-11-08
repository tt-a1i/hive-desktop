import assert from 'node:assert/strict';
import { mkdtemp,mkdir,writeFile,readFile,chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SessionManager } from '@earendil-works/pi-coding-agent';
import { createPiSession } from '../src/agents/pi.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-pi-context-persistence-'));
const project=join(directory,'project'),agent=join(directory,'agent');await mkdir(project);await mkdir(agent);
await writeFile(join(agent,'settings.json'),JSON.stringify({defaultProvider:'context-save',defaultModel:'gpt-6.1-terra'}));
await writeFile(join(agent,'models.json'),JSON.stringify({providers:{'context-save':{api:'openai-responses',apiKey:'local-only',baseUrl:'http://127.0.0.1:9/v1',models:[{id:'gpt-6.1-terra',name:'Terra context save',reasoning:true}]}}}));
process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
for(const operation of ['context','model','reload','reasoning','dispatch'] as const){
 console.log('[context-persistence]',operation);
 const sessionDir=join(directory,operation);await mkdir(sessionDir);const errors:Error[]=[];
 let handle=await createPiSession({cwd:project,sessionDir,agentDir:agent,onEvent(){},onUnexpectedExit:error=>errors.push(error)});
 const path=handle.sessionFile!;
 try{
  const initial=await readFile(path,'utf8');await chmod(path,0o400);
  const mutate=async()=>{
   if(operation==='context')return handle.addContext('Rejected context','hive-context-failure');
   if(operation==='model')return handle.setModel('context-save','gpt-6.1-terra');
   if(operation==='reload')return handle.refreshModels!();
   if(operation==='dispatch')return handle.prompt('Automated context write',{deliveryId:'dispatch-failure',presentation:{version:1,origin:'dispatch',displayText:'Automated context write'}});
   const state=await handle.reasoning!();const next=state.levels.find(level=>level!==state.level);assert.ok(next);return handle.setReasoning!(next);
  };
  await assert.rejects(mutate,error=>{const e=error as NodeJS.ErrnoException&{cause?:NodeJS.ErrnoException};return e.code==='EACCES'||e.cause?.code==='EACCES';});
  await chmod(path,0o600);
  await assert.rejects(handle.addContext('Must not reuse dirty native state','hive-context-unsafe'),'Every native writer failure must prevent further writes on that handle');
  assert.equal(errors.length,1,'The owner must receive exactly one fatal write failure');
  assert.equal(await readFile(path,'utf8'),initial);
  await handle.dispose();
  handle=await createPiSession({cwd:project,sessionDir,agentDir:agent,sessionFile:path,onEvent(){}});
  await handle.addContext('Recovered context','hive-context-retry');
  const entries=SessionManager.open(path).getEntries();const ids=new Set(entries.map(entry=>entry.id));
  assert.ok(entries.every(entry=>entry.parentId===null||ids.has(entry.parentId)));
  assert.equal(entries.some(entry=>entry.type==='custom_message'&&entry.customType==='hive-context-failure'),false);
  assert.equal(entries.filter(entry=>entry.type==='custom_message'&&entry.customType==='hive-context-retry').length,1);
 }finally{await chmod(path,0o600);await handle.dispose();}
}
// Reading an invalid provider config is recoverable and must not poison a healthy writer.
const configFile=join(agent,'models.json'),validConfig=await readFile(configFile,'utf8');
const readErrors:Error[]=[];const healthy=await createPiSession({cwd:project,sessionDir:join(directory,'config-read'),agentDir:agent,onEvent(){},onUnexpectedExit:error=>readErrors.push(error)});
try{
 await writeFile(configFile,'{broken');await assert.rejects(healthy.refreshModels!());assert.equal(readErrors.length,0);
 await writeFile(configFile,validConfig);await healthy.refreshModels!();await healthy.addContext('Config reader recovered');assert.equal(readErrors.length,0);
}finally{await writeFile(configFile,validConfig);await healthy.dispose();}
// The same context-only route through real Hive HTTP/SQLite and the spool.
let runtime=await createDesktopRuntime(join(directory,'state'),()=>{});let path='';
try{
 const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Context-only recovery',autostart_orchestrator:false}) as {id:string};
 const chat=await runtime.chats.create(workspace.id);const original=(runtime.chats as any).live.get(chat.id);path=original.handle.sessionFile;
 await chmod(path,0o400);await original.queue.enqueue('Queued context only',false);const id=original.queue.pending[0].id;original.queue.resume();await original.queue.idle();
 await chmod(path,0o600);const failed=await runtime.chats.open(chat.id);assert.equal(failed.pending,1);assert.equal(failed.deliveryFailure?.deliveryId,id);
 const restored=(runtime.chats as any).live.get(chat.id);assert.notEqual(restored.handle,original.handle);restored.queue.resume();await restored.queue.idle();
 assert.equal((await runtime.chats.open(chat.id)).pending,0);
 await runtime.close();runtime=await createDesktopRuntime(join(directory,'state'),()=>{});
 const cold=await runtime.chats.open(chat.id);assert.equal(cold.pending,0);assert.equal(cold.history.filter((row:any)=>row.customType==='hive-context'&&row.content==='Queued context only').length,1);
 const entries=SessionManager.open(path).getEntries(),ids=new Set(entries.map(entry=>entry.id));assert.ok(entries.every(entry=>entry.parentId===null||ids.has(entry.parentId)));
 console.log(`PI_CONTEXT_PERSISTENCE_OK: real read-only JSONL for context/model/reload/reasoning/dispatch; fatal owner signal, no dirty reuse, safe reopen; real Hive/SQLite context-only spool retry and cold recovery; no model requests. ${directory}`);
}finally{if(path)await chmod(path,0o600);await runtime.close();}
