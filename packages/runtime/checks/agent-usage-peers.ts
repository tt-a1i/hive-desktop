/** Real JSONL Codex transport and official Claude SDK, synthetic native peers. No vendor inference. */
import assert from'node:assert/strict';import{mkdtemp,writeFile,readFile,mkdir}from'node:fs/promises';import{tmpdir}from'node:os';import{join,resolve}from'node:path';import{createCodexSession}from'../src/agents/codex.js';import{createClaudeSession}from'../src/agents/claude.js';import type{UsageSnapshot}from'../src/agent-usage.js';
const dir=await mkdtemp(join(tmpdir(),'hive-usage-peers-'));const counter=(s:UsageSnapshot,label:string,key:string)=>s.reports.find(r=>r.label===label)!.counters.find(c=>c.key===key)!.tokens;
const codexDir=join(dir,'codex');await mkdir(codexDir);await writeFile(join(codexDir,'app-server'),`
const {createInterface}=require('node:readline');let reads=0;
const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line);if(q.id===undefined)continue;let result;
 if(q.method==='initialize')result={};
 else if(q.method==='model/list')result={data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra synthetic',hidden:false,supportedReasoningEfforts:[]}],nextCursor:null};
 else if(q.method==='thread/resume')result={thread:{id:'usage-thread',path:null,historyMode:'legacy',turns:[]},model:'gpt-6.1-terra',modelProvider:'openai'};
 else if(q.method==='thread/read'){
  reads++;const counts={inputTokens:100+reads*10,outputTokens:20,cachedInputTokens:70,cacheWriteInputTokens:4,reasoningOutputTokens:8,totalTokens:120+reads*10};
  send({method:'thread/tokenUsage/updated',params:{threadId:'foreign',turnId:'turn',tokenUsage:{total:{...counts,totalTokens:999999},last:counts,modelContextWindow:20000}}});
  if(reads>1)send({method:'thread/tokenUsage/updated',params:{threadId:'usage-thread',turnId:'turn',tokenUsage:{total:{...counts,totalTokens:reads===4?-1:counts.totalTokens},last:counts,modelContextWindow:20000}}});result={thread:{id:'usage-thread',model:'gpt-6.1-terra',reasoningEffort:'medium'}};
 }else throw Error('Unexpected non-read request '+q.method);
 send({id:q.id,result});
}})().catch(e=>{console.error(e);process.exitCode=1;});
`);
const codex=await createCodexSession({cwd:codexDir,sessionDir:codexDir,command:process.execPath,threadId:'usage-thread',onEvent(){}});
try{
 assert.equal((await codex.usage!()).reports.length,0);await codex.reasoning!();assert.equal((await codex.usage!()).reports.length,0,'Foreign-thread-only notification must not populate counters');await codex.reasoning!();assert.equal(counter(await codex.usage!(),'原生线程累计','totalTokens'),140);await codex.reasoning!();assert.equal(counter(await codex.usage!(),'原生线程累计','totalTokens'),150,'Latest native total replaces previous total');await codex.reasoning!();await assert.rejects(async()=>codex.usage!(),TypeError);assert.equal(codex.snapshot().history.length,0,'Invalid usage must not poison transcript reads');await codex.reasoning!();assert.equal(counter(await codex.usage!(),'原生线程累计','totalTokens'),170);
}finally{await codex.dispose();}
const old={config:process.env.CLAUDE_CONFIG_DIR,wire:process.env.HIVE_CLAUDE_REASONING_WIRE};process.env.CLAUDE_CONFIG_DIR=join(dir,'claude');process.env.HIVE_CLAUDE_REASONING_WIRE=join(dir,'claude-wire.jsonl');
let source=await readFile(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url),'utf8');source=source.replace("let parentUuid=null;","let parentUuid=null;let queryCount=0;");const needle="active=x.uuid;log({event:'query',model,content:x.message.content});";
assert.ok(source.includes(needle));source=source.replace(needle,needle+`
queryCount++;const answer={type:'assistant',uuid:crypto.randomUUID(),session_id:sid,parent_tool_use_id:null,message:{id:crypto.randomUUID(),role:'assistant',model,content:[{type:'text',text:'Synthetic usage reply'}],stop_reason:'end_turn',usage:{input_tokens:queryCount*3,output_tokens:queryCount*2}}};persist(answer);send(answer);
send({type:'result',subtype:'success',is_error:false,uuid:crypto.randomUUID(),session_id:sid,user_message_uuid:x.uuid,terminal_reason:'completed',duration_ms:1,duration_api_ms:1,num_turns:1,total_cost_usd:0,usage:{input_tokens:queryCount*3,output_tokens:queryCount*2,cache_read_input_tokens:1,cache_creation_input_tokens:0},modelUsage:{'gpt-6.1-terra':{inputTokens:queryCount===1?101:141,outputTokens:queryCount*11,cacheReadInputTokens:7,cacheCreationInputTokens:2,thinkingTokens:queryCount*3,contextWindow:20000,maxOutputTokens:1000,webSearchRequests:0,costUSD:0}},permission_denials:[]});active=undefined;
`);const peer=join(dir,'claude-peer.cjs');await writeFile(peer,source,{mode:0o755});let claude:Awaited<ReturnType<typeof createClaudeSession>>|undefined;
try{
 claude=await createClaudeSession({cwd:dir,sessionDir:join(dir,'metadata'),command:peer,model:{provider:'claude',id:'gpt-6.1-terra'},onEvent(){}});assert.equal((await claude.usage!()).reports.length,0);const id=claude.sessionId;await claude.prompt('First synthetic turn');let stats=await claude.usage!();assert.equal(counter(stats,'最近一轮主循环','input_tokens'),3);assert.equal(counter(stats,'模型累计 · gpt-6.1-terra','inputTokens'),101);await claude.prompt('Second synthetic turn');stats=await claude.usage!();assert.equal(counter(stats,'最近一轮主循环','input_tokens'),6);assert.equal(counter(stats,'模型累计 · gpt-6.1-terra','inputTokens'),141,'Do not sum cumulative result values');assert.equal(stats.context,undefined);await claude.dispose();claude=await createClaudeSession({cwd:dir,sessionDir:join(dir,'metadata'),command:peer,sessionId:id,onEvent(){}});assert.equal((await claude.usage!()).reports.length,0,'Reopen cannot invent native result statistics');assert.ok(claude.snapshot().history.some((m:any)=>m.role==='assistant'));
 await mkdir(resolve('../../.artifacts/agent-usage'),{recursive:true});await writeFile(resolve('../../.artifacts/agent-usage/peers-receipt.json'),JSON.stringify({pass:true,dir,codexSyntheticJsonl:true,claudeOfficialSdk:true,claudeSyntheticCli:true,vendorInference:false,foreignThreadIgnored:true,cumulativeNotResummed:true,malformedUsageReadOnlyFailure:true},null,2));console.log('AGENT_USAGE_PEERS_OK: Codex thread scoping/latest native totals and malformed recovery; official Claude SDK per-turn vs cumulative, history reopen unavailable. Synthetic peers.');
}finally{await claude?.dispose();if(old.config===undefined)delete process.env.CLAUDE_CONFIG_DIR;else process.env.CLAUDE_CONFIG_DIR=old.config;if(old.wire===undefined)delete process.env.HIVE_CLAUDE_REASONING_WIRE;else process.env.HIVE_CLAUDE_REASONING_WIRE=old.wire;}
