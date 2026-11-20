/** Real Hive HTTP/SQLite, desktop index and production JSONL. No Agent or model starts. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, readdir, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createInterface } from 'node:readline';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { ChatIndex, type ChatBackend } from '../src/chat-index.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-session-pins-'));
const state = join(directory, 'state');
const file = join(state, 'chats/index.json');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let events = 0;
try {
  runtime = await createDesktopRuntime(state, () => { events++; });
  const projects = [];
  for (const name of ['Alpha', 'Beta']) {
    const path = join(directory, name); await mkdir(path);
    projects.push(await runtime.hive.request('POST', '/api/workspaces', { path, name, autostart_orchestrator: false }) as { id: string });
  }
  const alpha = projects[0]!.id; const beta = projects[1]!.id;
  const alphaPath = await realpath(join(directory, 'Alpha'));
  const records = [];
  for (const backend of ['pi', 'codex', 'pi-rpc', 'claude'] satisfies ChatBackend[]) {
    records.push(await runtime.chats.createRecord(alpha, backend));
  }
  const foreign = await runtime.chats.createRecord(beta, 'codex');
  for (const original of records) {
    const result = runtime.chats.setPinned(original.id, true);
    assert.deepEqual(result, { ...original, pinned: true }, 'Pin must preserve transcript identity and activity timestamp');
    assert.equal(runtime.chats.list(alpha)[0]!.id, original.id, 'A sole pin must sort before all unpinned sessions');
    const global = await runtime.chats.pinned();
    assert.equal(global.length, 1);
    assert.deepEqual(global[0], {session: result, workspaceName:'Alpha', workspacePath:alphaPath, title:original.title});
    assert.deepEqual(runtime.chats.setPinned(original.id, true), result);
    assert.deepEqual(runtime.chats.setPinned(original.id, false), { ...original, pinned: false });
    assert.deepEqual(await runtime.chats.pinned(), []);
  }
  const target = records[0]!;
  runtime.chats.setPinned(target.id, true);
  const second = records[1]!; runtime.chats.setPinned(second.id, true);
  assert.ok(runtime.chats.list(alpha).slice(0, 2).every(record => record.pinned));
  assert.ok(runtime.chats.list(alpha).slice(2).every(record => !record.pinned));
  runtime.chats.rename(target.id, '主控 🐝');
  assert.equal(runtime.chats.list(alpha).find(record => record.id === target.id)!.pinned, true);
  const before = runtime.chats.list(alpha);
  const bytes = await readFile(file, 'utf8');
  for (const invalid of [undefined, null, 'true', 'false', 0, 1, {}, []]) {
    assert.throws(() => runtime!.chats.setPinned(target.id, invalid), TypeError);
  }
  assert.throws(() => runtime!.chats.setPinned('missing', true));
  assert.equal(await readFile(file, 'utf8'), bytes);
  assert.deepEqual(runtime.chats.list(alpha), before);
  assert.deepEqual(runtime.chats.list(beta), [foreign]);
  // Real rename failure after the complete temporary write, no injected filesystem mock.
  await rename(file, file + '.retained'); await mkdir(file);
  try {
    assert.throws(() => runtime!.chats.setPinned(target.id, false));
    assert.deepEqual(runtime.chats.list(alpha), before, 'Failed disk commit must not change memory');
    assert.deepEqual((await readdir(join(state, 'chats'))).filter(name => name.endsWith('.tmp')), []);
  } finally { await rm(file, { recursive: true }); await rename(file + '.retained', file); }
  await runtime.close();
  assert.throws(() => runtime!.chats.setPinned(target.id, false));
  runtime = undefined;
  // Old records have no pinned field; persisted invalid values must not be silently accepted.
  const stored = JSON.parse(await readFile(file, 'utf8'));
  const legacy = stored.find((record: { id: string }) => record.id === foreign.id); delete legacy.pinned;
  await writeFile(file, JSON.stringify(stored));
  const index = new ChatIndex(join(state, 'chats'));
  assert.deepEqual(index.list(alpha), before);
  assert.equal(index.get(foreign.id).pinned, undefined);
  stored[0].pinned = 'yes'; await writeFile(file, JSON.stringify(stored));
  assert.throws(() => new ChatIndex(join(state, 'chats')));
  await writeFile(file, bytes);
  runtime = await createDesktopRuntime(state, () => { events++; });
  assert.deepEqual(runtime.chats.list(alpha), before);
  assert.deepEqual(runtime.chats.list(beta), [foreign]);
  assert.equal(events, 0, 'Metadata operations must not produce Agent events');
  assert.deepEqual(await readdir(join(state, 'chats/pi')), [], 'Metadata operations must not materialize Agent transcripts');
  await runtime.close(); runtime = undefined;

  const agentDir = join(directory, 'pi'); await mkdir(agentDir);
  const child = spawn(process.execPath, ['dist/main.js'], { env: { ...process.env, HIVE_DESKTOP_DATA_DIR: state, PI_CODING_AGENT_DIR: agentDir, PI_OFFLINE: '1' }, stdio: ['pipe', 'pipe', 'pipe'] });
  const exited = once(child, 'exit'); let stderr = ''; child.stderr.on('data', chunk => { stderr += String(chunk); });
  let sequence = 0; const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
  const lines = createInterface({ input: child.stdout }); let agentEvents = 0;
  lines.on('line', line => { const frame = JSON.parse(line); if (frame.event === 'agent') agentEvents++; const request = pending.get(frame.id); if (request) {pending.delete(frame.id);request.resolve(frame);} });
  child.on('exit', () => { for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr)); pending.clear(); });
  function rpc(method: string, params = {}) {
    return new Promise<any>((resolve, reject) => {
      const id = ++sequence;
      const timeout = setTimeout(() => {pending.delete(id); reject(new Error('RPC timeout: ' + method));}, 10000);
      pending.set(id, {resolve: value => {clearTimeout(timeout);resolve(value);}, reject: error => {clearTimeout(timeout);reject(error);}});
      child.stdin.write(JSON.stringify({id, method, params}) + '\n');
    });
  }
  const ref = (workspaceId: string, sessionId: string) => ({workspaceId, sessionId});
  async function globalPins(): Promise<Array<{session: {id: string; title: string; pinRank?: number}; workspaceName: string}>> {
    const reply = await rpc('chat.pinned');
    assert.equal(reply.error, undefined);
    return reply.result;
  }
  const orderedIds = async () => (await globalPins()).map(hit => hit.session.id);
  let expectedAfterRestart: string[] = [];
  try {
    assert.ok((await rpc('initialize')).result.hivePort);
    const pinned = await rpc('chat.pin', {sessionId: foreign.id, pinned: true});
    assert.equal(pinned.error, undefined); assert.deepEqual(pinned.result.session, {...foreign, pinned: true});
    assert.deepEqual(pinned.result.sessions, [{...foreign, pinned: true}]);
    const global = await rpc('chat.pinned');
    assert.equal(global.error, undefined);
    assert.equal(global.result.length, 3);
    assert.deepEqual(new Set(global.result.map((hit:any)=>hit.workspaceName)),new Set(['Alpha','Beta']));
    assert.deepEqual(new Set(global.result.map((hit:any)=>hit.session.id)),new Set([target.id,second.id,foreign.id]));
    assert.deepEqual((await rpc('chat.list', {workspaceId: alpha})).result, before);
    for (const invalid of [null, 'false', 0]) assert.ok((await rpc('chat.pin', {sessionId: foreign.id, pinned: invalid})).error);
    assert.ok((await rpc('chat.pin', {sessionId: 'missing', pinned: true})).error);

    // Equal conversation titles and pins spanning Alpha and Beta exercise
    // the persisted IDs rather than titles or any workspace's local sequence.
    const archived = records[2]!;
    for (const original of [target, second, foreign, archived]) {
      assert.equal((await rpc('chat.rename', {sessionId:original.id, title:'Same title'})).error, undefined);
    }
    assert.equal((await rpc('chat.pin', {sessionId:archived.id, pinned:true})).error, undefined);
    const legacyPins = await globalPins();
    assert.ok(legacyPins.every(({session}) => session.title === 'Same title' && !('pinRank' in session)));
    const legacyOrder = legacyPins.map(hit => hit.session.id);
    const archivedSlot = legacyOrder.indexOf(archived.id);
    const archivedReply = await rpc('chat.archive', {sessionId:archived.id, archived:true});
    assert.equal(archivedReply.error, undefined);
    const expectedOrder = legacyOrder.filter(id => id !== archived.id);
    const source = ref(beta, foreign.id), anchor = ref(alpha, target.id);
    const position = expectedOrder.indexOf(foreign.id) < expectedOrder.indexOf(target.id) ? 'after' : 'before';
    const move = {source, target:anchor, position, expectedOrder};
    const nextOrder = expectedOrder.filter(id => id !== foreign.id);
    nextOrder.splice(nextOrder.indexOf(target.id) + Number(position === 'after'), 0, foreign.id);
    const reordered = await rpc('chat.pins.move', move);
    assert.equal(reordered.error, undefined);
    assert.deepEqual(reordered.result, {orderedIds:nextOrder});
    assert.deepEqual(await orderedIds(), nextOrder);
    const ranks = (await globalPins()).map(hit => hit.session.pinRank);
    assert.ok(ranks.every(rank => rank !== undefined && Number.isSafeInteger(rank)));
    assert.deepEqual(ranks, ranks.slice().sort((a, b) => a! - b!));

    const committed = await readFile(file, 'utf8');
    assert.equal((await rpc('chat.pins.move', move)).error.code, 'PIN_ORDER_STALE');
    assert.equal((await rpc('chat.pins.move', {...move, source:ref(alpha, foreign.id), expectedOrder:nextOrder})).error.code, 'PIN_SOURCE_UNAVAILABLE');
    assert.equal((await rpc('chat.pins.move', {...move, target:ref(alpha, archived.id), expectedOrder:nextOrder})).error.code, 'PIN_SOURCE_UNAVAILABLE');
    assert.equal((await rpc('chat.pins.move', {...move, target:ref(alpha, 'missing'), expectedOrder:nextOrder})).error.code, 'PIN_SOURCE_UNAVAILABLE');
    assert.ok((await rpc('chat.pins.move', {...move, position:'above'})).error);
    assert.ok((await rpc('chat.pins.move', {...move, expectedOrder:[...nextOrder, nextOrder[0]]})).error);
    assert.equal(await readFile(file, 'utf8'), committed);
    assert.deepEqual(await orderedIds(), nextOrder);
    assert.equal(agentEvents, 1, 'Moving pins cannot publish agent events');

    const restored = await rpc('chat.archive', {sessionId:archived.id, archived:false,
      undo:{workspaceId:alpha, version:archivedReply.result.mutation.version}});
    assert.equal(restored.error, undefined, 'Freezing a hidden pin must preserve its Archive Undo receipt');
    const restoredOrder = nextOrder.slice();
    restoredOrder.splice(archivedSlot, 0, archived.id);
    assert.deepEqual(await orderedIds(), restoredOrder);
    assert.equal(restored.result.session.pinRank, archivedSlot);

    const another = records[3]!;
    const manualPin = await rpc('chat.pin', {sessionId: another.id, pinned: true});
    assert.equal(manualPin.error, undefined);
    assert.equal(manualPin.result.session.pinRank, -1);
    assert.deepEqual(await orderedIds(), [another.id, ...restoredOrder]);
    const unpinned = await rpc('chat.pin', {sessionId: target.id, pinned: false});
    assert.equal(unpinned.error, undefined);
    assert.equal(unpinned.result.session.pinned, false);
    assert.equal(unpinned.result.session.pinRank, undefined);
    assert.equal(unpinned.result.session.updatedAt, target.updatedAt);
    expectedAfterRestart = [another.id, ...restoredOrder.filter(id => id !== target.id)];
    assert.deepEqual(await orderedIds(), expectedAfterRestart);
    assert.equal(agentEvents, 2);
    await rpc('shutdown'); assert.equal((await exited)[0], 0, stderr);
  } finally { lines.close(); if (child.exitCode === null) {child.kill(); await exited;} }
  const finalIndex = new ChatIndex(join(state, 'chats'));
  assert.equal(finalIndex.get(target.id).pinned, false);
  assert.equal(finalIndex.get(foreign.id).pinned, true);
  const foreignRank = finalIndex.get(foreign.id).pinRank;
  finalIndex.save({...foreign, updatedAt: Date.now() + 500_000, title:'Late activity save'});
  assert.equal(finalIndex.get(foreign.id).pinned, true, 'A stale save cannot unpin a conversation');
  assert.equal(finalIndex.get(foreign.id).pinRank, foreignRank, 'A stale save cannot replace a rank');
  assert.equal(finalIndex.noteInboxReply(foreign.id, 'late-reply')?.revision, 1);
  assert.equal(finalIndex.get(foreign.id).pinRank, foreignRank);

  const frozen = finalIndex.all();
  await rename(file, file + '.retained'); await mkdir(file);
  try {
    assert.throws(() => finalIndex.movePinned(ref(beta, foreign.id), ref(alpha, records[3]!.id), 'before',
      expectedAfterRestart, new Set([alpha, beta])));
    assert.deepEqual(finalIndex.all(), frozen, 'A failed publication cannot affect the visible pin set or order');
  } finally { await rm(file, {recursive:true}); await rename(file + '.retained', file); }
  runtime = await createDesktopRuntime(state, () => { events++; });
  assert.deepEqual((await runtime.chats.pinned()).map(hit => hit.session.id), expectedAfterRestart);
  assert.deepEqual(await readdir(join(state, 'chats/pi')), []);
  assert.equal(events, 0);
  console.log('SESSION_PINS_OK: legacy order, same titles across workspaces, durable explicit rank, stale and crossed claims, Archive Undo, pin-to-top, late metadata and atomic disk failure. No Agent or model request.');
} finally { await runtime?.close(); await rm(directory, {recursive:true, force:true}); }
