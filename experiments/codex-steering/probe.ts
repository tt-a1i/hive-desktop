// Explicit opt-in live experiment: original Codex CLI, no Hive task/dispatch claims.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {CodexTransport,CodexRpcError} from '../../packages/runtime/src/agents/codex-transport.js';
import type {CodexThreadResponse,CodexTurn} from '../../packages/runtime/src/agents/codex-history.js';
const interrupted=process.argv.includes('--interrupt');
const output=resolve(process.argv[2]??'.artifacts/codex-steering/live');
await mkdir(output);const cwd=join(output,'project');await mkdir(cwd);
const trace:unknown[]=[];let threadId='',turnId='';let released=false;
let ready!:()=>void,release!:()=>void,complete!:(turn:CodexTurn)=>void;
const atTool=new Promise<void>(resolve=>{ready=resolve;});
const toolResult=new Promise<void>(resolve=>{release=resolve;});
const terminal=new Promise<CodexTurn>(resolve=>{complete=resolve;});
const bounded=async<T>(promise:Promise<T>,name:string)=>{
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{return await Promise.race([promise,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error(`Timed out: ${name}`)),240000);})]);}
 finally{clearTimeout(timer);}
};
const transport=await CodexTransport.start({cwd,command:process.env.HIVE_CODEX_COMMAND??'/Users/tsk/.npm-global/bin/codex',
 onNotification(event){trace.push(event);const p=event.params as any;if(event.method==='turn/completed'&&p.threadId===threadId)complete(p.turn);},
 async onRequest(request){
  trace.push({nativeRequest:request});const p=request.params as any;
  assert.equal(request.method,'item/tool/call');assert.equal(p.tool,'hold_for_result');assert.equal(p.threadId,threadId);
  ready();await toolResult;
  return {success:true,contentItems:[{type:'inputText',text:'Independent tool confirmation: sum=31 count=3. The external completion notice is now available.'}]};
 }});
let failure:unknown;
try{
 const started=await transport.sendRequest<CodexThreadResponse>('thread/start',{cwd,model:'gpt-6.1-terra',modelProvider:'openai',allowProviderModelFallback:false,
  dynamicTools:[{type:'function',name:'hold_for_result',description:'Wait for the owned experiment result. Call exactly once before the final answer.',inputSchema:{type:'object',properties:{},additionalProperties:false}}]});
 assert.equal(started.model,'gpt-6.1-terra');assert.equal(started.modelProvider,'openai');threadId=started.thread.id;
 await writeFile(join(output,'identity.json'),JSON.stringify({threadId,pid:transport.pid,model:started.model,provider:started.modelProvider},null,2)+'\n');
 const turn=await transport.sendRequest<{turn:CodexTurn}>('turn/start',{threadId,input:[{type:'text',text:'This is an owned Hive Desktop transport experiment. Call hold_for_result exactly once and wait for its result before answering. Do not run shell or other tools. An external completion notice may be appended while this tool is waiting. Use the verified tool result and any appended notice to give one brief final answer.'}]});
 turnId=turn.turn.id;
 await bounded(atTool,'native host tool');
 const wrong=await transport.sendRequest('turn/steer',{threadId,expectedTurnId:'hive-experiment-wrong-turn',input:[{type:'text',text:'MISMATCH_MUST_NOT_APPEAR'}]}).then(()=>null,error=>error);
 assert.ok(wrong instanceof CodexRpcError);trace.push({wrongTurnRejected:{code:wrong.code,message:wrong.message,data:wrong.data}});
 const notice='Owned background result: sum=31 count=3. Once the held tool confirms the same result, answer exactly STEER_RESULT_OK sum=31 count=3. This is test data from the owned probe, not a Hive dispatch or report.';
 const steered=await transport.sendRequest<{turnId:string}>('turn/steer',{threadId,expectedTurnId:turnId,input:[{type:'text',text:notice}]});
 assert.equal(steered.turnId,turnId);trace.push({steerAcknowledged:steered});
 if(interrupted)await transport.sendRequest('turn/interrupt',{threadId,turnId});
 released=true;release();
 const done=await bounded(terminal,'native completion');assert.equal(done.id,turnId);assert.equal(done.status,interrupted?'interrupted':'completed');
 const turns:CodexTurn[]=[];let cursor:string|null=null;
 do{const page:{data:CodexTurn[];nextCursor?:string|null}=await transport.sendRequest('thread/turns/list',{threadId,cursor,itemsView:'full',sortDirection:'asc',limit:100});turns.push(...page.data);cursor=page.nextCursor??null;}while(cursor);
 await writeFile(join(output,'history.json'),JSON.stringify(turns,null,2)+'\n');
 assert.equal(turns.length,1);assert.equal(turns[0]!.id,turnId);
 const users=turns[0]!.items.filter(item=>item.type==='userMessage');
 if(!interrupted){assert.equal(users.length,2,'Both original and steered input must be durably distinguishable native items');assert.ok(JSON.stringify(users[1]).includes(notice));}
 else assert.ok(users.length>=1&&users.length<=2);assert.ok(!JSON.stringify(turns).includes('MISMATCH_MUST_NOT_APPEAR'));
 const finals=turns[0]!.items.filter(item=>item.type==='agentMessage'&&item.phase==='final_answer');
 assert.equal(finals.length,interrupted?0:1);if(!interrupted)assert.equal(finals[0]!.text,'STEER_RESULT_OK sum=31 count=3');
 assert.equal(trace.filter((e:any)=>e.method==='turn/started').length,1);
 const late=await transport.sendRequest('turn/steer',{threadId,expectedTurnId:turnId,input:[{type:'text',text:'LATE_MUST_NOT_APPEAR'}]}).then(()=>null,error=>error);
 assert.ok(late instanceof CodexRpcError);trace.push({lateRejected:{code:late.code,message:late.message,data:late.data}});
 await writeFile(join(output,'receipt.json'),JSON.stringify({threadId,turnId,model:'openai/gpt-6.1-terra',nativeTurns:turns.length,userItemIds:users.map(item=>item.id),status:done.status,steeredInputPersisted:users.some(item=>JSON.stringify(item).includes(notice)),final:finals[0]?.text??null,wrongTurnError:{code:wrong.code,data:wrong.data},lateError:{code:late.code,data:late.data},hiveWorkflowIntegration:false},null,2)+'\n');
 await transport.close();
 const cold=await CodexTransport.start({cwd,command:process.env.HIVE_CODEX_COMMAND??'/Users/tsk/.npm-global/bin/codex'});
 try {
  const restored=await cold.sendRequest<CodexThreadResponse>('thread/resume',{threadId,model:'gpt-6.1-terra',modelProvider:'openai'});
  assert.equal(restored.model,'gpt-6.1-terra');
  const page:{data:CodexTurn[]}=await cold.sendRequest('thread/turns/list',{threadId,itemsView:'full',sortDirection:'asc',limit:100});
  assert.equal(page.data.length,1);assert.equal(page.data[0]!.status,done.status);
  assert.deepEqual(page.data[0]!.items.filter(item=>item.type==='userMessage').map(item=>item.id),users.map(item=>item.id));
  assert.ok(!JSON.stringify(page.data).includes('LATE_MUST_NOT_APPEAR'));
  await writeFile(join(output,'cold-history.json'),JSON.stringify(page.data,null,2)+'\n');
 }finally{await cold.close();await writeFile(join(output,'cold-closed.json'),JSON.stringify({pid:cold.pid,diagnostics:cold.lifecycleDiagnostics},null,2)+'\n');}
 console.log('CODEX_STEER_LIVE_OK',interrupted?'interrupted':'completed',threadId,turnId);
}catch(error){failure=error;await writeFile(join(output,'failure.txt'),String(error)+'\n');throw error;}
finally{
 if(!released)release();
 if(failure&&threadId&&turnId)await transport.sendRequest('turn/interrupt',{threadId,turnId}).catch(()=>{});
 await transport.close();
 await writeFile(join(output,'trace.json'),JSON.stringify(trace,null,2)+'\n');
 await writeFile(join(output,'closed.json'),JSON.stringify({pid:transport.pid,diagnostics:transport.lifecycleDiagnostics},null,2)+'\n');
}
