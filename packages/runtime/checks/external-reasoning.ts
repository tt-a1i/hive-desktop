/** Real Hive/SQLite + original Codex controls; official Claude SDK + synthetic peer. No vendor inference. */
import assert from 'node:assert/strict';import {mkdtemp,mkdir,writeFile,readFile,rename,rm}from'node:fs/promises';import{tmpdir}from'node:os';import{join,resolve}from'node:path';import{fileURLToPath}from'node:url';
import{ChatIndex}from'../src/chat-index.js';
const dir=await mkdtemp(join(tmpdir(),'hive-external-reasoning-'));const envKeys=['HIVE_CODEX_COMMAND','HIVE_CLAUDE_COMMAND','CLAUDE_CONFIG_DIR','HIVE_CLAUDE_REASONING_WIRE','HIVE_CLAUDE_REASONING_CONTROL']as const;const previous=Object.fromEntries(envKeys.map(key=>[key,process.env[key]]));
process.env.HIVE_CODEX_COMMAND=process.env.HIVE_CODEX_COMMAND??'codex';process.env.HIVE_CLAUDE_COMMAND=fileURLToPath(new URL('./fixtures/claude-reasoning-peer.cjs',import.meta.url));process.env.CLAUDE_CONFIG_DIR=join(dir,'claude-config');process.env.HIVE_CLAUDE_REASONING_WIRE=join(dir,'wire.jsonl');process.env.HIVE_CLAUDE_REASONING_CONTROL=join(dir,'control.json');await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL,'{}');
const{createDesktopRuntime}=await import('../src/desktop-runtime.js');let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
try{
 for(const backend of ['codex','claude']as const){
  const state=join(dir,backend),project=join(dir,backend+'-project');await mkdir(project);runtime=await createDesktopRuntime(state,()=>{});
  const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:backend,autostart_orchestrator:false})as{id:string};const chat=await runtime.chats.createRecord(ws.id,backend);await runtime.close();runtime=undefined;
  const indexFile=join(state,'chats/index.json');const originalIndex=await readFile(indexFile,'utf8');
  for(const reasoning of [null,[],{},true,{provider:'openai',modelId:'gpt-6.1-terra',level:4},{provider:'',modelId:'gpt-6.1-terra',level:'high'}]){
    const rows=JSON.parse(originalIndex);rows[0].reasoning=reasoning;await writeFile(indexFile,JSON.stringify(rows));assert.throws(()=>new ChatIndex(join(state,'chats')));
  }
  await writeFile(indexFile,originalIndex);
  const index=new ChatIndex(join(state,'chats'));index.save({...index.get(chat.id),model:{provider:backend==='codex'?'openai':'claude',id:'gpt-6.1-terra',name:'Terra control check'}});
  runtime=await createDesktopRuntime(state,()=>{});const opened=await runtime.chats.open(chat.id);assert.equal(opened.model?.id,'gpt-6.1-terra');
  const available=await runtime.chats.reasoning(chat.id);assert.ok(available.levels.includes('high'));assert.ok(available.levels.includes('low'));
  assert.equal((await runtime.chats.reasoning(chat.id,'high')).level,'high');await assert.rejects(runtime.chats.reasoning(chat.id,'not-a-level'),TypeError);
  const saved=new ChatIndex(join(state,'chats')).get(chat.id);assert.deepEqual(saved.reasoning,{provider:backend==='codex'?'openai':'claude',modelId:'gpt-6.1-terra',level:'high'});
  await runtime.chats.setModel(chat.id,backend==='codex'?'openai':'claude','gpt-6.1-terra');assert.equal((await runtime.chats.reasoning(chat.id)).level,'high','Re-selecting the current model must not reset reasoning');
  await runtime.close();runtime=await createDesktopRuntime(state,()=>{});assert.equal((await runtime.chats.reasoning(chat.id)).level,'high','Preference must survive an empty native thread/session restart');
  const file=join(state,'chats/index.json');const before=await readFile(file,'utf8');await rename(file,file+'.retained');await mkdir(file);
  try{await assert.rejects(runtime.chats.reasoning(chat.id,'low'));}finally{await rm(file,{recursive:true});await rename(file+'.retained',file);}
  assert.equal(await readFile(file,'utf8'),before);assert.equal((await runtime.chats.reasoning(chat.id)).level,'high','Disk error must not publish an unsaved override');
  if(backend==='claude'){
   await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL!,JSON.stringify({rejectEffort:'low'}));await assert.rejects(runtime.chats.reasoning(chat.id,'low'));assert.equal((await runtime.chats.reasoning(chat.id)).level,'high');
   await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL!,JSON.stringify({clampEffort:'medium'}));assert.equal((await runtime.chats.reasoning(chat.id,'high')).level,'medium','UI must read native applied value, not echo requested level');
   await writeFile(process.env.HIVE_CLAUDE_REASONING_CONTROL!,'{}');
  }
  await runtime.close();runtime=undefined;
  const stale=new ChatIndex(join(state,'chats'));stale.save({...stale.get(chat.id),reasoning:{...stale.get(chat.id).reasoning!,level:'retired-level'}});
  runtime=await createDesktopRuntime(state,()=>{});const history=await runtime.chats.open(chat.id);assert.equal(history.id,chat.id);assert.equal(history.model?.id,'gpt-6.1-terra');
  const unavailable=await runtime.chats.reasoning(chat.id);assert.ok(unavailable.description?.includes('原设置未能恢复'));
  await assert.rejects(runtime.chats.prompt(chat.id,'Must not be submitted while saved effort is unavailable'));
  assert.equal((await runtime.chats.open(chat.id)).pending,0);
  assert.equal((await runtime.chats.reasoning(chat.id,'low')).level,'low');await runtime.close();runtime=undefined;
 }
 const lines=(await readFile(process.env.HIVE_CLAUDE_REASONING_WIRE!,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
 assert.ok(lines.some(line=>line.event==='settings'&&line.settings.effortLevel==='high'));assert.ok(lines.filter(line=>line.event==='settings').every(line=>Object.keys(line.settings).join(',')==='effortLevel'));assert.equal(lines.filter(line=>line.event==='query').length,0);
 await mkdir(resolve('../../.artifacts/external-reasoning'),{recursive:true});await writeFile(resolve('../../.artifacts/external-reasoning/core-receipt.json'),JSON.stringify({dir,pass:true,codexOriginalCli:true,claudeSyntheticPeer:true,officialSdk:true,model:'gpt-6.1-terra',vendorInference:false},null,2));console.log('EXTERNAL_REASONING_OK: actual Codex controls and SDK peer, saved preferences, same-model no-op, restart, disk/native errors, applied clamp readback and unsupported cold-session repair. No inference.');
}finally{await runtime?.close();for(const key of envKeys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}}
