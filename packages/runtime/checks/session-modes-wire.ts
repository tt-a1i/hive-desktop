/** Production JSONL RPCs with synthetic local Codex and Claude peers; no inference. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {once} from 'node:events';
import {mkdtemp,mkdir,readFile,rename,rm,unlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import {setTimeout as delay} from 'node:timers/promises';
import {ChatIndex} from '../src/chat-index.js';
import {CodexProjection} from '../src/agents/codex-projection.js';
import {codexMetadataFile} from '../src/agents/codex-metadata.js';
import {DeliveryQueue} from '../src/delivery-queue.js';
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
let codexWorkspace: string;
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
 codexWorkspace=ws.id;
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

const sessionId = ids.codex!;
const workspaceId = codexWorkspace!;
const indexDirectory = join(data, 'chats');
const indexPath = join(indexDirectory, 'index.json');
const index = () => new ChatIndex(indexDirectory);
const savedRecord = index().get(sessionId);
const nativeId = 'modes-thread';
index().save({...savedRecord, codexThreadId: nativeId, identityState:'known', originAgentId:`${workspaceId}:orchestrator`});
const history = {id:'kept-turn', status:'completed', items:[{id:'kept-item', type:'agentMessage', text:'Previously saved Codex reply'}]};
const nativeDirectory = join(indexDirectory, 'codex');
await mkdir(nativeDirectory, {recursive: true});
new CodexProjection(nativeDirectory, nativeId).recover([history], true);
const projectionFile = codexMetadataFile(nativeDirectory,nativeId)+'.display.json';
const originalProjection = await readFile(projectionFile);
const queues = join(indexDirectory, 'deliveries');
const spool = join(queues, createHash('sha256').update(sessionId).digest('hex')+'.json');
const queued = 'A queued request awaiting explicit continuation';
await new DeliveryQueue({directory:queues,sessionId,startPaused:true,completedIds:()=>[],
 execute:async()=>{throw new Error('Unexpected offline delivery')}, onError:assert.fail,
}).enqueue(queued,true,{version:1,origin:'user',displayText:queued});
const originalSpool = await readFile(spool);
const fixture = async (value: Record<string, unknown>) => writeFile(join(dir,'modes-control.json'), JSON.stringify({turns:[history],...value}));
const wireRows = async () => parse(await readFile(join(dir,'modes-wire.jsonl'),'utf8'));
async function waitFor<T>(read: () => Promise<T | undefined>) {
 const deadline = Date.now()+5000;
 for (;;) {
  const value = await read();
  if (value !== undefined) return value;
  assert.ok(Date.now()<deadline,'Timed out waiting for the Codex protocol event');
  await delay(25);
 }
}
const failure = async (method: string, params: Record<string, unknown>, match: RegExp) => {
 const response = await session.rpc(method,params);
 assert.match(response.error?.message??'',match);
};
const preview = 'chat.codexModes.previewRecovery', apply = 'chat.codexModes.applyRecovery';
const replacements = {mode:'native-default',permissionMode:'read-only'};
const recoverable = {modes:[{mode:'default'}],requirements:{allowedSandboxModes:['read-only']}};
await fixture(recoverable);
const recoveryStart = (await wireRows()).length;
session = start();
try {
 await api('initialize');
 const open = (viewRevision: number) => api('chat.open',{sessionId,viewRevision});
 const scope = {workspaceId,sessionId,viewRevision:1};
 const readOnly = await open(1);
 assert.equal(readOnly.readOnly,true);
 assert.equal(readOnly.identityError.code,'CODEX_SAVED_MODE_UNAVAILABLE');
 assert.notEqual(readOnly.historyIncomplete,true);
 assert.deepEqual(readOnly.historyMessageIds,['kept-item']);
 assert.match(JSON.stringify(readOnly.history),/Previously saved Codex reply/);
 assert.equal(readOnly.pendingUserInputs[0].text,queued);
 await failure('chat.resume',{sessionId},/Unsupported Codex collaboration mode/);
 await fixture({hangRequirements:true,hangMode:true});
 const timedBefore = (await wireRows()).length;
 const timedStart = Date.now();
 const stalledProbe = failure(preview,scope,/Could not verify the saved modes/);
 await waitFor(async () => (await wireRows()).slice(timedBefore).find(row=>row.method==='configRequirements/read'));
 await stalledProbe;
 const elapsed = Date.now()-timedStart;
 assert.ok(elapsed>=9000 && elapsed<14000, `Unexpected probe bound ${elapsed}ms`);
 const timedRows = (await wireRows()).slice(timedBefore);
 const probePid = timedRows.findLast(row=>row.method==='collaborationMode/list').pid;
 if(process.platform!=='win32') assert.throws(()=>process.kill(probePid,0),{code:'ESRCH'});
 assert.deepEqual(await readFile(spool),originalSpool);
 assert.ok(!timedRows.some(row=>row.pid===probePid && (row.method==='turn/start'||row.method==='turn/steer')));
 console.log(`HANGING_CODEX_MODE_PROBE_BASELINE_MS=${elapsed}; process reaped; queue unchanged`);
 await fixture({exitRequirements:true});
 const exitBefore = (await wireRows()).length;
 const exitStart = Date.now();
 await failure(preview,scope,/Could not verify the saved modes/);
 assert.ok(Date.now()-exitStart<4000);
 const exitRows = (await wireRows()).slice(exitBefore);
 const exited = exitRows.find(row=>row.method==='configRequirements/read');
 assert.ok(exited);
 if(process.platform!=='win32') assert.throws(()=>process.kill(exited.pid,0),{code:'ESRCH'});
 assert.deepEqual(await readFile(spool),originalSpool);
 assert.ok(!exitRows.some(row=>row.method==='turn/start'||row.method==='turn/steer'));
 console.log(`DISCONNECTED_CODEX_MODE_PROBE_MS=${Date.now()-exitStart}; queue unchanged`);
 await fixture(recoverable);

 await failure(preview,{...scope,workspaceId:'another-workspace'},/no saved Codex mode/);
 await fixture({...recoverable,modes:[{unexpected:true}]});
 await failure(preview,scope,/Could not verify the saved modes/);
 await fixture({...recoverable,error:-32602});
 await failure(preview,scope,/Could not verify the saved modes/);
 await fixture({error:-32601});
 const inherit = await api(preview,scope);
 assert.equal(inherit.mode.canInherit,true);
 assert.equal(inherit.permissionMode,undefined);
 await fixture({requirements:{allowedSandboxModes:['read-only']}});
 const permissionsOnly = await api(preview,scope);
 assert.equal(permissionsOnly.mode,undefined);
 assert.equal(permissionsOnly.permissionMode.canInherit,false);
 await failure(apply,{...scope,sessionVersion:permissionsOnly.sessionVersion,choices:replacements},/only replace an unavailable/);
 await fixture(recoverable);
 const choices = await api(preview,scope);
 assert.deepEqual([choices.mode.saved,choices.permissionMode.saved],['native-plan','workspace-write']);
 assert.deepEqual([choices.mode.options[0].id,choices.permissionMode.options[0].id],Object.values(replacements));
 const change = {...scope,sessionVersion:choices.sessionVersion,choices:replacements};
 await failure(apply,{...change,choices:{mode:'native-default'}},/every unavailable saved mode/);
 await failure(apply,{...change,choices:{...replacements,permissionMode:null}},/permissionMode is required/);
 await rename(indexPath,indexPath+'.test-backup');
 await mkdir(indexPath);
 try { await failure(apply,change,/EISDIR|EPERM|EEXIST/); }
 finally { await rm(indexPath,{recursive:true}); await rename(indexPath+'.test-backup',indexPath); }
 await fixture({...recoverable,modeDelay:500});
 const before = (await wireRows()).length;
 const staleWrite = failure(apply,change,/Stale desktop conversation view/);
 await waitFor(async () => (await wireRows()).slice(before).find(row=>row.method==='collaborationMode/list'));
 await api('chat.clearView',{viewRevision:2});
 const reopenedDuringProbe = open(3);
 await staleWrite;
 assert.deepEqual(index().get(sessionId).nativeMode,savedRecord.nativeMode);
 assert.equal((await reopenedDuringProbe).readOnly,true);
 await fixture(recoverable);
 await api(apply,{...change,viewRevision:3});
 const {nativeMode,...unchanged} = index().get(sessionId);
 const {nativeMode:oldNativeMode,...expectedUnchanged} = {...savedRecord,codexThreadId:nativeId,
  identityState:'known',originAgentId:`${workspaceId}:orchestrator`};
 assert.deepEqual(unchanged,expectedUnchanged);
 assert.deepEqual(nativeMode,{backend:'codex',...replacements});
 assert.deepEqual(await readFile(spool),originalSpool);
 assert.deepEqual(await readFile(projectionFile),originalProjection);
 const recoveryWire = (await wireRows()).slice(recoveryStart);
 assert.ok(recoveryWire.every(row => ['initialize','model/list','configRequirements/read','collaborationMode/list'].includes(row.method)));
 if (process.platform !== 'win32') for (const pid of new Set<number>(recoveryWire.map(row=>row.pid)))
  assert.throws(()=>process.kill(pid,0),{code:'ESRCH'});
 const reopened = await open(4);
 assert.notEqual(reopened.readOnly,true);
 assert.equal(reopened.codexThreadId,nativeId);
 assert.equal(reopened.pendingUserInputs[0].text,queued);
 assert.deepEqual(reopened.historyMessageIds,['kept-item']);
 const reopenedWire = (await wireRows()).slice(recoveryStart);
 assert.ok(reopenedWire.some(row=>row.method==='thread/resume'&&row.params.threadId===nativeId&&row.params.sandbox==='read-only'));
 assert.ok(!reopenedWire.some(row=>row.method==='turn/start'||row.method==='turn/steer'));
 await api('chat.resume',{sessionId});
 const sent = await waitFor(async () => (await wireRows()).slice(recoveryStart).find(row=>row.method==='turn/start'));
 assert.equal(sent.params.threadId,nativeId);
 assert.equal(sent.params.collaborationMode.mode,'default');
 assert.equal(sent.params.sandboxPolicy.type,'readOnly');
 assert.ok(sent.params.input.some((item:{text?:string})=>item.text?.includes(queued)));
 await waitFor(async () => DeliveryQueue.persistedCount(queues,sessionId) === 0 ? true : undefined);
 console.log('CODEX_MODE_RECOVERY_WIRE_OK: retained historical item and paused input, rejected ambiguous and stale writes, closed probe processes, then resumed the same thread with the chosen modes only on explicit continuation.');
} finally {
 await session.stop();
}

await fixture({modes:[{mode:'plan'}]});
session = start();
try {
 await api('initialize');
 const viewRevision = 1;
 assert.equal((await api('chat.open',{sessionId,viewRevision})).readOnly,true);
 await fixture({modes:[{mode:'plan'}],modeDelay:30_000});
 const before = (await wireRows()).length;
 const pending = session.rpc(preview,{workspaceId,sessionId,viewRevision}).then(response=>response.error,error=>error);
 const probe = await waitFor(async () => (await wireRows()).slice(before).find(row=>row.method==='collaborationMode/list'));
 await Promise.race([
  session.stop(),
  delay(4_000,undefined,{ref:false}).then(()=>assert.fail('Shutdown waited for a stalled Codex capability request')),
 ]);
 assert.ok(await pending);
 if (process.platform !== 'win32') assert.throws(()=>process.kill(probe.pid,0),{code:'ESRCH'});
 console.log('CODEX_MODE_RECOVERY_SHUTDOWN_OK: closing the runtime stops an in-flight capability probe and reaps its process.');
} finally {
 await session.stop();
 await rm(dir,{recursive:true,force:true});
}
