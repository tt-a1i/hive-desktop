import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopSessionManager } from '../src/agents/pi-session-manager.js';
import { createPiSession } from '../src/agents/pi.js';
import { createExternalPiSession } from '../src/agents/external-pi.js';
import { PI_USER_INPUT, PI_USER_INPUT_RESULT } from '../src/agents/pi-user-input.js';

// Real SDK-owned JSONL trees and original Pi RPC. No inference or mocked manager.
const directory=await mkdtemp(join(tmpdir(),'hive-pi-history-'));
const project=join(directory,'project'),agent=join(directory,'agent');
await mkdir(project);await mkdir(agent);
process.env.PI_CODING_AGENT_DIR=agent;process.env.PI_OFFLINE='1';
await writeFile(join(agent,'settings.json'),JSON.stringify({defaultProjectTrust:'never'}));
for(const backend of ['pi','pi-rpc'] as const){
 const sessionDir=join(directory,backend);await mkdir(sessionDir);
 const manager=createDesktopSessionManager(project,sessionDir);
 const add=(id:string)=>{
  const input={version:1,origin:'user',deliveryId:id,displayText:`Original ${id}`,rawText:`Original ${id}`};
  manager.appendCustomEntry(PI_USER_INPUT,input);
  const user=manager.appendMessage({role:'user',content:`Expanded ${id}`,timestamp:Date.now()});
  manager.appendCustomEntry(PI_USER_INPUT_RESULT,{deliveryId:id,disposition:'started'});
  return user;
 };
 const first=add('first'),second=add('second');
 const open=()=>backend==='pi'?createPiSession({cwd:project,sessionDir,sessionFile:manager.getSessionFile()!,agentDir:agent,onEvent(){}}):createExternalPiSession({cwd:project,sessionDir,sessionFile:manager.getSessionFile()!,command:process.env.HIVE_PI_COMMAND??'pi',onEvent(){}});
 const inputs=(snapshot:any)=>snapshot.history.filter((row:any)=>row.presentation?.origin==='user');
 let session=await open();
 assert.deepEqual(inputs(session.snapshot()).map((row:any)=>row.presentation.deliveryId),['first','second']);
 await session.dispose();
 manager.appendCompaction('Earlier input summarized',second,1000);
 session=await open();
 const compacted=session.snapshot();
 assert.deepEqual(inputs(compacted).map((row:any)=>row.presentation.deliveryId),['first','second'],'Model compaction must not delete earlier original chat messages');
 for(const [id,entryId] of [['first',first],['second',second]]){
  const row=inputs(compacted).find((row:any)=>row.presentation.deliveryId===id);
  assert.equal(row.content,`Expanded ${id}`);assert.equal(row.source.entryId,entryId);assert.equal(row.rawBackendItem.id,entryId);
 }
 if(backend==='pi'){
  assert.equal(JSON.stringify(session.history()).includes('Expanded first'),false,'Model context still uses native compaction');
  assert.ok(JSON.stringify(session.history()).includes('Earlier input summarized'));
 }
 await session.dispose();
 manager.appendContextEdit(second,{content:'Context edit for second'});
 session=await open();
 assert.equal(inputs(session.snapshot()).find((row:any)=>row.presentation.deliveryId==='second').content,'Expanded second','Context-only edits must not rewrite original chat history');
 if(backend==='pi'){
  assert.ok(JSON.stringify(session.history()).includes('Context edit for second'));
  assert.equal(JSON.stringify(session.history()).includes('Expanded second'),false);
 }
 await session.dispose();
 manager.branch(first);
 add('third');
 session=await open();
 assert.deepEqual(inputs(session.snapshot()).map((row:any)=>row.presentation.deliveryId),['first','third'],'Only the active native branch belongs to this chat');
 await session.dispose();
 session=await open();
 assert.deepEqual(inputs(session.snapshot()).map((row:any)=>[row.presentation.deliveryId,row.source.entryId]),[['first',first],['third',manager.getBranch().findLast(entry=>entry.type==='message'&&entry.message.role==='user')!.id]]);
 await session.dispose();
}
console.log(`PI_HISTORY_OK: real SDK JSONL/original RPC; compaction preserves original chat and source IDs without restoring old model context; active branch isolation and cold reopen. ${directory}`);
