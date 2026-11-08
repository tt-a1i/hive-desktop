/** Opt-in original Codex Terra inference: queued explicit input survives Stop and a cold runtime restart. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,unlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';import {join,resolve} from 'node:path';
import {randomUUID} from 'node:crypto';import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {ChatIndex} from '../src/chat-index.js';
if(!process.argv.includes('--live'))throw new Error('Pass --live to execute original Codex gpt-6.1-terra');
const command=process.env.HIVE_CODEX_COMMAND;assert.ok(command,'Explicit original Codex CLI required');
const directory=await mkdtemp(join(tmpdir(),'hive-queued-native-codex-')),data=join(directory,'state'),project=join(directory,'project');await mkdir(project);
const events:any[]=[];const receipt:Record<string,unknown>={pass:false,directory,model:'openai/gpt-6.1-terra',originalCodex:true,vendorInference:true};
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined,id:string|undefined;
const boot=()=>createDesktopRuntime(data,e=>events.push(e),{executableEnvironment:{HIVE_CODEX_COMMAND:command}});
const wait=async(predicate:()=>Promise<boolean>,label:string)=>{const end=Date.now()+120000;while(!await predicate()){
 const state=await runtime!.chats.open(id!);assert.ok(!state.interactions?.some(i=>i.state==='pending'),'Native interaction requires user response; no automatic permission granted');
 if(!state.busy&&events.some(e=>e.type==='error'))throw new Error('Native queue run failed: '+events.filter(e=>e.type==='error').map(e=>e.message).join('; '));
 assert.ok(Date.now()<end,label);await delay(100);
}};
try{
 runtime=await boot();const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Native queued input',autostart_orchestrator:false})as{id:string};const record=await runtime.chats.createRecord(ws.id,'codex');id=record.id;await runtime.close();runtime=undefined;
 const index=new ChatIndex(join(data,'chats'));index.save({...index.get(id),model:{provider:'openai',id:'gpt-6.1-terra',name:'Terra native queue proof'}});
 runtime=await boot();const opened=await runtime.chats.open(id);assert.equal(opened.model?.id,'gpt-6.1-terra');
 await runtime.chats.prompt(id,'For this cancellation check, run the read-only shell command sleep 30. Do not create files or a team. After it finishes reply FIRST_TURN_FINISHED.');
 await wait(async()=>events.some(e=>e.type==='tool_start'&&JSON.stringify(e.args).includes('sleep 30')),'Native sleep tool did not start');
 const source=join(directory,'queued-details.txt'),marker='queued-'+randomUUID();await writeFile(source,'marker='+marker+'\n');const uploaded=await runtime.chats.attachments.add(ws.id,source);await unlink(source);
 const text='Read the attached saved file with a read-only local tool and reply with its exact marker value. Do not write files, create a team, or repeat sleep. The source file has been deleted; use the attachment copy.';
 await runtime.chats.prompt(id,text,[uploaded.attachment.id]);const queued=await runtime.chats.open(id);const item=queued.pendingUserInputs.find(i=>i.text===text);assert.ok(item);assert.equal(item.processing,false);receipt.queuedDeliveryId=item.deliveryId;
 await runtime.chats.abort(id);const stopped=await runtime.chats.open(id);assert.equal(stopped.busy,false);assert.equal(stopped.pendingUserInputs.length,1);assert.equal(stopped.pendingUserInputs[0]!.deliveryId,item.deliveryId);
 await runtime.close();runtime=await boot();const cold=await runtime.chats.open(id);assert.equal(cold.busy,false);assert.equal(cold.pendingUserInputs[0]!.deliveryId,item.deliveryId);assert.equal(cold.pendingUserInputs[0]!.attachments[0]!.id,uploaded.attachment.id);
 await runtime.chats.resume(id);await wait(async()=>{const s=await runtime!.chats.open(id!);return !s.busy&&s.pending===0;},'Native resumed queue did not complete');
 const done=await runtime.chats.open(id);assert.deepEqual(done.pendingUserInputs,[]);assert.equal(done.history.filter((m:any)=>m.presentation?.deliveryId===item.deliveryId).length,1);
 assert.ok(done.history.some((m:any)=>m.role==='assistant'&&m.content?.some((c:any)=>c.type==='text'&&c.text.includes(marker))),'Model must return marker known only from uploaded bytes');
 assert.ok(done.history.some((m:any)=>m.role==='toolResult'&&JSON.stringify(m.toolInput??m.rawBackendItem).includes(uploaded.filePath!)),'Native tool must read saved attachment copy');
 receipt.pass=true;receipt.chatId=id;receipt.threadId=done.codexThreadId;receipt.markerReturned=true;receipt.queuedHistoryEntries=1;receipt.coldPaused=true;receipt.toolMessages=done.history.filter((m:any)=>m.role==='toolResult').length;
}catch(error){receipt.error=error instanceof Error?error.message:String(error);process.exitCode=1;}
finally{if(runtime){if(id)await runtime.chats.abort(id).catch(e=>{receipt.abortError=String(e);});await runtime.close();}const out=resolve('../../.artifacts/queued-inputs-native-codex');await mkdir(out,{recursive:true});await writeFile(join(out,'receipt.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));}
