// Controlled ACP agent for production registration checks. Never runs a model or reads credentials.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{spawn}=require('node:child_process'),rl=require('node:readline'),{setTimeout:delay}=require('node:timers/promises');
const root=process.argv[2],mode=process.argv[3]||'normal';fs.mkdirSync(root,{recursive:true});
let sid,history=[],bridge,lines,next=0;
let selected={mode:'read',collaboration_mode:'default'};
const send=value=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',...value})+'\n');
const log=value=>fs.appendFileSync(path.join(root,'protocol.jsonl'),JSON.stringify(value)+'\n');
log({event:'start',pid:process.pid,cwd:process.cwd(),mode});
const file=()=>path.join(root,sid+'.json');
const modeState=()=>{
 if(mode==='legacy-modes')return {modes:{currentModeId:selected.mode,availableModes:[{id:'read',name:'Peer Read'},{id:'write',name:'Peer Write'}]}};
 if(mode!=='config-modes')return {};
 return {configOptions:[{id:'mode',name:'Access',category:'mode',type:'select',currentValue:selected.mode,options:[{group:'permissions',name:'Permissions',options:[{value:'read',name:'Peer Read'},{value:'write',name:'Peer Write'}]}]},
   {id:'collaboration_mode',name:'Behavior',category:'collaboration_mode',type:'select',currentValue:selected.collaboration_mode,options:[{value:'default',name:'Peer Build'},{value:'plan',name:'Peer Plan'}]}]};
};
const persist=()=>fs.writeFileSync(file(),JSON.stringify({history,selected}));
const update=(text,id=crypto.randomUUID(),kind='agent_message_chunk')=>send({method:'session/update',params:{sessionId:sid,update:{sessionUpdate:kind,messageId:id,content:{type:'text',text}}}});
async function rpc(method,params){const id=++next;bridge.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');for(;;){const message=JSON.parse((await lines.next()).value);if(message.id!==id)continue;if(message.error)throw Error(JSON.stringify(message.error));return message.result;}}
async function connect(servers){if(mode==='no-mcp')return;const server=servers[0];if(!server)throw Error('Production session omitted team bridge');const env=Object.fromEntries(['HOME','LOGNAME','PATH','SHELL','USER','__CF_USER_TEXT_ENCODING','LANG','LC_ALL','TERM','TMPDIR','TZ'].flatMap(name=>process.env[name]===undefined?[]:[[name,process.env[name]]]));for(const {name,value} of server.env)env[name]=value;const binding=JSON.parse(env.HIVE_ACP_TEAM_BINDING);if(process.argv.some(arg=>arg.includes(binding.token)))throw Error('token reached ACP argv');bridge=spawn(server.command,server.args,{env,stdio:['pipe','pipe','pipe']});lines=rl.createInterface({input:bridge.stdout})[Symbol.asyncIterator]();await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'controlled-production-peer',version:'1'}});bridge.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');await rpc('tools/list',{});}
let requests=Promise.resolve();
rl.createInterface({input:process.stdin}).on('line',line=>{requests=requests.then(async()=>{const q=JSON.parse(line);log({method:q.method,sessionId:q.params?.sessionId,cwd:q.params?.cwd,configId:q.params?.configId,value:q.params?.value,modeId:q.params?.modeId});const reply=result=>send({id:q.id,result});
if(q.method==='initialize'){if('HIVE_ACP_TEAM_BINDING'in process.env||'HIVE_ACP_TEAM_PROOF'in process.env)throw Error('private bridge environment reached ACP parent');return reply({protocolVersion:1,agentCapabilities:{loadSession:mode!=='no-load'},agentInfo:{name:'controlled-production-peer',version:'1'}});}
if(q.method==='session/new'){if(mode==='auth')return send({id:q.id,error:{code:-32000,message:'auth_required'}});sid=crypto.randomUUID();history=[];persist();if(mode==='delayed-mcp'){reply({sessionId:sid});setTimeout(()=>connect(q.params.mcpServers).catch(error=>{process.stderr.write(String(error));process.exit(2);}),1600);return;}await connect(q.params.mcpServers);return reply({sessionId:sid,...modeState()});}
if(q.method==='session/load'){sid=q.params.sessionId;const saved=JSON.parse(fs.readFileSync(file(),'utf8'));history=saved.history;selected=saved.selected??selected;await connect(q.params.mcpServers);for(const item of history)update(item.text,item.id,item.kind);return reply(modeState());}
if(q.method==='session/set_mode'&&mode==='legacy-modes'){
 if(q.params.sessionId!==sid||!['read','write'].includes(q.params.modeId))throw Error('Wrong native mode identity');
 selected.mode=q.params.modeId;persist();return reply({});
}
if(q.method==='session/set_config_option'&&mode==='config-modes'){
 const {configId,value,sessionId}=q.params;
 const options=modeState().configOptions.find(option=>option.id===configId)?.options.flatMap(option=>option.options??[option]);
 if(sessionId!==sid||!options?.some(option=>option.value===value))throw Error('Wrong native option identity');
 if(fs.existsSync(path.join(root,'reject-mode')))return reply(modeState());
 selected[configId]=value;persist();
 if(fs.existsSync(path.join(root,'agent-mode-update'))){
  const response={jsonrpc:'2.0',id:q.id,result:modeState()};
  selected.collaboration_mode='default';persist();
  const notification={jsonrpc:'2.0',method:'session/update',params:{sessionId:sid,update:{sessionUpdate:'config_option_update',...modeState()}}};
  process.stdout.write(JSON.stringify(response)+'\n'+JSON.stringify(notification)+'\n');
 } else reply(modeState());
 return;
}
if(q.method==='session/prompt'){const text=q.params.prompt[0].text;const user={id:crypto.randomUUID(),kind:'user_message_chunk',text};history.push(user);if(mode==='uncertain'&&!fs.existsSync(path.join(root,'allow'))){fs.writeFileSync(file(),JSON.stringify({history}));bridge?.stdin.end();return process.exit(2);}
if(mode==='prompt-gate'){while(fs.existsSync(path.join(root,'hold-prompt')))await delay(10);}
let result={content:[{type:'text',text:'turn ended without a Hive report'}]};const marker='ACP_TEST_COMMAND:';if(text.includes(marker)){const args=JSON.parse(text.slice(text.lastIndexOf(marker)+marker.length).split('\n')[0]);if(args!==null)result=await rpc('tools/call',{name:'team',arguments:{args}});}const message={id:crypto.randomUUID(),kind:'agent_message_chunk',text:'ACP_TEST_RESULT:'+JSON.stringify(result)};history.push(message);persist();update(message.text,message.id);return reply({stopReason:'end_turn'});}
if(q.method==='session/cancel')return;
throw Error('Unexpected ACP method '+q.method);
}).catch(error=>{process.stderr.write(String(error));process.exit(2);});});
process.stdin.on('end',()=>{bridge?.stdin.end();});
