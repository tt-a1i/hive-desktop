// Controlled ACP agent for production registration checks. Never runs a model or reads credentials.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{spawn}=require('node:child_process'),rl=require('node:readline');
const root=process.argv[2],mode=process.argv[3]||'normal';fs.mkdirSync(root,{recursive:true});
let sid,history=[],bridge,lines,next=0;
const send=value=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',...value})+'\n');
const log=value=>fs.appendFileSync(path.join(root,'protocol.jsonl'),JSON.stringify(value)+'\n');
log({event:'start',pid:process.pid,cwd:process.cwd(),mode});
const file=()=>path.join(root,sid+'.json');
const update=(text,id=crypto.randomUUID(),kind='agent_message_chunk')=>send({method:'session/update',params:{sessionId:sid,update:{sessionUpdate:kind,messageId:id,content:{type:'text',text}}}});
async function rpc(method,params){const id=++next;bridge.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');for(;;){const message=JSON.parse((await lines.next()).value);if(message.id!==id)continue;if(message.error)throw Error(JSON.stringify(message.error));return message.result;}}
async function connect(servers){if(mode==='no-mcp')return;const server=servers[0];if(!server)throw Error('Production session omitted team bridge');bridge=spawn(server.command,server.args,{env:process.env,stdio:['pipe','pipe','pipe']});lines=rl.createInterface({input:bridge.stdout})[Symbol.asyncIterator]();await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'controlled-production-peer',version:'1'}});bridge.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');await rpc('tools/list',{});}
let requests=Promise.resolve();
rl.createInterface({input:process.stdin}).on('line',line=>{requests=requests.then(async()=>{const q=JSON.parse(line);log({method:q.method,sessionId:q.params?.sessionId,cwd:q.params?.cwd});const reply=result=>send({id:q.id,result});
if(q.method==='initialize')return reply({protocolVersion:1,agentCapabilities:{loadSession:mode!=='no-load'},agentInfo:{name:'controlled-production-peer',version:'1'}});
if(q.method==='session/new'){if(mode==='auth')return send({id:q.id,error:{code:-32000,message:'auth_required'}});sid=crypto.randomUUID();history=[];fs.writeFileSync(file(),JSON.stringify({history}));if(mode==='delayed-mcp'){reply({sessionId:sid});setTimeout(()=>connect(q.params.mcpServers).catch(error=>{process.stderr.write(String(error));process.exit(2);}),1600);return;}await connect(q.params.mcpServers);return reply({sessionId:sid});}
if(q.method==='session/load'){sid=q.params.sessionId;history=JSON.parse(fs.readFileSync(file(),'utf8')).history;await connect(q.params.mcpServers);for(const item of history)update(item.text,item.id,item.kind);return reply({});}
if(q.method==='session/prompt'){const text=q.params.prompt[0].text;const user={id:crypto.randomUUID(),kind:'user_message_chunk',text};history.push(user);if(mode==='uncertain'&&!fs.existsSync(path.join(root,'allow'))){fs.writeFileSync(file(),JSON.stringify({history}));bridge?.stdin.end();return process.exit(2);}let result={content:[{type:'text',text:'turn ended without a Hive report'}]};const marker='ACP_TEST_COMMAND:';if(text.includes(marker)){const args=JSON.parse(text.slice(text.lastIndexOf(marker)+marker.length).split('\n')[0]);if(args!==null)result=await rpc('tools/call',{name:'team',arguments:{args}});}const message={id:crypto.randomUUID(),kind:'agent_message_chunk',text:'ACP_TEST_RESULT:'+JSON.stringify(result)};history.push(message);fs.writeFileSync(file(),JSON.stringify({history}));update(message.text,message.id);return reply({stopReason:'end_turn'});}
if(q.method==='session/cancel')return;
throw Error('Unexpected ACP method '+q.method);
}).catch(error=>{process.stderr.write(String(error));process.exit(2);});});
process.stdin.on('end',()=>{bridge?.stdin.end();});
