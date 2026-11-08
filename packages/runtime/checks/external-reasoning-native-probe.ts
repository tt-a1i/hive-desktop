/** Native controls only. Codex explicitly selects Terra; Claude retains its current model. No prompt/query is sent. */
import assert from 'node:assert/strict';
import { mkdtemp,mkdir,writeFile }from'node:fs/promises';import{tmpdir}from'node:os';import{join,resolve}from'node:path';
import { CodexTransport }from'../src/agents/codex-transport.js';
import{query,type Query}from'@anthropic-ai/claude-agent-sdk';
const directory=await mkdtemp(join(tmpdir(),'hive-external-reasoning-probe-'));const notifications:string[]=[];const receipt:Record<string,unknown>={directory,inference:false};
const codex=await CodexTransport.start({cwd:directory,command:process.env.HIVE_CODEX_COMMAND??'codex',onNotification:event=>notifications.push(event.method)});
try{
 const catalog=await codex.sendRequest<any>('model/list',{includeHidden:true});const model=catalog.data.find((m:any)=>m.model==='gpt-6.1-terra');assert.ok(model);const levels=model.supportedReasoningEfforts.map((entry:any)=>entry.reasoningEffort);assert.ok(levels.includes('high'));
 const started=await codex.sendRequest<any>('thread/start',{cwd:directory,model:'gpt-6.1-terra',modelProvider:'openai',historyMode:'legacy',allowProviderModelFallback:false});assert.equal(started.model,'gpt-6.1-terra');
 await codex.sendRequest('thread/settings/update',{threadId:started.thread.id,effort:'high'});
 const read=await codex.sendRequest<any>('thread/read',{threadId:started.thread.id,includeTurns:false});assert.equal(read.thread.reasoningEffort,'high');assert.equal(read.thread.turns.length,0);assert.ok(!notifications.includes('turn/started'));
 receipt.codex={model:started.model,levels,initial:started.reasoningEffort,readBack:read.thread.reasoningEffort,turns:0};
}finally{await codex.close();}
let finish!:()=>void;const waiting=new Promise<void>(resolve=>finish=resolve);async function* input(){await waiting;}
const controller=new AbortController();const timer=setTimeout(()=>controller.abort(),30_000);
const running=query({prompt:input(),options:{cwd:directory,pathToClaudeCodeExecutable:process.env.HIVE_CLAUDE_COMMAND??'claude',abortController:controller}});
const consume=(async()=>{for await(const message of running){assert.ok(message.type!=='assistant','No model assistant should be generated');}})();void consume.catch(()=>{});
try{
 await running.initializationResult();const models=await running.supportedModels();const control=running as Query &{getSettings():Promise<{applied?:{model?:string;effort?:string|null}}>};assert.equal(typeof control.getSettings,'function');
 const before=await control.getSettings();assert.ok(before.applied);const current=models.find(model=>model.value===before.applied!.model||model.resolvedModel===before.applied!.model);assert.ok(current);const levels=current.supportedEffortLevels??[];
 let readBack=before.applied.effort;
 if(typeof before.applied.effort==='string'&&levels.includes(before.applied.effort as any)){
  const target=levels.find(level=>level!==before.applied!.effort);assert.ok(target);
  await running.applyFlagSettings({effortLevel:target});readBack=(await control.getSettings()).applied?.effort;
  receipt.claudeRequested=target;
  await running.applyFlagSettings({effortLevel:before.applied.effort as any});assert.equal((await control.getSettings()).applied?.effort,before.applied.effort);
 }
 receipt.claude={modelSelectionUnchanged:true,currentModel:before.applied.model,levels,initial:before.applied.effort,readBack,sentUserMessages:0};
}finally{clearTimeout(timer);finish();running.close();await consume.catch(()=>{});}
await mkdir(resolve('../../.artifacts/external-reasoning'),{recursive:true});await writeFile(resolve('../../.artifacts/external-reasoning/native-probe.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
