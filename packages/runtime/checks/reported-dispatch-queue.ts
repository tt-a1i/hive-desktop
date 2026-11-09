// Real Hive HTTP/SQLite, Pi SDK and team CLI. Queues deliberately paused: no model calls.
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {createTeamTool} from '../src/team-tool.js';
import type {Chats} from '../src/chats.js';
import type {DeliveryQueue} from '../src/delivery-queue.js';
const directory=await mkdtemp(join(tmpdir(),'hive-reported-dispatch-queue-'));
const queues=(chats:Chats)=>(chats as unknown as {live:Map<string,{queue:DeliveryQueue}>}).live;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try {
 runtime=await createDesktopRuntime(join(directory,'state'),()=>{});
 const first=runtime,ws=await first.hive.request('POST','/api/workspaces',{path:directory,name:'Reported queue',autostart_orchestrator:false}) as {id:string};
 const main=await first.chats.create(ws.id);await first.chats.resume(main.id);
 await queues(first.chats).get(main.id)!.queue.pause();
 const binding=(id:string)=>({workspaceId:ws.id,agentId:id,token:first.agents.peekAgentToken(id)!,hivePort:String(first.hive.port),cwd:directory});
 const command=async(tool:ReturnType<typeof createTeamTool>,args:string[])=>JSON.parse((await tool.execute('queue-check',{args},undefined,undefined,undefined as never)).content.map(x=>'text' in x?x.text:'').join('')||'null');
 const lead=createTeamTool(binding(`${ws.id}:orchestrator`));
 await command(lead,['spawn','coder','--name','queue-worker']);
 const team=await first.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as {id:string;name:string}[];
 const member=team.find(x=>x.name==='queue-worker')!;assert.ok(member);
 await first.hive.request('POST',`/api/workspaces/${ws.id}/agents/${member.id}/start`);
 const worker=first.chats.list(ws.id).find(x=>x.agentId===member.id)!;assert.ok(worker);
 const queue=queues(first.chats).get(worker.id)!.queue;await queue.pause();
 const reported=await command(lead,['send','queue-worker','settled original responsibility']);
 const open=await command(lead,['send','queue-worker','still open responsibility']);
 const workerTool=createTeamTool(binding(member.id));
 const inbox=await command(workerTool,['messages','--dispatch',reported.dispatch_id]);
 await workerTool.execute('valid-report',{args:['report','accepted outcome','--dispatch',reported.dispatch_id,'--seen',String(inbox.required_seen_seq)]},undefined,undefined,undefined as never);
 // A rejected report must never settle or discard the other task.
 await assert.rejects(workerTool.execute('invalid-report',{args:['report','invalid outcome','--dispatch',open.dispatch_id,'--seen','999999']},undefined,undefined,undefined as never));
 await queue.enqueue('later user input',true,{version:1,origin:'user',displayText:'later user input'});
 await queue.enqueue('report-shaped transport input',true,{version:1,origin:'report',displayText:'preserve report',dispatchId:reported.dispatch_id});
 // Same task ID in another member's queue is not that member's responsibility.
 const mainQueue=queues(first.chats).get(main.id)!.queue;
 await mainQueue.enqueue('foreign dispatch',true,{version:1,origin:'dispatch',displayText:'foreign dispatch',dispatchId:reported.dispatch_id});
 const original=queue.pending;assert.equal(original.length,4);
 const retained=original.filter(x=>x.presentation?.dispatchId!==reported.dispatch_id||x.presentation.origin!=='dispatch');
 const live=first.agents.getActiveRunByAgentId(ws.id,member.id)!;assert.ok(live);
 first.agents.stopAgentRun(live.runId);assert.equal(await first.agents.waitForRunExit(live.runId,5000),true);
 const afterStop=await first.chats.open(worker.id);assert.equal(afterStop.pending,3);
 assert.deepEqual(queues(first.chats).get(worker.id)!.queue.pending,retained);
 assert.equal((await first.hive.request('GET',`/api/ui/workspaces/${ws.id}/dispatches/${reported.dispatch_id}`) as {state:string}).state,'reported');
 assert.equal((await first.hive.request('GET',`/api/ui/workspaces/${ws.id}/dispatches/${open.dispatch_id}`) as {state:string}).state,'submitted');
 assert.ok(mainQueue.pending.some(x=>x.text==='foreign dispatch'));
 // Preserve an old-version spool with an already reported task for cold-load reconciliation.
 const legacy=original.find(x=>x.presentation?.origin==='dispatch'&&x.presentation.dispatchId===reported.dispatch_id)!;
 await queues(first.chats).get(worker.id)!.queue.enqueue(legacy.text,legacy.triggerTurn,legacy.presentation);
 assert.equal((await first.chats.open(worker.id)).pending,4);
 await first.close();runtime=undefined;
 runtime=await createDesktopRuntime(join(directory,'state'),()=>{});
 assert.equal((await runtime.chats.open(worker.id)).pending,3);
 assert.deepEqual(queues(runtime.chats).get(worker.id)!.queue.pending,retained);
 await runtime.chats.open(main.id);
 assert.ok(queues(runtime.chats).get(main.id)!.queue.pending.some(x=>x.text==='foreign dispatch'));
 console.log('Reported dispatch queue passed: real report/retirement/cold recovery; unreported task, user input, report delivery and foreign ownership retained. No model requests.');
}finally{await runtime?.close();await rm(directory,{recursive:true,force:true});}
