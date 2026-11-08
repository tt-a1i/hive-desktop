import assert from 'node:assert/strict';
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
import { mkdtemp, mkdir, writeFile, chmod, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { SessionManager } from '@earendil-works/pi-coding-agent';

// Instrument only the write boundary to chmod this test's actual JSONL just
// before the completion append. The original OS append then fails with EACCES;
// SDK, Hive HTTP/SQLite and delivery spool are real, not replaced by mocks.
const directory=await mkdtemp(join(tmpdir(),'hive-pi-persistence-'));
const project=join(directory,'project'),agent=join(directory,'agent');
await mkdir(agent);await mkdir(join(project,'.pi','extensions'),{recursive:true});
await writeFile(join(project,'.pi','extensions','local.ts'),"export default function(pi){pi.registerCommand('local-save',{description:'Local persistence probe',handler(){}})}");
await writeFile(join(agent,'settings.json'),JSON.stringify({defaultProvider:'local-save',defaultModel:'gpt-6.1-terra',retry:{enabled:false}}));
await writeFile(join(agent,'models.json'),JSON.stringify({providers:{'local-save':{api:'openai-responses',apiKey:'local-only',baseUrl:'http://127.0.0.1:9/v1',models:[{id:'gpt-6.1-terra',name:'Terra local save'}]}}}));
process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
let runtime=await createDesktopRuntime(join(directory,'state'),()=>{});
const append=fs.appendFileSync;let path='',armed=true;let diskError:unknown;
try{
 const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Persistence failure',autostart_orchestrator:false}) as {id:string};
 const chat=await runtime.chats.create(workspace.id,'pi');await runtime.chats.setModel(chat.id,'local-save','gpt-6.1-terra');
 const live=(runtime.chats as any).live.get(chat.id);path=live.handle.sessionFile;
 fs.appendFileSync=((...args:Parameters<typeof append>)=>{
  if(armed&&args[0]===path&&String(args[1]).includes('"customType":"hive-delivery-completed"')){
   armed=false;fs.chmodSync(path,0o400);
   try{return append(...args);}catch(error){diskError=error;throw error;}
  }
  return append(...args);
 }) as typeof append;
 syncBuiltinESMExports();
 await runtime.chats.prompt(chat.id,'/local-save');await live.queue.idle();
 assert.equal((diskError as NodeJS.ErrnoException)?.code,'EACCES','The real OS write must reject');
 const id=live.queue.pending[0].id;
 assert.equal(SessionManager.open(path).getEntries().some(entry=>entry.type==='custom'&&entry.customType==='hive-delivery-completed'),false);
 assert.equal(live.handle.completedDeliveryIds().includes(id),false,'An in-memory append that failed on disk is not a completion acknowledgement');
 await chmod(path,0o600);fs.appendFileSync=append;syncBuiltinESMExports();
 const failed=await runtime.chats.open(chat.id);assert.equal(failed.pending,1);assert.equal(failed.completedDeliveryIds.includes(id),false);assert.equal(failed.deliveryFailure?.deliveryId,id);assert.equal(failed.runId,null);
 await runtime.chats.resume(chat.id);
 const retried=(runtime.chats as any).live.get(chat.id);await retried.queue.idle();
 assert.notEqual(retried.handle,live.handle);
 const complete=await runtime.chats.open(chat.id);assert.equal(retried.queue.pending.some((item:any)=>item.id===id),false);assert.ok(complete.completedDeliveryIds.includes(id));
 const entries=SessionManager.open(path).getEntries();assert.equal(entries.filter(entry=>entry.type==='custom'&&entry.customType==='hive-delivery-completed').length,1);
 const allIds=new Set(entries.map(entry=>entry.id));assert.ok(entries.every(entry=>entry.parentId===null||allIds.has(entry.parentId)),'Retry must not persist a ghost parent from the failed in-memory append');
 await runtime.close();runtime=await createDesktopRuntime(join(directory,'state'),()=>{});
 const cold=await runtime.chats.open(chat.id);assert.equal(cold.pending,complete.pending);assert.ok(cold.completedDeliveryIds.includes(id));
 assert.ok((await readFile(path,'utf8')).includes(id));
 console.log(`PI_PERSISTENCE_OK: original OS EACCES at completion, no false acknowledgement, retained same delivery, safe retry and cold restart. Automatic recovery may remain queued after loopback port 9 refuses its model request; no vendor inference. ${directory}`);
}finally{fs.appendFileSync=append;syncBuiltinESMExports();if(path)await chmod(path,0o600);await runtime.close();}
