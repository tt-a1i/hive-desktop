/** Official SDK + synthetic exit notification; real Hive, SQLite, transcript and spool. No inference. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-claude-write-loss-'));
const output=resolve(process.env.HIVE_CLAUDE_WRITE_LOSS_OUTPUT??'../../.artifacts/claude-write-loss');await mkdir(output,{recursive:true});
const keys=['CLAUDE_CONFIG_DIR','HIVE_CLAUDE_REASONING_WIRE','HIVE_CLAUDE_REASONING_CONTROL']as const;
const before=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const results:unknown[]=[];
async function wait(check:()=>Promise<boolean>){const until=Date.now()+12000;while(!await check()){assert.ok(Date.now()<until,'Claude write-loss contract timed out');await delay(15);}}
try {
 for(const scope of ['global','other','current','early-exit']as const){
  const incomplete=scope==='current'||scope==='early-exit';
  const project=join(directory,scope),data=join(project,'state');await mkdir(project);
  process.env.CLAUDE_CONFIG_DIR=join(project,'claude');process.env.HIVE_CLAUDE_REASONING_WIRE=join(project,'wire.jsonl');process.env.HIVE_CLAUDE_REASONING_CONTROL=join(project,'control.json');
  const finishGate=join(project,'release-terminal');
  await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL,JSON.stringify(scope==='early-exit'?{exitOnQuery:true,finishGate}:{completeQueries:true,writesLost:scope,finishGate}));
  const options={executableEnvironment:{HIVE_CLAUDE_COMMAND:fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url))}};
  const events:any[]=[];runtime=await createDesktopRuntime(data,event=>events.push(event),options);
  const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Write-loss '+scope,autostart_orchestrator:false})as{id:string};
  const chat=await runtime.chats.create(ws.id,'claude');
  assert.equal((await runtime.chats.prompt(chat.id,'First durable input')).accepted,true);
  assert.equal((await runtime.chats.prompt(chat.id,'Queued followup')).accepted,true);
  // These cases require an accepted followup before the terminal/exit. An
  // immediate process exit can correctly reject the second HTTP delivery.
  const queued=await runtime.chats.open(chat.id);
  assert.deepEqual(queued.pendingUserInputs.map(input=>input.text),['First durable input','Queued followup']);
  await writeFile(finishGate,'finish');
  const first=events.flatMap(e=>e.type==='state'?e.pendingUserInputs??[]:[]).find(i=>i.text==='First durable input');assert.ok(first?.deliveryId);
  await wait(async()=>events.some(e=>e.type==='error')&&!(await runtime!.chats.open(chat.id)).busy);
  const settled=await runtime.chats.open(chat.id);
  results.push({scope,completed:settled.completedDeliveryIds,pending:settled.pendingUserInputs.map(i=>i.text),first:first.deliveryId,events});
  await writeFile(join(output,'observed.json'),JSON.stringify({directory,results},null,2));
  assert.equal(settled.completedDeliveryIds.includes(first.deliveryId),!incomplete,`${scope}: loss scope and observed durable terminal must decide completion`);
  assert.deepEqual(settled.pendingUserInputs.map(i=>i.text),incomplete?['First durable input','Queued followup']:['Queued followup']);
  assert.equal(settled.deliveryFailure?.deliveryId,incomplete?first.deliveryId:undefined);
  await runtime.close();runtime=await createDesktopRuntime(data,event=>events.push(event),options);
  const cold=await runtime.chats.open(chat.id);assert.equal(cold.busy,false);
  assert.deepEqual(cold.pendingUserInputs.map(i=>i.deliveryId),settled.pendingUserInputs.map(i=>i.deliveryId));
  assert.equal(cold.completedDeliveryIds.includes(first.deliveryId),!incomplete);
  await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL,JSON.stringify({completeQueries:true}));
  await runtime.chats.resume(chat.id);await wait(async()=>{const s=await runtime!.chats.open(chat.id);return!s.busy&&s.pending===0;});
  const done=await runtime.chats.open(chat.id);assert.equal(done.completedDeliveryIds.filter(id=>id===first.deliveryId).length,1);
  const wire=(await readFile(process.env.HIVE_CLAUDE_REASONING_WIRE,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
  const queries=wire.filter(row=>row.event==='query');
  assert.equal(queries.filter(row=>typeof row.content==='string'&&row.content.startsWith('First durable input')).length,incomplete?2:1);
  assert.equal(queries.filter(row=>typeof row.content==='string'&&row.content.startsWith('Queued followup')).length,1);
  await runtime.close();runtime=undefined;
 }
 await writeFile(join(output,'receipt.json'),JSON.stringify({directory,scopes:['global','other','current','early-exit'],officialSdk:true,syntheticCli:true,vendorInference:false,results},null,2));
 console.log('CLAUDE_WRITE_LOSS_OK: loss scope, post-terminal exit, durable completion, paused queue, cold recovery and explicit resume. No inference.');
} finally {
 await runtime?.close();
 for(const key of keys){if(before[key]===undefined)delete process.env[key];else process.env[key]=before[key];}
}
