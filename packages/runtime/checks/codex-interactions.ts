import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { CodexInteraction, PiSessionHandle } from '../src/agents/types.js';
import { CodexInteractionError } from '../src/agents/codex-interactions.js';

// Real subprocess JSONL plus Hive HTTP/SQLite. Synthetic protocol server, no
// native model calls; this does not claim native approval execution coverage.
const directory = await mkdtemp(resolve('.artifacts/codex-interactions-'));
const fixture = String.raw`
const {createInterface}=require('node:readline');
const {existsSync,appendFileSync,readFileSync}=require('node:fs');
const {createHash}=require('node:crypto');
const thread={id:require('node:path').basename(process.cwd()),path:null,historyMode:'legacy',turns:[]};
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
let mode,turn,cancelled=false,answered=false;
const end=()=>{turn.status='interrupted';notify('turn/completed',{turn});};
const resolved=()=>{cancelled=true;notify('serverRequest/resolved',{requestId:7});};
const timer=setInterval(()=>{
 if(!turn||turn.status!=='inProgress')return;
 if(existsSync('exit'))process.exit(7);
 if(existsSync('native-resolve')){resolved();end();}
 else if(existsSync('turn-end'))end();
 else if(answered&&existsSync('finish')){resolved();end();}
},10);
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line);
 if(!q.method){
  appendFileSync('responses.jsonl',JSON.stringify({id:q.id,late:cancelled,hash:createHash('sha256').update(JSON.stringify(q.result===undefined?{error:q.error}:q.result)).digest('hex'),...(mode!=='question'&&{result:q.result,error:q.error})})+'\n');answered=true;continue;
 }
 appendFileSync('control.jsonl',JSON.stringify({method:q.method})+'\n');
 const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='thread/settings/update')answer({});
 else if(q.method==='turn/interrupt'){answer({});if(mode!=='unknown'&&mode!=='malformed')resolved();end();}
 else if(q.method==='turn/start'){
  mode=q.params.input[0].text;turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});
  const params={threadId:thread.id,turnId:turn.id,itemId:'item',startedAtMs:1};
  let method='item/commandExecution/requestApproval';
  if(mode==='question'){method='item/tool/requestUserInput';delete params.startedAtMs;params.isBlocking=true;params.questions=[{id:'secret',header:'Secret',question:'Enter fixture value',isSecret:true,isOther:true,options:null}];}
  else if(mode==='file'||mode==='file-missing'){
   method='item/fileChange/requestApproval';
   if(mode==='file')notify('item/started',{turnId:turn.id,item:{id:'item',type:'fileChange',status:'inProgress',changes:[{path:'/fixture/a',kind:{type:'update',move_path:null},diff:'-old\n+new'}]}});
  }else {params.command='printf fixture';params.cwd=process.cwd();params.availableDecisions=mode==='limited'?['decline','cancel']:['accept','decline','cancel'];}
  if(mode==='malformed')delete params.startedAtMs;
  if(mode==='unknown')method='item/unknown/requestApproval';
  send({id:7,method,params});
 }
}clearInterval(timer);})().catch(e=>{console.error(e);process.exit(1)});
`;
const events: unknown[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event), { executableEnvironment: {} });
const live = () => (runtime.chats as unknown as { live: Map<string, { handle: PiSessionHandle }> }).live;
const wait = async <T>(fn: () => T | Promise<T>, predicate: (value: T) => boolean): Promise<T> => {
  const deadline = Date.now() + 6000;
  for (;;) { const value = await fn(); if (predicate(value)) return value;
    if (Date.now() > deadline) throw new Error('Fixture condition timed out');
    await new Promise(resolve => setTimeout(resolve, 10)); }
};
const stale = (fn: () => Promise<unknown>) => assert.rejects(fn, (error: unknown) => error instanceof CodexInteractionError && error.code === 'INTERACTION_STALE');
const invalid = (fn: () => Promise<unknown>) => assert.rejects(fn, (error: unknown) => error instanceof CodexInteractionError && error.code === 'INTERACTION_INVALID_RESPONSE');
let count = 0;
async function start(mode: string) {
  const cwd = join(directory, `session-${++count}`); await mkdir(cwd);
  await writeFile(join(cwd, 'package.json'), '{"type":"commonjs"}'); await writeFile(join(cwd, 'app-server'), fixture);
  const ws = await runtime.hive.request('POST', '/api/workspaces', { path: cwd, name: mode, autostart_orchestrator: false }) as { id: string };
  const record = await runtime.chats.create(ws.id, 'codex');
  const handle = live().get(record.id)!.handle;
  const done = handle.prompt(mode); void done.catch(() => {});
  if (mode === 'unknown' || mode === 'malformed') return { cwd, record, handle, done, card: undefined as unknown as CodexInteraction };
  const snapshot = await wait(() => runtime.chats.open(record.id), value => value.interactions.length === 1);
  const card = snapshot.interactions[0]!;
  return { cwd, record, handle, done, card };
}
async function responses(cwd: string) {
  return wait(async () => { try { return (await readFile(join(cwd, 'responses.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line)); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return []; throw error; } }, value => value.length > 0);
}
async function finish(run: Awaited<ReturnType<typeof start>>) {
  await responses(run.cwd); assert.equal((await runtime.chats.open(run.record.id)).interactions[0]?.state, 'responding');
  await stale(() => runtime.chats.respondInteraction(run.record.id, run.card.id, { decision: 'accept' }));
  await writeFile(join(run.cwd, 'finish'), ''); await run.done;
  assert.deepEqual((await runtime.chats.open(run.record.id)).interactions, []);
  await stale(() => runtime.chats.respondInteraction(run.record.id, run.card.id, { decision: 'accept' }));
}
try {
  await runtime.settings.update({ piCommand: null, codexCommand: process.execPath });
  const a = await start('command'), b = await start('limited');
  assert.equal(a.card.nativeRequestId, b.card.nativeRequestId); assert.notEqual(a.card.id, b.card.id);
  await stale(() => runtime.chats.respondInteraction(b.record.id, a.card.id, { decision: 'accept' }));
  const before = live().size; await stale(() => runtime.chats.respondInteraction('missing', a.card.id, { decision: 'accept' })); assert.equal(live().size, before);
  await invalid(() => runtime.chats.respondInteraction(b.record.id, b.card.id, { decision: 'accept' }));
  await invalid(() => runtime.chats.respondInteraction(a.record.id, a.card.id, { decision: 'acceptForSession' }));
  await runtime.chats.respondInteraction(a.record.id, a.card.id, { decision: 'accept' });
  await runtime.chats.respondInteraction(b.record.id, b.card.id, { decision: 'decline' });
  await finish(a); await finish(b);
  assert.deepEqual((await responses(a.cwd))[0].result, { decision: 'accept' });
  assert.deepEqual((await responses(b.cwd))[0].result, { decision: 'decline' });
  for (const mode of ['file', 'file-missing', 'question', 'command']) {
    const run = await start(mode);
    let response: unknown = { decision: 'cancel' };
    if (mode === 'file') { assert.equal((run.card.item as { changes: { diff: string }[] }).changes[0]!.diff, '-old\n+new'); response = { decision: 'accept' }; }
    if (mode === 'file-missing') await invalid(() => runtime.chats.respondInteraction(run.record.id, run.card.id, { decision: 'accept' }));
    if (mode === 'question') {
      await invalid(() => runtime.chats.respondInteraction(run.record.id, run.card.id, { answers: { unknown: { answers: ['bad'] } } }));
      response = { answers: { secret: { answers: ['SECRET-SENTINEL-ONLY-NATIVE'] } } };
    }
    await runtime.chats.respondInteraction(run.record.id, run.card.id, response); await finish(run);
    assert.equal((await responses(run.cwd))[0].hash, createHash('sha256').update(JSON.stringify(response)).digest('hex'));
    assert.equal(JSON.stringify(await runtime.chats.open(run.record.id)).includes('SECRET-SENTINEL-ONLY-NATIVE'), false);
  }
  for (const mode of ['native-resolve', 'turn-end', 'abort', 'exit']) {
    const run = await start('command');
    if (mode === 'abort') await run.handle.abort(); else await writeFile(join(run.cwd, mode), '');
    if (mode === 'exit') await assert.rejects(run.done); else await run.done;
    assert.deepEqual(run.handle.interactions?.(), []);
    await stale(() => runtime.chats.respondInteraction(run.record.id, run.card.id, { decision: 'accept' }));
    await assert.rejects(readFile(join(run.cwd, 'responses.jsonl')), (e: NodeJS.ErrnoException) => e.code === 'ENOENT');
  }
  const racing = await start('command');
  const racingAnswer = runtime.chats.respondInteraction(racing.record.id, racing.card.id, { decision: 'accept' });
  await racing.handle.abort(); await racingAnswer; await racing.done;
  await assert.rejects(readFile(join(racing.cwd, 'responses.jsonl')), (e: NodeJS.ErrnoException) => e.code === 'ENOENT');
  for (const mode of ['unknown', 'malformed']) {
    const unsupported = await start(mode); await assert.rejects(unsupported.done);
    assert.deepEqual(unsupported.handle.interactions?.(), []);
    const rejected = await responses(unsupported.cwd);
    assert.equal(rejected[0].error.code, -32603);
    assert.equal(rejected[0].late, false);
    const control = (await readFile(join(unsupported.cwd, 'control.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
    assert.ok(control.some(entry => entry.method === 'turn/interrupt'));
    await unsupported.handle.setModel('openai', 'gpt-6.1-terra'); // RPC control proves fixture survived the rejection; no inference.
  }
  assert.equal(JSON.stringify(events).includes('SECRET-SENTINEL-ONLY-NATIVE'), false);
  assert.ok(events.some(event => (event as { type?: string }).type === 'interaction'));
  assert.ok(events.some(event => { const e = event as { type: string; sessionId: string; interaction?: CodexInteraction };
    return e.type === 'interaction' && e.sessionId === a.record.id && e.interaction?.id === a.card.id; }));
  assert.equal((await runtime.chats.open(a.record.id)).eventSequence, Math.max(...events.filter(event => (event as { sessionId: string }).sessionId === a.record.id).map(event => (event as { sequence: number }).sequence)));
  const sequenced = events as { sessionId: string; sequence: number }[];
  for (const id of new Set(sequenced.map(event => event.sessionId))) {
    const sequence = sequenced.filter(event => event.sessionId === id).map(event => event.sequence);
    assert.deepEqual(sequence, sequence.map((_, index) => index + 1));
  }
  async function inspectSaved(path: string): Promise<void> {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = join(path, entry.name);
      if (entry.isDirectory()) await inspectSaved(child);
      else if (entry.isFile()) assert.equal((await readFile(child)).includes(Buffer.from('SECRET-SENTINEL-ONLY-NATIVE')), false, `Secret persisted in ${child}`);
    }
  }
  await inspectSaved(join(directory, 'state'));
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ synthetic: true, noModelRequests: true, sameNativeIdIsolated: true,
    snapshotRespondingAndResolved: true, nativeCancellationAbortTerminalAndDeath: true, noLateReplies: true, secretNativeOnly: true, sessions: count }, null, 2));
  console.log(`CODEX_INTERACTIONS_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: String(error), stack: (error as Error).stack }, null, 2)); throw error;
} finally { await runtime.close(); }
