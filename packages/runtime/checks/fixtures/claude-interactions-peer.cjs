#!/usr/bin/env node
// Synthetic protocol peer; never contacts a model provider.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),readline=require('node:readline');
const root=process.env.HIVE_CLAUDE_FIXTURE_DIR, receipt=process.env.HIVE_CLAUDE_FIXTURE_RECEIPT, control=process.env.HIVE_CLAUDE_FIXTURE_CONTROL;
const args=process.argv.slice(2);const getArg=n=>args.find(a=>a.startsWith(n+'='))?.slice(n.length+1) ?? (args.indexOf(n)>=0 ? args[args.indexOf(n)+1] : undefined);const sid=getArg('--session-id')||getArg('--resume');let servers=[],active;
const log=x=>fs.appendFileSync(receipt,JSON.stringify({pid:process.pid,synthetic:true,noInference:true,...x})+'\n');
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
const sessionPath=path.join(process.env.CLAUDE_CONFIG_DIR,'projects',process.cwd().replace(/[^a-zA-Z0-9]/g,'-'),`${sid}.jsonl`);
let parentUuid=null;try { for(const line of fs.readFileSync(sessionPath,'utf8').trim().split('\n')) { const row=JSON.parse(line); if(row.uuid && (row.type==='user'||row.type==='assistant')) parentUuid=row.uuid; } } catch {}
const persist=x=>{fs.mkdirSync(path.dirname(sessionPath),{recursive:true});fs.appendFileSync(sessionPath,JSON.stringify({...x,parentUuid,isSidechain:false,timestamp:new Date().toISOString(),cwd:process.cwd(),sessionId:sid})+'\n');parentUuid=x.uuid;};
log({event:'start',sid,args});
readline.createInterface({input:process.stdin}).on('line',line=>{let x;try{x=JSON.parse(line)}catch{return}log({event:'incoming',type:x.type,subtype:x.request?.subtype,request_id:x.request_id});
 if(x.type==='control_request'){
  const sub=x.request.subtype;let response={};
  if(sub==='initialize'){servers=x.request.sdkMcpServers||[];response={commands:[],models:[{value:'default',displayName:'Synthetic Claude',description:'No inference'}],account:{}};}
  if(sub==='mcp_status')response={mcpServers:servers.map(name=>({name,status:'connected',serverInfo:{name,version:'1.0'},config:{type:'sdk',name}}))};
  if(sub==='interrupt'){const run=active;active=undefined;if(run)send({type:'control_cancel_request',request_id:run.id});send({type:'control_response',response:{subtype:'success',request_id:x.request_id,response}});if(run){log({event:'aborted',id:run.id});send({type:'result',subtype:'error_during_execution',is_error:true,uuid:crypto.randomUUID(),session_id:sid,user_message_uuid:run.user,terminal_reason:'aborted',errors:['Synthetic interruption'],duration_ms:0,duration_api_ms:0,num_turns:0,total_cost_usd:0,usage:{input_tokens:0,output_tokens:0},modelUsage:{},permission_denials:[]});}return;}
  send({type:'control_response',response:{subtype:'success',request_id:x.request_id,response}});return;
 }
 if(x.type==='user'){
  persist(x);if(x.shouldQuery===false)return;
  let c;try{c=JSON.parse(fs.readFileSync(control,'utf8'))}catch{c={scenario:'permission'}}
  const id='fixture-'+crypto.randomUUID();let tool='Bash',input={command:'echo fixture-only'};
  if(c.scenario==='questions') {tool='AskUserQuestion';input={questions:[{question:'请选择一个选项？',header:'单选',multiSelect:false,options:[{label:'选项甲',description:'第一项'},{label:'选项乙',description:'第二项'}]},{question:'请选择多个选项？',header:'多选',multiSelect:true,options:[{label:'选项一',description:'第一项'},{label:'选项二',description:'第二项'}]}]};}
  if(c.scenario==='long')input={command:'echo '+('这是用于检查窄窗口内容换行的长文本 '.repeat(40))};
  active={id,user:x.uuid,scenario:c.scenario};log({event:'request',id,scenario:c.scenario,session:sid});
  send({type:'control_request',request_id:id,request:{subtype:'can_use_tool',tool_name:tool,input,tool_use_id:'tool-'+id}});return;
 }
 if(x.type==='control_response'&&active&&x.response.request_id===active.id){const run=active;active=undefined;log({event:'answer',id:run.id,response:x.response.response});const uuid=crypto.randomUUID();const assistant={type:'assistant',uuid,session_id:sid,parent_tool_use_id:null,message:{id:'msg_'+uuid,type:'message',role:'assistant',model:'synthetic',content:[{type:'text',text:'夹具交互已结束。'}],stop_reason:'end_turn',stop_sequence:null,usage:{input_tokens:0,output_tokens:0}}};persist(assistant);send(assistant);send({type:'result',subtype:'success',is_error:false,uuid:crypto.randomUUID(),session_id:sid,user_message_uuid:run.user,terminal_reason:'completed',result:'夹具交互已结束。',duration_ms:0,duration_api_ms:0,num_turns:1,total_cost_usd:0,usage:{input_tokens:0,output_tokens:0},modelUsage:{},permission_denials:[]});}
});
