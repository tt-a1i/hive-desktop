#!/usr/bin/env node
// Synthetic native peer for the official SDK. No network or model provider.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),readline=require('node:readline');
const receipt=process.env.HIVE_CLAUDE_REASONING_WIRE,control=process.env.HIVE_CLAUDE_REASONING_CONTROL;
const args=process.argv.slice(2);const arg=name=>args.find(v=>v.startsWith(name+'='))?.slice(name.length+1)??(args.indexOf(name)>=0?args[args.indexOf(name)+1]:undefined);
const sid=arg('--session-id')||arg('--resume');let servers=[],effort='medium',model='gpt-6.1-terra',active;
const models=[{value:'gpt-6.1-terra',resolvedModel:'gpt-6.1-terra',displayName:'Terra synthetic control peer',description:'No inference',supportsEffort:true,supportedEffortLevels:['low','medium','high']}];
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');const log=x=>fs.appendFileSync(receipt,JSON.stringify(x)+'\n');
const file=path.join(process.env.CLAUDE_CONFIG_DIR,'projects',process.cwd().replace(/[^a-zA-Z0-9]/g,'-'),`${sid}.jsonl`);let parentUuid=null;
try{for(const line of fs.readFileSync(file,'utf8').trim().split('\n')){const x=JSON.parse(line);if(x.uuid)parentUuid=x.uuid;}}catch{}
const persist=x=>{fs.mkdirSync(path.dirname(file),{recursive:true});fs.appendFileSync(file,JSON.stringify({...x,parentUuid,isSidechain:false,timestamp:new Date().toISOString(),cwd:process.cwd(),sessionId:sid})+'\n');parentUuid=x.uuid;};
log({event:'start',sid,model});
readline.createInterface({input:process.stdin}).on('line',line=>{
 const x=JSON.parse(line);
 if(x.type==='control_request'){
  const sub=x.request.subtype;const answer=response=>send({type:'control_response',response:{subtype:'success',request_id:x.request_id,response}});
  if(sub==='initialize'){servers=x.request.sdkMcpServers||[];answer({commands:[],models,account:{}});}
  else if(sub==='mcp_status')answer({mcpServers:servers.map(name=>({name,status:'connected',serverInfo:{name,version:'1'},config:{type:'sdk',name}}))});
  else if(sub==='get_settings')answer({applied:{model,effort}});
  else if(sub==='apply_flag_settings'){
   const settings=x.request.settings;log({event:'settings',settings});let c={};try{c=JSON.parse(fs.readFileSync(control,'utf8'));}catch{}
   if(c.rejectEffort===settings.effortLevel){send({type:'control_response',response:{subtype:'error',request_id:x.request_id,error:'Synthetic rejected effort'}});return;}
   effort=c.clampEffort||settings.effortLevel;answer({});
  }else if(sub==='set_model'){model=x.request.model;answer({});}
  else if(sub==='interrupt'){answer({});if(active){const user=active;active=undefined;send({type:'result',subtype:'error_during_execution',is_error:true,uuid:crypto.randomUUID(),session_id:sid,user_message_uuid:user,terminal_reason:'aborted',errors:['Synthetic abort'],duration_ms:0,duration_api_ms:0,num_turns:0,total_cost_usd:0,usage:{input_tokens:0,output_tokens:0},modelUsage:{},permission_denials:[]});}}
  else answer({});
 }else if(x.type==='user'){persist(x);if(x.shouldQuery!==false){active=x.uuid;log({event:'query',model});}}
});
