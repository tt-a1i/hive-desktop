const {createInterface}=require('node:readline');
const mode=process.argv[2]||'normal';const native=process.argv[3]||'fixture-session';
const send=m=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',...m})+'\n');
const update=(type,fields={},sessionId=native)=>send({method:'session/update',params:{sessionId,update:{sessionUpdate:type,...fields}}});
let pending;let permission;
createInterface({input:process.stdin}).on('line',line=>{
 const m=JSON.parse(line);if(m.method==='initialize')return send({id:m.id,result:{protocolVersion:mode==='version'?2:1,agentCapabilities:{loadSession:mode!=='no-load'}}});
 if(m.method==='session/new'){if(mode==='auth')return send({id:m.id,error:{code:-32000,message:'auth_required'}});return send({id:m.id,result:{sessionId:native}});}
 if(m.method==='session/load'){update('user_message_chunk',{content:{type:'text',text:'previous input'}});update('agent_message_chunk',{content:{type:'text',text:'restored once'}});return send({id:m.id,result:{}});}
 if(m.method==='session/cancel'){if(mode==='ignore-cancel')return;if(pending){send({id:pending,result:{stopReason:'cancelled'}});pending=undefined;}return;}
 if(m.method==='session/prompt'){
  if(mode==='eof')return process.exit(0);
  if(mode==='timeout'||mode==='ignore-cancel'||mode==='cancel'){pending=m.id;return;}
  if(mode==='permission'){permission=m.id;return send({id:'p1',method:'session/request_permission',params:{sessionId:native,toolCall:{toolCallId:'native-tool'},options:[{optionId:'deny-1',name:'Deny',kind:'reject_once'},{optionId:'allow-1',name:'Allow',kind:'allow_once'}]}});}
  if(mode==='unknown'){permission=m.id;return send({id:'unknown1',method:'fs/read_text_file',params:{}});}
  update('agent_thought_chunk',{content:{type:'text',text:'think'}});
  update('agent_message_chunk',{content:{type:'text',text:'hello'}},mode==='wrong-session'?'wrong':native);
  update('tool_call',{toolCallId:'native-tool',title:'Read',status:'pending',rawInput:{file:'a'}});
  update('tool_call_update',{toolCallId:'native-tool',status:'completed',content:[{type:'content',content:{type:'text',text:'done'}}]});
  return send({id:m.id,result:{stopReason:'end_turn'}});
 }
 if(m.id==='p1'){update('agent_message_chunk',{content:{type:'text',text:JSON.stringify(m.result)}});return send({id:permission,result:{stopReason:'end_turn'}});}
 if(m.id==='unknown1'){if(m.error?.code!==-32601)process.exit(9);return send({id:permission,result:{stopReason:'end_turn'}});}
});
