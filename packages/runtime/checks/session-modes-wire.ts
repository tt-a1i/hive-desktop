/** Production JSONL RPCs with synthetic local Codex and Claude peers; no inference. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,readFile,unlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
const dir=await mkdtemp(join(tmpdir(),'hive-session-modes-wire-'));
const runtime=resolve(process.env.HIVE_MODES_RUNTIME??'.');
const codex=fileURLToPath(new URL('./codex-modes-peer.cjs',import.meta.url));
const claude=fileURLToPath(new URL('./fixtures/claude-modes-peer.cjs',import.meta.url));
const acp=fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs',import.meta.url));
const config=join(dir,'claude'),data=join(dir,'desktop'),wire=join(dir,'claude-wire'),control=join(dir,'claude-control');
await mkdir(config);await writeFile(control,'{}');
function start(){
 const child=spawn(process.execPath,[join(runtime,'dist/main.js')],{cwd:dir,env:{...process.env,HIVE_DESKTOP_DATA_DIR:data,CLAUDE_CONFIG_DIR:config,HIVE_CODEX_COMMAND:codex,HIVE_CLAUDE_COMMAND:claude,HIVE_CLAUDE_MODES_WIRE:wire,HIVE_CLAUDE_MODES_CONTROL:control},stdio:['pipe','pipe','pipe']});
 const closed=once(child,'close');let errors='';child.stderr.on('data',chunk=>{errors+=chunk;process.stderr.write(chunk);});let sequence=0;
 const requests=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void}>();const agent:any[]=[];
 const output=createInterface({input:child.stdout});output.on('line',line=>{const value=JSON.parse(line);if(value.event==='agent')agent.push(value.data);requests.get(value.id)?.resolve(value);});
 child.on('close',()=>{for(const request of requests.values())request.reject(new Error('Runtime closed: '+errors));requests.clear();});
 async function rpc(method:string,params={}){const id=++sequence;let timer:ReturnType<typeof setTimeout>;try{return await new Promise<any>((yes,no)=>{timer=setTimeout(()=>no(new Error('Timed out: '+method)),20000);requests.set(id,{resolve:yes,reject:no});child.stdin.write(JSON.stringify({id,method,params})+'\n');});}finally{clearTimeout(timer!);requests.delete(id);}}
 async function stop(){if(child.exitCode!==null||child.signalCode!==null){await closed;output.close();return;}try{assert.equal((await rpc('shutdown')).result.ok,true);assert.equal((await closed)[0],0);}finally{if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await closed;}output.close();}}
 return {rpc,stop,agent};
}
let session=start();const ids:Record<string,string>={};
const acpRuns = new Map<string, {sessionId: string; root: string; nativeId: string; mode: string; permission?: string}>();
const api = async (method: string, params: Record<string, unknown> = {}) => {
 const response = await session.rpc(method, params);
 assert.equal(response.error, undefined, JSON.stringify(response.error));
 return response.result;
};
const modeOption = (state: {options: {id:string;label:string}[]}, label: string) => {
 const option = state.options.find(value => value.label === label);
 assert.ok(option, label);
 return option.id;
};
try{
 assert.ok((await session.rpc('initialize')).result.hivePort);
 const ws=(await session.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:dir,name:'Mode wire',autostart_orchestrator:false}})).result;
 for(const backend of ['codex','claude']){const response=await session.rpc('chat.createRecord',{workspaceId:ws.id,backend});assert.equal(response.error,undefined);ids[backend]=response.result.id;}
 for(const backend of ['codex','claude']){
  const sessionId=ids[backend],selected=backend==='codex'?'native-plan':'acceptEdits';
  const initial=await session.rpc('chat.mode',{sessionId});assert.equal(initial.error,undefined);
  const changed=await session.rpc('chat.setMode',{sessionId,mode:selected});assert.equal(changed.error,undefined);assert.equal(changed.result.selected,selected);
  assert.equal((await session.rpc('chat.mode',{sessionId})).result.selected,selected);
  assert.ok((await session.rpc('chat.setMode',{sessionId,mode:'invalid-mode'})).error);
 }
 const permission=await session.rpc('chat.permissionMode',{sessionId:ids.codex});assert.equal(permission.error,undefined);
 const changed=await session.rpc('chat.setPermissionMode',{sessionId:ids.codex,mode:'workspace-write'});assert.equal(changed.error,undefined);assert.equal(changed.result.selected,'workspace-write');
 assert.equal((await session.rpc('chat.permissionMode',{sessionId:ids.codex})).result.selected,'workspace-write');
 assert.ok((await session.rpc('chat.setPermissionMode',{sessionId:ids.codex,mode:'invalid-mode'})).error);
 for (const kind of ['config-modes', 'legacy-modes']) {
  const project=join(dir,kind),root=join(dir,kind+'-native');await mkdir(project);
  const workspace=await api('hive.request',{method:'POST',path:'/api/workspaces',body:{path:project,name:kind,autostart_orchestrator:false}});
  const {profile}=await api('agent.profiles.create',{driver:'acp',label:kind,launch:{command:process.execPath,args:[acp,root,kind]},expectedStoreVersion:(await api('agent.profiles.list')).storeVersion});
  const {id:sessionId}=await api('chat.createRecord',{workspaceId:workspace.id,backend:'acp',profile:profile.ref});
  assert.equal((await api('chat.mode',{sessionId})).connectAvailable,true);
  assert.equal((await api('agent.capabilities',{sessionId})).features.nativeModes.state,'unknown');
  await assert.rejects(readFile(join(root,'protocol.jsonl')),{code:'ENOENT'});
  assert.ok((await session.rpc('chat.setMode',{sessionId,mode:'invalid-mode'})).error);
  await api('chat.acp.prepareModes',{sessionId});
  assert.equal((await api('agent.capabilities',{sessionId})).features.nativeModes.state,'supported');
  const nativeId=(await api('chat.open',{sessionId})).acpSessionId;
  const initial=await api('chat.mode',{sessionId});
  const isConfig=kind==='config-modes';
  const mode=modeOption(initial,isConfig?'Peer Plan':'Peer Write');
  assert.equal((await api('chat.setMode',{sessionId,mode})).applied,mode);
  const access=await api('chat.permissionMode',{sessionId});
  const permission=isConfig?modeOption(access,'Peer Write'):undefined;
  if(permission) {
   assert.ok((await session.rpc('chat.setPermissionMode',{sessionId,mode})).error,'a collaboration option cannot mutate the permission axis');
   assert.equal((await api('chat.setPermissionMode',{sessionId,mode:permission})).applied,permission);
   assert.equal((await api('chat.mode',{sessionId})).applied,mode,'the permission write preserves collaboration mode');
  } else assert.deepEqual(access.options,[]);
  assert.ok((await session.rpc('chat.setMode',{sessionId,mode:'invalid-mode'})).error);
  acpRuns.set(kind,{sessionId,root,nativeId,mode,...(permission&&{permission})});
 }
}finally{await session.stop();}
session=start();try{
 assert.ok((await session.rpc('initialize')).result.hivePort);
 assert.equal((await session.rpc('chat.mode',{sessionId:ids.codex})).result.selected,'native-plan');
 assert.equal((await session.rpc('chat.permissionMode',{sessionId:ids.codex})).result.selected,'workspace-write');
 assert.equal((await session.rpc('chat.mode',{sessionId:ids.claude})).result.selected,'acceptEdits');
 for (const [kind,{sessionId,root,nativeId,mode,permission}] of acpRuns) {
  const protocol = async () => (await readFile(join(root,'protocol.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
  const counts = () => protocol().then(rows=>rows.filter(row=>['session/set_mode','session/set_config_option'].includes(row.method)).length);
  const before=await counts();
  assert.equal(before,permission?2:1);
  assert.equal((await api('chat.mode',{sessionId})).connectAvailable,true);
  await api('chat.acp.prepareModes',{sessionId});
  assert.equal((await api('chat.open',{sessionId})).acpSessionId,nativeId);
  assert.equal((await api('chat.mode',{sessionId})).applied,mode);
  assert.equal((await api('chat.permissionMode',{sessionId})).applied,permission??null);
  assert.equal(await counts(),before,'cold load reads native mode without replaying a setter');
  let rows=await protocol();assert.ok(rows.some(row=>row.method==='session/load'&&row.sessionId===nativeId));
  if (kind==='config-modes') {
   assert.ok(rows.some(row=>row.method==='session/set_config_option'&&row.configId==='mode'&&row.value==='write'));
   assert.ok(rows.some(row=>row.method==='session/set_config_option'&&row.configId==='collaboration_mode'&&row.value==='plan'));
   const execute=modeOption(await api('chat.mode',{sessionId}),'Peer Build');
   await writeFile(join(root,'reject-mode'),'');
   assert.ok((await session.rpc('chat.setMode',{sessionId,mode:execute})).error,'a valid-looking but mismatched reply cannot apply a mode');
   await unlink(join(root,'reject-mode'));
   await api('chat.acp.prepareModes',{sessionId});
   assert.equal((await api('chat.mode',{sessionId})).applied,mode);
   assert.equal((await api('chat.setMode',{sessionId,mode:execute})).applied,execute);
   await writeFile(join(root,'agent-mode-update'),'');
   assert.equal((await api('chat.setMode',{sessionId,mode})).applied,execute,'an authoritative update after the ACK supersedes it');
   assert.equal((await api('chat.mode',{sessionId})).applied,execute);
   assert.notEqual((await api('chat.open',{sessionId})).passive,true);
   assert.ok(session.agent.some(event=>event.type==='session_mode_changed'&&event.sessionId===sessionId));
   rows=await protocol();
  } else assert.ok(rows.some(row=>row.method==='session/set_mode'&&row.modeId==='write'));
  assert.ok(!rows.some(row=>row.method==='session/prompt'),'connecting solely for mode settings must never send a prompt');
 }
 assert.ok(!session.agent.some(event=>event.type==='assistant_delta'||event.type==='tool_start'));
}finally{await session.stop();}
const parse=(text:string)=>text.trim().split('\n').map(line=>JSON.parse(line));
const codexWire=parse(await readFile(join(dir,'modes-wire.jsonl'),'utf8')),claudeWire=parse(await readFile(wire,'utf8'));
assert.equal(codexWire.filter(x=>x.method==='turn/start').length,0);assert.equal(claudeWire.filter(x=>x.event==='query').length,0);
assert.ok(claudeWire.some(x=>x.event==='set_permission_mode'&&x.request.mode==='acceptEdits'));
console.log('SESSION_MODES_WIRE_OK: production RPCs, synthetic peers, cold persistence and invalid mode rejection; no inference.');
