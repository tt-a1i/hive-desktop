/** Actual adapters + JSONL subprocess/official SDK synthetic peers, no vendor inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createCodexSession } from '../src/agents/codex.js';
const directory=await mkdtemp(join(tmpdir(),'hive-images-peers-'));
const image={type:'image' as const,mimeType:'image/png',data:'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC'};
const previous=process.env.CLAUDE_CONFIG_DIR;process.env.CLAUDE_CONFIG_DIR=join(directory,'claude-config');
try{
 await writeFile(join(directory,'package.json'),'{"type":"commonjs"}');
 const codex=join(directory,'codex');await mkdir(codex);await writeFile(join(codex,'package.json'),'{}');
 await writeFile(join(codex,'app-server'),`const fs=require('node:fs'),rl=require('node:readline');const thread={id:'image-thread',historyMode:'legacy',path:null,turns:[]};const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');rl.createInterface({input:process.stdin}).on('line',line=>{const q=JSON.parse(line);const reply=result=>send({id:q.id,result});const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});if(q.method==='initialize')reply({});else if(q.method==='model/list')reply({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});else if(q.method==='thread/start'||q.method==='thread/resume')reply({thread,model:'gpt-6.1-terra',modelProvider:'openai'});else if(q.method==='thread/read')reply({thread});else if(q.method==='turn/start'){fs.writeFileSync('input.json',JSON.stringify(q.params.input));const turn={id:'turn',status:'completed',items:[{id:'user',type:'userMessage',content:q.params.input},{id:'assistant',type:'agentMessage',text:'Synthetic image peer acknowledged'}]};thread.turns.push(turn);reply({turn:{...turn,status:'inProgress'}});notify('turn/started',{turn:{...turn,status:'inProgress'}});for(const item of turn.items)notify('item/completed',{turnId:turn.id,item});notify('turn/completed',{turn});}});`);
 const handle=await createCodexSession({cwd:codex,sessionDir:join(directory,'codex-state'),command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(){}});
 try{
  await handle.prompt('核验原图',{images:[image],deliveryId:'codex-image',automated:true,presentation:{version:1,origin:'user',displayText:'核验原图'}});
  assert.deepEqual(JSON.parse(await readFile(join(codex,'input.json'),'utf8')),[{type:'text',text:'核验原图',text_elements:[]},{type:'image',url:`data:${image.mimeType};base64,${image.data}`}]);
  assert.deepEqual((handle.snapshot().history[0] as {content:unknown}).content,[{type:'text',text:'核验原图'},image]);
  assert.ok(handle.completedDeliveryIds().includes('codex-image'));
  const bytes=await readFile(join(codex,'input.json'),'utf8');await assert.rejects(handle.prompt('reject',{images:[{...image,data:'invalid'}]}));assert.equal(await readFile(join(codex,'input.json'),'utf8'),bytes);
 }finally{await handle.dispose();}
 // The official SDK launches .js entries with Node on Windows; .cjs is treated
 // as a native executable there and cannot be spawned by CreateProcess.
 const peer=join(directory,'claude.js');
 await writeFile(peer,`#!/usr/bin/env node
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),rl=require('node:readline');const args=process.argv.slice(2);const arg=n=>args.find(x=>x.startsWith(n+'='))?.slice(n.length+1)||(args.includes(n)?args[args.indexOf(n)+1]:undefined);const sid=arg('--session-id')||arg('--resume');const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');let parentUuid=null,servers=[];const file=path.join(process.env.CLAUDE_CONFIG_DIR,'projects',process.cwd().replace(/[^a-zA-Z0-9]/g,'-'),sid+'.jsonl');const persist=x=>{fs.mkdirSync(path.dirname(file),{recursive:true});fs.appendFileSync(file,JSON.stringify({...x,parentUuid,isSidechain:false,timestamp:new Date().toISOString(),cwd:process.cwd(),sessionId:sid})+'\\n');parentUuid=x.uuid;};rl.createInterface({input:process.stdin}).on('line',l=>{const x=JSON.parse(l);if(x.type==='control_request'){const sub=x.request.subtype;let response={};if(sub==='initialize'){servers=x.request.sdkMcpServers||[];response={commands:[],models:[{value:'gpt-6.1-terra',displayName:'Synthetic Terra peer',description:'No inference'}],account:{}};}if(sub==='mcp_status')response={mcpServers:servers.map(name=>({name,status:'connected',serverInfo:{name,version:'1'},config:{type:'sdk',name}}))};send({type:'control_response',response:{subtype:'success',request_id:x.request_id,response}});return;}if(x.type!=='user')return;fs.writeFileSync('claude-input.json',JSON.stringify(x));persist(x);send(x);const uuid=crypto.randomUUID();const a={type:'assistant',uuid,session_id:sid,parent_tool_use_id:null,message:{id:'msg_'+uuid,type:'message',role:'assistant',model:'synthetic-no-inference',content:[{type:'text',text:'Image received by synthetic peer'}],stop_reason:'end_turn',stop_sequence:null,usage:{input_tokens:0,output_tokens:0}}};persist(a);send(a);send({type:'result',subtype:'success',is_error:false,uuid:crypto.randomUUID(),session_id:sid,user_message_uuid:x.uuid,terminal_reason:'completed',result:'Synthetic image acknowledged',duration_ms:0,duration_api_ms:0,num_turns:1,total_cost_usd:0,usage:{input_tokens:0,output_tokens:0},modelUsage:{},permission_denials:[]});});`,{mode:0o700});
 const {createClaudeSession}=await import('../src/agents/claude.js');
 const claude=await createClaudeSession({cwd:directory,sessionDir:join(directory,'claude-state'),command:peer,model:{provider:'claude',id:'gpt-6.1-terra'},onEvent(){}});
 try{
  await claude.prompt('核验原图',{images:[image],deliveryId:'claude-image',automated:true,presentation:{version:1,origin:'user',displayText:'核验原图'}});
  const sent=JSON.parse(await readFile(join(directory,'claude-input.json'),'utf8'));
  assert.deepEqual(sent.message.content,[{type:'text',text:'核验原图'},{type:'image',source:{type:'base64',media_type:'image/png',data:image.data}}]);
  assert.deepEqual((claude.snapshot().history[0] as {content:unknown}).content,[{type:'text',text:'核验原图'},image]);
  assert.ok(claude.completedDeliveryIds().includes('claude-image'));
  await assert.rejects(claude.prompt('invalid',{images:[{...image,data:'invalid'}]}));
  assert.equal(JSON.parse(await readFile(join(directory,'claude-input.json'),'utf8')).uuid,sent.uuid);
 }finally{await claude.dispose();}
 console.log('PROMPT_IMAGE_PEERS_OK: Codex adapter through real JSONL child and Claude adapter through official SDK/synthetic CLI; exact image bytes on wire, durable native history normalization, delivery completion and invalid input rejected before send. No vendor requests.');
}finally{if(previous===undefined)delete process.env.CLAUDE_CONFIG_DIR;else process.env.CLAUDE_CONFIG_DIR=previous;await rm(directory,{recursive:true,force:true});}
