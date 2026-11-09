// Real desktop runtime, Hive HTTP/SQLite and native protocol processes; no model/network.
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const directory=await mkdtemp(join(tmpdir(),'hive-workflow-shutdown-'));
const data=join(directory,'state'), project=join(directory,'project');
await mkdir(project);await writeFile(join(project,'package.json'),'{"type":"commonjs"}');
await writeFile(join(project,'app-server'),String.raw`
const fs=require('node:fs'),crypto=require('node:crypto'),rl=require('node:readline');let thread;
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
rl.createInterface({input:process.stdin}).on('line',line=>{const q=JSON.parse(line);if(!q.method)return;const answer=result=>send({id:q.id,result});
if(q.method==='initialize')answer({});
else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Controlled metadata',hidden:false}],nextCursor:null});
else if(q.method==='thread/start'||q.method==='thread/resume'){thread={id:q.params.threadId||crypto.randomUUID(),turns:[],historyMode:'legacy'};answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});}
else if(q.method==='thread/read')answer({thread});
else if(q.method==='turn/start'){const turn={id:crypto.randomUUID(),status:'inProgress',items:[{id:crypto.randomUUID(),type:'userMessage',content:q.params.input}]};thread.turns.push(turn);fs.writeFileSync('peer-'+process.pid+'.json',JSON.stringify({pid:process.pid,input:q.params.input}));answer({turn});send({method:'turn/started',params:{threadId:thread.id,turn}});send({method:'item/completed',params:{threadId:thread.id,turnId:turn.id,item:turn.items[0]}});}
else if(q.method==='turn/interrupt'){const turn=thread.turns.at(-1);if(turn)turn.status='interrupted';answer({});if(turn)send({method:'turn/completed',params:{threadId:thread.id,turn}});}
});`);
const previous=process.env.HIVE_CODEX_COMMAND;process.env.HIVE_CODEX_COMMAND=process.execPath;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const wait=async<T>(fn:()=>Promise<T>,ready:(x:T)=>boolean)=>{const deadline=Date.now()+12000;for(;;){const x=await fn();if(ready(x))return x;assert.ok(Date.now()<deadline,'Workflow state timed out');await delay(20);}};
type Run={id:string;status:string;error:string|null};
type Team={id:string;name:string;pending_task_count:number};
try{
 runtime=await createDesktopRuntime(data,()=>{});const first=runtime;
 const ws=await first.hive.request('POST','/api/workspaces',{path:project,name:'Shutdown check',autostart_orchestrator:false}) as {id:string};
 await first.hive.request('PUT','/api/settings/workflow-feature',{enabled:true});
 await first.hive.request('PUT','/api/settings/workflow-cli-policy',{default:'codex',allowed:['codex']});
 await first.hive.request('POST','/api/settings/role-templates',{name:'ShutdownCoder',role_type:'coder',description:'Owned shutdown check',default_command:'hive:codex',default_args:['--provider','openai','--model','gpt-6.1-terra'],default_env:{}});
 const main=await first.chats.create(ws.id,'codex');await first.chats.setModel(main.id,'openai','gpt-6.1-terra');await first.chats.resume(main.id);
 const token=first.agents.peekAgentToken(`${ws.id}:orchestrator`)!;assert.ok(token);
 const start=async(name:string,body:string)=>{const result=await first.hive.request('POST','/api/team/workflow/run',{project_id:ws.id,from_agent_id:`${ws.id}:orchestrator`,token,source:`export const meta={name:${JSON.stringify(name)},description:'Owned check',maxDurationMs:60000};${body}`}) as {run_id:string};return {run:{id:result.run_id}};};
 const get=async(id:string)=>(await first.hive.request('GET',`/api/workflows/runs/${id}`) as {run:Run}).run;
 const finished=async(id:string)=>wait(()=>get(id),x=>x.status!=='running');
 const completed=await start('Complete','return 7;');assert.equal((await finished(completed.run.id)).status,'completed');
 const failed=await start('Fail',"throw new Error('Owned genuine workflow error');");assert.equal((await finished(failed.run.id)).status,'failed');
 const agent=(label:string)=>`agent('Owned pending responsibility',{agentType:'ShutdownCoder',label:'${label}',timeoutMs:55000})`;
 const stopped=await start('Explicit stop',`return await ${agent('Stop-only')};`);
 await wait(()=>first.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as Promise<Team[]>,xs=>xs.some(x=>x.name==='Stop-only'&&x.pending_task_count===1));
 await first.hive.request('POST',`/api/workflows/runs/${stopped.run.id}/stop`);
 assert.equal((await finished(stopped.run.id)).status,'stopped');
 await wait(()=>first.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as Promise<Team[]>,xs=>!xs.some(x=>x.name==='Stop-only'));
 const pending=await start('Shutdown two workers',`return Promise.all([${agent('Pending-a')},${agent('Pending-b')}]);`);
 await wait(()=>first.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as Promise<Team[]>,xs=>['Pending-a','Pending-b'].every(n=>xs.some(x=>x.name===n&&x.pending_task_count===1)));
 let nestedId:string|undefined, childId:string|undefined;
 if(!process.argv.includes('--agents-only')){
  await mkdir(join(project,'.hive','workflows'),{recursive:true});
  await writeFile(join(project,'.hive','workflows','child.ts'),`export const meta={name:'Nested child',description:'Owned nested shutdown'};return await ${agent('Nested-member')};`);
  nestedId=(await start('Nested parent',"return await workflow('child');")).run.id;
  await wait(()=>first.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as Promise<Team[]>,xs=>xs.some(x=>x.name==='Nested-member'&&x.pending_task_count===1));
  const runs=await first.hive.request('GET',`/api/workspaces/${ws.id}/workflows/runs`) as {runs:(Run&{parent_run_id:string|null})[]};
  childId=runs.runs.find(x=>x.parent_run_id===nestedId)!.id;assert.ok(childId);
 }
 let cpuId:string|undefined;
 if(!process.argv.includes('--agents-only')){
  const cpu=await start('CPU workflow',"await log('Owned CPU entered');while(true){}");cpuId=cpu.run.id;
  await wait(async()=>first.hive.request('GET',`/api/workflows/runs/${cpuId}/logs`) as Promise<{logs:{message:string}[]}>,x=>x.logs.some(l=>l.message==='Owned CPU entered'));
 }
 // Wait for native input evidence, rather than assuming dispatch delivery means process activity.
 await wait(()=>readdir(project),xs=>xs.filter(x=>x.startsWith('peer-')).length>=(process.argv.includes('--agents-only')?4:5));
 const memberChats=first.chats.list(ws.id).filter(c=>c.agentId&&c.id!==main.id);
 const shuttingDown=first.close();assert.equal(first.close(),shuttingDown);await shuttingDown;runtime=undefined;
 const peers=await Promise.all((await readdir(project)).filter(x=>x.startsWith('peer-')).map(async x=>JSON.parse(await readFile(join(project,x),'utf8')) as {pid:number}));
 for(const peer of peers){assert.throws(()=>process.kill(peer.pid,0),(e:unknown)=>(e as NodeJS.ErrnoException).code==='ESRCH');}
 runtime=await createDesktopRuntime(data,()=>{});const cold=runtime;
 const restored=async(id:string)=>(await cold.hive.request('GET',`/api/workflows/runs/${id}`) as {run:Run}).run;
 assert.equal((await restored(pending.run.id)).status,'interrupted');
 if(cpuId)assert.equal((await restored(cpuId)).status,'interrupted');
 if(nestedId)assert.equal((await restored(nestedId)).status,'interrupted');
 if(childId)assert.equal((await restored(childId)).status,'interrupted');
 assert.equal((await restored(completed.run.id)).status,'completed');assert.equal((await restored(failed.run.id)).status,'failed');assert.equal((await restored(stopped.run.id)).status,'stopped');
 const tasks=await cold.hive.request('GET',`/api/workflows/runs/${pending.run.id}/dispatches`) as {dispatches:{status:string}[]};
 assert.equal(tasks.dispatches.length,2);assert.ok(tasks.dispatches.every(d=>d.status==='cancelled'));
 assert.equal((await cold.hive.request('GET',`/api/ui/workspaces/${ws.id}/team`) as Team[]).filter(x=>x.name.startsWith('Pending-')).length,0);
 for(const chat of memberChats)assert.equal((await cold.chats.open(chat.id)).pending,0);
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,noInference:true,realHttpSQLite:true,ownedPeerPids:peers.map(p=>p.pid),runId:pending.run.id,cpuId,nestedId,childId,completed:completed.run.id,failed:failed.run.id,stopped:stopped.run.id,coldPendingZero:true},null,2));
 console.log('Workflow shutdown passed; evidence:',directory);
}catch(error){console.error('Workflow shutdown evidence:',directory);throw error;}
finally{await runtime?.close();if(previous===undefined)delete process.env.HIVE_CODEX_COMMAND;else process.env.HIVE_CODEX_COMMAND=previous;}
