/** Opt-in original Codex/Terra: Stop must reap owned commands, preserve queued work and leave another connection alive. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import type {PiSessionHandle} from '../src/agents/types.js';
if(!process.argv.includes('--live'))throw new Error('Pass --live for real original Codex gpt-6.1-terra');
assert.ok(process.env.HIVE_CODEX_COMMAND,'Select the original Codex CLI explicitly');
const directory=await mkdtemp(join(resolve('../../.artifacts'),'codex-stop-live-'));
const project=join(directory,'project');await mkdir(project);
await writeFile(join(project,'hold.py'),"from pathlib import Path\nimport os,time\nPath('probe.pid').write_text(str(os.getpid()))\nprint('CANCELLATION_PROBE_READY',flush=True)\ntime.sleep(300)\nPath('unexpected-finished').write_text('finished')\n");
const events:any[]=[];const receipt:Record<string,unknown>={pass:false,directory,model:'openai/gpt-6.1-terra',originalCodex:true};
const runtime=await createDesktopRuntime(join(directory,'state'),event=>events.push(event));
const handles=()=> (runtime.chats as unknown as {live:Map<string,{handle:PiSessionHandle}>}).live;
const alive=(pid:number)=>{try{process.kill(pid,0);return true;}catch(error){if((error as NodeJS.ErrnoException).code==='ESRCH')return false;throw error;}};
async function wait(predicate:()=>Promise<boolean>,label:string,timeout=120000){const end=Date.now()+timeout;while(!await predicate()){assert.ok(Date.now()<end,label);await delay(50);}}
try{
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Owned Codex stop',autostart_orchestrator:false})as{id:string};
 const chat=await runtime.chats.create(ws.id,'codex');await runtime.chats.setModel(chat.id,'openai','gpt-6.1-terra');
 const controlProject=join(directory,'control');await mkdir(controlProject);
 const cw=await runtime.hive.request('POST','/api/workspaces',{path:controlProject,name:'Unaffected Codex connection',autostart_orchestrator:false})as{id:string};
 const control=await runtime.chats.create(cw.id,'codex');await runtime.chats.setModel(control.id,'openai','gpt-6.1-terra');const controlHandle=handles().get(control.id)!.handle;
 await runtime.chats.prompt(chat.id,'Run only python3 hold.py using exec_command with yield_time_ms 1000. After that call yields a running session_id, say PROBE_BACKGROUND_READY, then wait using write_stdin. This is an owned cancellation probe. Do not create a team or modify files yourself. If interrupted, never run this command again.');
 let pid=0;
 await wait(async()=>{try{pid=Number(await readFile(join(project,'probe.pid'),'utf8'));return Number.isSafeInteger(pid)&&pid>1&&alive(pid)&&(await runtime.chats.open(chat.id)).history.some((message:any)=>message.role==='assistant'&&message.content?.some((part:any)=>part.type==='text'&&part.text?.includes('PROBE_BACKGROUND_READY')));}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return false;throw error;}},'Original Codex did not start the owned probe');
 receipt.pid=pid;const running=await runtime.chats.open(chat.id);assert.equal(running.busy,true);assert.ok(running.runId);receipt.threadId=running.codexThreadId;
 await runtime.chats.prompt(chat.id,'Do not use tools or repeat the probe. Reply exactly STOP_QUEUE_ONE_OK.');
 await runtime.chats.prompt(chat.id,'Do not use tools or repeat the probe. Reply exactly STOP_QUEUE_TWO_OK.');
 const queued=(await runtime.chats.open(chat.id)).pendingUserInputs.filter(input=>!input.processing);assert.equal(queued.length,2);receipt.queuedIds=queued.map(input=>input.deliveryId);
 await runtime.chats.abort(chat.id);
 await wait(async()=>!alive(pid),'Stop returned but the owned native command remained alive',5000);receipt.commandReapedBeforeContinue=true;
 const stopped=await runtime.chats.open(chat.id);assert.equal(stopped.busy,false);assert.equal(stopped.runId,null);assert.equal(stopped.codexThreadId,running.codexThreadId);assert.equal(stopped.model?.id,'gpt-6.1-terra');assert.deepEqual(stopped.pendingUserInputs.map(input=>input.deliveryId),receipt.queuedIds);
 assert.equal(runtime.agents.peekAgentToken(`${ws.id}:orchestrator`),undefined);
 assert.equal(handles().get(control.id)!.handle,controlHandle);await controlHandle.reasoning!();assert.ok(!events.some(event=>event.type==='error'&&event.sessionId===control.id));receipt.otherConnectionAlive=true;
 await runtime.chats.resume(chat.id);await wait(async()=>{const s=await runtime.chats.open(chat.id);return !s.busy&&s.pending===0;},'Preserved inputs did not finish');
 const done=await runtime.chats.open(chat.id);assert.equal(done.codexThreadId,running.codexThreadId);assert.equal(done.model?.id,'gpt-6.1-terra');
 for(const input of queued)assert.equal(done.history.filter((m:any)=>m.presentation?.deliveryId===input.deliveryId).length,1);
 const answers=done.history.flatMap((m:any)=>m.role==='assistant'&&Array.isArray(m.content)?m.content.filter((p:any)=>p.type==='text').map((p:any)=>p.text):[]).filter((text:string)=>text==='STOP_QUEUE_ONE_OK'||text==='STOP_QUEUE_TWO_OK');
 assert.deepEqual(answers,['STOP_QUEUE_ONE_OK','STOP_QUEUE_TWO_OK']);receipt.answers=answers;receipt.pass=true;
}catch(error){receipt.error=error instanceof Error?error.message:String(error);process.exitCode=1;}
finally{await runtime.close();await writeFile(join(directory,'receipt.json'),JSON.stringify(receipt,null,2));await writeFile(join(directory,'events.json'),JSON.stringify(events,null,2));console.log(JSON.stringify(receipt));}
