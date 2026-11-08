/** Native Pi SDK + original Pi RPC; no inference. Codex/Claude normalization uses explicit native fields. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile}from'node:fs/promises';
import{join,resolve}from'node:path';import{tmpdir}from'node:os';
import{SessionManager}from'@earendil-works/pi-coding-agent';
import{createPiSession}from'../src/agents/pi.js';import{createExternalPiSession}from'../src/agents/external-pi.js';
import{piUsage,codexUsage,claudeUsage,type UsageSnapshot}from'../src/agent-usage.js';
const dir=await mkdtemp(join(tmpdir(),'hive-agent-usage-'));const pi=join(dir,'pi');await mkdir(pi);
const prior=process.env.PI_CODING_AGENT_DIR;process.env.PI_CODING_AGENT_DIR=pi;
await writeFile(join(pi,'settings.json'),JSON.stringify({defaultProvider:'native-usage',defaultModel:'gpt-6.1-terra'}));await writeFile(join(pi,'models.json'),JSON.stringify({providers:{'native-usage':{api:'openai-responses',apiKey:'local-only',baseUrl:'http://127.0.0.1:9/v1',models:[{id:'gpt-6.1-terra',name:'Terra local usage',contextWindow:20000,maxTokens:1000,input:['text']}]}}}));
const manager=SessionManager.create(dir,join(dir,'sessions'));manager.appendModelChange('native-usage','gpt-6.1-terra');manager.appendMessage({role:'user',content:[{type:'text',text:'Seed persisted usage; no model request'}],timestamp:Date.now()});
manager.appendMessage({role:'assistant',content:[{type:'text',text:'Recorded native usage'}],api:'openai-responses',provider:'native-usage',model:'gpt-6.1-terra',usage:{input:37,output:11,cacheRead:7,cacheWrite:3,totalTokens:58,cost:{input:0,output:0,cacheRead:0,cacheWrite:0,total:0}},stopReason:'stop',timestamp:Date.now()});
const file=manager.getSessionFile()!;let native:Awaited<ReturnType<typeof createPiSession>>|undefined;let external:Awaited<ReturnType<typeof createExternalPiSession>>|undefined;
const values=(s:UsageSnapshot)=>Object.fromEntries(s.reports[0]!.counters.map(c=>[c.key,c.tokens]));
try{
 native=await createPiSession({cwd:dir,sessionDir:join(dir,'sessions'),agentDir:pi,sessionFile:file,onEvent(){}});const stats=await native.usage!();assert.deepEqual(values(stats),{input:37,output:11,cacheRead:7,cacheWrite:3,total:58});assert.equal(stats.context?.window,20000);assert.equal(stats.context?.estimated,true);assert.ok(stats.context?.tokens!==null&&stats.context?.tokens!==undefined&&stats.context.tokens>0);assert.ok(stats.context?.percent!==null);
 native.dispose();native=undefined;
 external=await createExternalPiSession({cwd:dir,sessionDir:join(dir,'sessions'),sessionFile:file,command:process.env.HIVE_PI_COMMAND??'/Users/tsk/Library/pnpm/pi',onEvent(){}});assert.deepEqual(values(await external.usage!()),values(stats));await external.dispose();external=undefined;
 const compacted=SessionManager.open(file,join(dir,'sessions'));const user=compacted.getEntries().find(e=>e.type==='message'&&e.message.role==='user');assert.ok(user);compacted.appendCompaction('Recorded summary fixture',user.id,58);
 native=await createPiSession({cwd:dir,sessionDir:join(dir,'sessions'),agentDir:pi,sessionFile:file,onEvent(){}});const after=await native.usage!();assert.equal(after.context?.tokens,null);assert.equal(after.context?.percent,null);assert.deepEqual(values(after),values(stats),'Compacted-away usage remains in native session totals');native.dispose();native=undefined;
 assert.equal(codexUsage(undefined).reports.length,0);assert.equal(claudeUsage(undefined).reports.length,0);
 const codes={inputTokens:100,outputTokens:20,cachedInputTokens:70,cacheWriteInputTokens:4,reasoningOutputTokens:8,totalTokens:120};const cx=codexUsage({total:codes,last:codes,modelContextWindow:20000});assert.equal(cx.reports[0]!.counters.find(c=>c.key==='totalTokens')!.tokens,120,'Cache/reasoning must not be added to native total');assert.equal(cx.context,undefined,'Last token usage is not a context measurement');
 const unknown=piUsage({tokens:{input:0,output:0,cacheRead:0,cacheWrite:0,total:0},contextUsage:{tokens:null,contextWindow:20000,percent:null}},'Fixture');assert.equal(unknown.context?.tokens,null);assert.equal(unknown.context?.percent,null);
 for(const invalid of [-1,NaN,Infinity,1.5,Number.MAX_SAFE_INTEGER+1])assert.throws(()=>codexUsage({total:{totalTokens:invalid},last:{},modelContextWindow:null}),TypeError);
 assert.throws(()=>piUsage({tokens:{total:1},contextUsage:{tokens:1,contextWindow:0,percent:0}},'Invalid'),TypeError);
 await mkdir(resolve('../../.artifacts/agent-usage'),{recursive:true});await writeFile(resolve('../../.artifacts/agent-usage/pi-receipt.json'),JSON.stringify({pass:true,dir,file,nativePiSdk:true,originalPiRpc:true,compactionUnknownContext:true,vendorInference:false,seededTranscriptUsage:true},null,2));console.log('AGENT_USAGE_PI_OK: real SDK/RPC and durable native usage, compaction null vs zero, exact native counters and invalid-value rejection. No inference.');
}finally{native?.dispose();await external?.dispose();if(prior===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=prior;}
