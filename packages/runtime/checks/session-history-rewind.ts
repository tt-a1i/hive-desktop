import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, realpath, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { setTimeout as delay } from 'node:timers/promises';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { ChatIndex } from '../src/chat-index.js';
import { CodexProjection } from '../src/agents/codex-projection.js';
import { readCodexMetadata, writeCodexMetadata } from '../src/agents/codex-metadata.js';

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-rewind-wire-')));
const project = join(root, 'project'), pi = join(root, 'pi'), data = join(root, 'state');
const log = join(root, 'requests.jsonl'), native = join(root, 'native.json');
const dropResponse = join(root, 'lost-revert-reply'), dropBefore = join(root, 'drop-before');
const runtime = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const peer = fileURLToPath(new URL('./fixtures/session-history-peer.cjs', import.meta.url));
const sidecarCrash = process.argv.includes('--sidecar-crash');
if (sidecarCrash && process.platform !== 'darwin') {
  throw new Error('--sidecar-crash is supported by this check on macOS only');
}
let execution = project;
const git = (cwd: string, ...args: string[]) => execFileSync('git', ['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args], {cwd, encoding:'utf8', env:{...process.env, GIT_OPTIONAL_LOCKS:'0'}}).trim();
const guard = `const fetch=globalThis.fetch;globalThis.fetch=(input,init)=>{const url=new URL(typeof input==='string'?input:input instanceof URL?input:input.url);if(!['127.0.0.1','localhost','::1'].includes(url.hostname))return Promise.reject(Error('fixture prohibits remote access'));return fetch(input,init);};`;
function start() {
  const child = spawn(process.execPath, ['--import',`data:text/javascript,${encodeURIComponent(guard)}`,runtime], {env:{...process.env,
    HIVE_DESKTOP_DATA_DIR:data, PI_CODING_AGENT_DIR:pi, PI_OFFLINE:'1', GIT_CONFIG_GLOBAL:join(root,'gitconfig'), GIT_CONFIG_NOSYSTEM:'1',
    HIVE_HISTORY_STATE:native, HIVE_HISTORY_LOG:log, HIVE_HISTORY_SOURCE:execution, HIVE_HISTORY_DROP:join(root,'drop-fork'),
    HIVE_HISTORY_DROP_REVERT:dropResponse, HIVE_HISTORY_DROP_BEFORE_REVERT:dropBefore,
  },stdio:['pipe','pipe','pipe']});
  const exited=once(child,'exit'), lines=createInterface({input:child.stdout});
  let id=0, stderr='';
  child.stderr.on('data',bytes=>stderr+=String(bytes));
  const pending=new Map<number,{resolve(value:any):void;reject(error:Error):void}>();
  lines.on('line',line=>{const response=JSON.parse(line), waiter=pending.get(response.id);if(waiter){pending.delete(response.id);waiter.resolve(response);}});
  child.on('exit',()=>{for(const waiter of pending.values())waiter.reject(Error('Runner exited: '+stderr));pending.clear();});
  const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<any>((resolve,reject)=>{
    const next=++id, timer=setTimeout(()=>{pending.delete(next);reject(Error(`RPC expired: ${method}: ${stderr}`));},90000);
    pending.set(next,{resolve:result=>{clearTimeout(timer);if(result.error)reject(Error(result.error.message));else resolve(result.result);},reject:error=>{clearTimeout(timer);reject(error);}});
    child.stdin.write(JSON.stringify({id:next,method,params})+'\n');
  });
  return {
    rpc,
    kill: async () => {
      child.kill('SIGKILL');
      assert.deepEqual(await exited, [null, 'SIGKILL']);
      lines.close();
    },
    close:async()=>{if(child.exitCode===null){try{await rpc('shutdown');}catch(error){child.kill();throw error;}finally{await exited;}}lines.close();assert.equal(child.exitCode,0,stderr);},
    async loseOutput() {
      child.stdout.destroy();
      child.stdin.write(JSON.stringify({id:++id,method:'chat.list',params:{workspaceId:'test'}})+'\n');
      let timeout: ReturnType<typeof setTimeout> | undefined;
      try {
        assert.deepEqual(await Promise.race([exited, new Promise((_, reject) => {
          timeout = setTimeout(() => reject(Error('Sidecar kept running with the desktop output pipe closed')), 15_000);
        })]), [0,null], stderr);
      } finally {
        clearTimeout(timeout);
        if (child.exitCode === null) child.kill();
      }
    },
  };
}
let wire: ReturnType<typeof start> | undefined;
const calls = async () => (await readFile(log,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
const state = async () => JSON.parse(await readFile(native,'utf8'));

try {
  await mkdir(project); await mkdir(pi); await writeFile(join(root,'gitconfig'),'');
  git(project,'init','--template=');
  await writeFile(join(project,'tracked.txt'),'original root');
  git(project,'add','.');git(project,'commit','-m','Owned test project');
  wire=start();await wire.rpc('initialize');
  const workspace=await wire.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:project,name:'Rewind wire test',autostart_orchestrator:false}});
  const {storeVersion}=await wire.rpc('agent.profiles.list');
  const {profile}=await wire.rpc('agent.profiles.create',{driver:'codex',label:'Owned history peer', launch:{command:process.execPath,args:[peer]},expectedStoreVersion:storeVersion});
  const chat=await wire.rpc('chat.createRecord',{workspaceId:workspace.id,backend:'codex',profile:profile.ref,isolation:'worktree'});
  execution=chat.execution.path;
  const base={workspaceId:workspace.id,sessionId:chat.id};
  await wire.close();wire=undefined;
  const navigation=new ChatIndex(join(data,'chats'));
  navigation.save({...navigation.get(chat.id),codexThreadId:'source',originAgentId:`${workspace.id}:orchestrator`,identityState:'known'});
  const metadata=join(data,'chats','codex');
  writeCodexMetadata(metadata,'source',{version:1,context:[],deliveries:[],completed:['hive-delivered-before-rewind']});
  wire=start();await wire.rpc('initialize');
  assert.equal((await wire.rpc('history.capabilities',base)).rewindCode,true);
  const target=await wire.rpc('checkpoint.preview',base);
  const {checkpointId}=await wire.rpc('checkpoint.capture',{...base,expectedVersion:target.version,requestId:randomUUID()});
  await writeFile(join(execution,'tracked.txt'),'future index');git(execution,'add','tracked.txt');
  await writeFile(join(execution,'tracked.txt'),'future file');
  const selection={...base,boundary:{nativeUserItemId:'u2'},code:{kind:'checkpoint',checkpointId}};
  const preview=await wire.rpc('history.rewind.preview',selection);
  assert.equal(preview.removedTurns,1);
  const request={...base,previewToken:preview.previewToken,requestId:randomUUID()};
  await assert.rejects(wire.rpc('history.rewind.apply',request),/Explicit confirmation/);
  assert.ok(!(await calls()).some(call=>call.method==='thread/fork'||call.method==='thread/revert'));

  const gitDir = git(execution, 'rev-parse', '--absolute-git-dir');
  const indexLock = join(gitDir, 'index.lock');
  await writeFile(indexLock, 'owned by another Git process', {flag:'wx'});
  const locked = await wire.rpc('history.rewind.apply',{...request,requestId:randomUUID(),confirmed:true});
  assert.equal(locked.state,'failed',JSON.stringify(locked));
  assert.match(locked.error,/locked by another operation/);
  assert.equal(await readFile(indexLock,'utf8'),'owned by another Git process');
  assert.equal(await readFile(join(execution,'tracked.txt'),'utf8'),'future file');
  await assert.rejects(state(),{code:'ENOENT'}); // The peer writes state only after a native mutation.
  assert.ok(!(await calls()).some(call=>call.method==='thread/fork'||call.method==='thread/revert'));
  assert.equal((await wire.rpc('history.capabilities',base)).rewind,true);
  await unlink(indexLock);
  const fresh = await wire.rpc('history.rewind.preview',selection);
  request.previewToken = fresh.previewToken;

  let operationId: string | undefined;
  if (sidecarCrash) {
    // Pause after persistence but before the native reply, then kill the sidecar.
    await writeFile(dropResponse, 'pause');
    const interrupted = wire.rpc('history.rewind.apply', {...request, confirmed:true})
      .then(() => { throw Error('Unexpected rewind response'); }, () => {});
    let peerPid = 0;
    for (let i = 0; i < 300; i++) {
      try {
        peerPid = Number(await readFile(dropResponse + '.ready', 'utf8'));
        break;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
        await delay(100);
      }
    }
    assert.ok(peerPid, 'Peer did not persist and pause');
    assert.deepEqual((await state()).source.turns.map((turn:any)=>turn.id), ['t1']);
    await wire.kill();
    await interrupted;
    wire = undefined;
    for (let i = 0; i < 100; i++) {
      try {
        process.kill(peerPid, 0);
        await delay(100);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ESRCH') break;
        throw error;
      }
    }
    assert.throws(() => process.kill(peerPid, 0), {code:'ESRCH'});
  } else {
    // The peer dies before replying; the still-running sidecar reports uncertainty.
    await writeFile(dropResponse, 'yes');
    const uncertain = await wire.rpc('history.rewind.apply', {...request, confirmed:true});
    assert.equal(uncertain.state, 'uncertain', JSON.stringify(uncertain));
    assert.equal(uncertain.recoverable, true);
    operationId = uncertain.operationId;
    assert.equal((await wire.rpc('chat.open', {sessionId:chat.id})).identityError.code, 'HISTORY_REWIND_PENDING');
    await wire.close();
    wire = undefined;
  }
  assert.equal(await readFile(join(execution,'tracked.txt'),'utf8'),'future file');
  assert.deepEqual((await state()).source.turns.map((turn:any)=>turn.id),['t1']);
  assert.equal((await state()).source.thread.path,'/native/source-rewound');
  assert.deepEqual((await state()).child1.turns.map((turn:any)=>turn.id),['t1','t2']);
  const backupRequestTypes=(await calls()).filter(call=>['thread/fork','thread/revert'].includes(call.method)).map(call=>call.method);
  assert.deepEqual(backupRequestTypes,['thread/fork','thread/revert']);

  await unlink(dropResponse);
  wire=start();await wire.rpc('initialize');
  const operations=await wire.rpc('history.operations.list',{workspaceId:workspace.id});
  const pending=operations.operations.find((op:any)=>op.sourceSessionId===chat.id && op.recoverable);
  assert.ok(pending,JSON.stringify(operations));
  assert.notEqual(pending.state,'ready');
  if (operationId) assert.equal(pending.operationId, operationId);
  assert.equal((await wire.rpc('chat.open',{sessionId:chat.id})).identityError.code,'HISTORY_REWIND_PENDING');
  assert.equal(await readFile(join(execution,'tracked.txt'),'utf8'),'future file');
  assert.deepEqual((await calls()).filter(call=>['thread/fork','thread/revert'].includes(call.method)).map(call=>call.method),backupRequestTypes);
  const recovered=await wire.rpc('history.operations.recover',{workspaceId:workspace.id,operationId:pending.operationId});
  assert.equal(recovered.state,'ready',JSON.stringify(recovered));
  assert.equal(recovered.session.id,chat.id);
  assert.notEqual(recovered.backupSession.id,chat.id);
  assert.equal(await readFile(join(execution,'tracked.txt'),'utf8'),'original root');
  assert.equal(git(execution,'show',':tracked.txt'),'original root');
  assert.equal(await readFile(join(recovered.backupSession.executionPath,'tracked.txt'),'utf8'),'future file');
  assert.equal(git(recovered.backupSession.executionPath,'show',':tracked.txt'),'future index');
  assert.equal((await wire.rpc('chat.open',{sessionId:recovered.backupSession.id})).history.length,2);
  assert.equal(new CodexProjection(metadata,'source').current().length,1);
  assert.deepEqual(readCodexMetadata(metadata,'source').completed,['hive-delivered-before-rewind']);
  assert.equal((await wire.rpc('history.rewind.apply',{...request,confirmed:true})).backupSession.id,recovered.backupSession.id);
  assert.deepEqual((await calls()).filter(call=>['thread/fork','thread/revert'].includes(call.method)).map(call=>call.method),backupRequestTypes);

  const aborted=await wire.rpc('history.rewind.preview',{...base,boundary:{nativeUserItemId:'u1'},code:{kind:'current'}});
  await writeFile(dropBefore,'yes');
  const notApplied=await wire.rpc('history.rewind.apply',{...base,previewToken:aborted.previewToken,requestId:randomUUID(),confirmed:true});
  assert.equal(notApplied.state,'uncertain',JSON.stringify(notApplied));
  const settled=await wire.rpc('history.operations.recover',{workspaceId:workspace.id,operationId:notApplied.operationId});
  assert.equal(settled.state,'failed',JSON.stringify(settled));
  assert.deepEqual((await state()).source.turns.map((turn:any)=>turn.id),['t1']);
  assert.equal((await wire.rpc('history.capabilities',base)).rewind,true);
  assert.ok(!(await calls()).some(call=>['turn/start','turn/steer','account/read'].includes(call.method)));
  assert.equal((await calls()).filter(call=>call.method==='thread/revert').length,2);
  assert.equal(await readFile(join(project,'tracked.txt'),'utf8'),'original root');
  await wire.loseOutput();
  wire=undefined;
  console.log((sidecarCrash ? 'SESSION_HISTORY_REWIND_SIDECAR_CRASH_OK:' : 'SESSION_HISTORY_REWIND_OK:') + ' backup first, exact native prefix, code restore, cold recovery and original delivery receipts');
} finally {
  try { await wire?.close(); } finally { await rm(root,{recursive:true,force:true,maxRetries:5}); }
}
