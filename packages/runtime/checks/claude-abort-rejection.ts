/** Official SDK + real owned child; synthetic rejection, no vendor inference. */
import assert from 'node:assert/strict';
import { mkdtemp,mkdir,readFile,writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { createClaudeSession } from '../src/agents/claude.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import { DatabaseSync } from 'node:sqlite';
const directory=await mkdtemp(join(tmpdir(),'hive-claude-abort-rejection-'));
const keys=['CLAUDE_CONFIG_DIR','HIVE_CLAUDE_REASONING_WIRE','HIVE_CLAUDE_REASONING_CONTROL'] as const;
const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
process.env.CLAUDE_CONFIG_DIR=join(directory,'claude');
process.env.HIVE_CLAUDE_REASONING_WIRE=join(directory,'wire.jsonl');
process.env.HIVE_CLAUDE_REASONING_CONTROL=join(directory,'control.json');
await mkdir(process.env.CLAUDE_CONFIG_DIR);
await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL,JSON.stringify({rejectInterrupt:true}));
async function rows(){return (await readFile(process.env.HIVE_CLAUDE_REASONING_WIRE!,'utf8')).trim().split('\n').map(line=>JSON.parse(line));}
async function wait(event:string){const deadline=Date.now()+3000;while(!(await rows()).some(row=>row.event===event)){assert.ok(Date.now()<deadline,'Missing owned child event: '+event);await delay(10);}}
let session:Awaited<ReturnType<typeof createClaudeSession>>|undefined;
let prompt:Promise<void>|undefined;let stopping:Promise<void>|undefined;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
async function until(check:()=>Promise<boolean>){const deadline=Date.now()+8000;while(!await check()){assert.ok(Date.now()<deadline,'Owned runtime recovery timed out');await delay(10);}}
try{
 const failures:Error[]=[];
 session=await createClaudeSession({cwd:directory,sessionDir:directory,command:fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url)),model:{provider:'claude',id:'gpt-6.1-terra'},onEvent(){},onUnexpectedExit(error){failures.push(error);}});
 const pid=session.processId!;assert.ok(pid);
 prompt=session.prompt('Owned interruption rejection',{deliveryId:'pending-delivery'});void prompt.catch(()=>{});await wait('query');
 stopping=session.abort();void stopping.catch(()=>{});
 await assert.rejects(stopping);await assert.rejects(prompt);
 assert.throws(()=>process.kill(pid,0));assert.equal(failures.length,1);assert.deepEqual(session.completedDeliveryIds(),[]);
 await session.dispose();session=undefined;
 const project=join(directory,'project'),state=join(directory,'state');await mkdir(project);
 const options={executableEnvironment:{HIVE_CLAUDE_COMMAND:fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url))}};
 const live=()=>(runtime!.chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live;
 runtime=await createDesktopRuntime(state,()=>{},options);
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Rejected Claude stop',autostart_orchestrator:false}) as {id:string};
 const chat=await runtime.chats.create(ws.id,'claude');
 await runtime.chats.prompt(chat.id,'Preserve first input');await until(async()=>(await rows()).filter(row=>row.event==='query').length===2);
 await runtime.chats.prompt(chat.id,'Preserve queued followup');
 const before=await runtime.chats.open(chat.id);assert.ok(before.runId);assert.equal(before.pending,2);
 const ids=live().get(chat.id)!.queue.pending.map(item=>item.id);
 const deliveredTexts=live().get(chat.id)!.queue.pending.map(item=>item.text);
 const runtimePid=(await rows()).findLast(row=>row.event==='start').pid;
 await assert.rejects(runtime.chats.abort(chat.id));assert.throws(()=>process.kill(runtimePid,0));
 assert.equal(await runtime.agents.waitForRunExit(before.runId,3000),true);
 assert.equal(runtime.agents.findLiveRun(before.runId)?.status,'error');assert.equal(runtime.agents.peekAgentToken(`${ws.id}:orchestrator`),undefined);
 const failed=await runtime.chats.open(chat.id);assert.equal(failed.busy,false);assert.equal(failed.runId,null);assert.equal(failed.pending,2);assert.deepEqual(failed.completedDeliveryIds,[]);
 assert.deepEqual(live().get(chat.id)!.queue.pending.map(item=>item.id),ids);
 await runtime.close();runtime=await createDesktopRuntime(state,()=>{},options);
 const cold=await runtime.chats.open(chat.id);assert.equal(cold.busy,false);assert.equal(cold.pending,2);assert.equal(cold.runId,null);
 assert.deepEqual(live().get(chat.id)!.queue.pending.map(item=>item.id),ids);assert.deepEqual(cold.completedDeliveryIds,[]);
 assert.deepEqual(cold.pendingUserInputs.map(item=>item.text),['Preserve first input','Preserve queued followup']);
 assert.equal((await rows()).filter(row=>row.event==='query').length,2,'Cold open must not replay pending work');
 await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL!,JSON.stringify({completeQueries:true}));
 await runtime.chats.resume(chat.id);await until(async()=>{const snapshot=await runtime!.chats.open(chat.id);return !snapshot.busy&&snapshot.pending===0;});
 const done=await runtime.chats.open(chat.id);
 for(const id of ids)assert.equal(done.completedDeliveryIds.filter(item=>item===id).length,1);
 const queries=(await rows()).filter(row=>row.event==='query');
 assert.equal(queries.filter(row=>row.content===deliveredTexts[0]).length,2);assert.equal(queries.filter(row=>row.content===deliveredTexts[1]).length,1);
 await runtime.close();runtime=undefined;
 const db=new DatabaseSync(join(state,'hive/runtime.sqlite'),{readOnly:true});
 try{assert.deepEqual(db.prepare("select text from messages where type='user_input' order by sequence").all().map(row=>row.text),['Preserve first input','Preserve queued followup']);}finally{db.close();}
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,officialSdk:true,syntheticCli:true,noInference:true,ownedPid:pid,runtimePid,failedPromptSettled:true,childExitedBeforeAbortReturned:true,unexpectedExitCount:failures.length,pendingIdsRetained:ids,pausedColdPending:2,resumedPending:done.pending,completionIds:done.completedDeliveryIds},null,2));
 console.log(JSON.stringify({pass:true,directory,noInference:true}));
}finally{
 await stopping?.catch(()=>{});await prompt?.catch(()=>{});await session?.dispose();await runtime?.close();
 for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}
}
