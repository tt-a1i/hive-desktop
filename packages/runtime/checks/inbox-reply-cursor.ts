/** Exact inbox reply cursor through the production wire and a controlled ACP peer. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdir, mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import type {ChatRecord} from '../src/chat-index.js';

const root = await mkdtemp(join(tmpdir(), 'hive-inbox-replies-'));
const dataDir = join(root, 'desktop');
const peerDir = join(root, 'peer');
const peer = fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs', import.meta.url));
const entry = fileURLToPath(new URL('../dist/main.js', import.meta.url));

function launch() {
  const events: Array<{type: string; sessionId: string; messageId?: string; message?: {role: string}; inboxReply?: {workspaceId: string; revision: number}}> = [];
  const processHandle = spawn(process.execPath, [entry], {
    env: {...process.env, HIVE_DESKTOP_DATA_DIR: dataDir, PI_CODING_AGENT_DIR: join(root, 'pi'), PI_OFFLINE: '1'},
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let stderr = '';
  processHandle.stderr.on('data', chunk => {stderr += String(chunk);});
  const exit = once(processHandle, 'exit');
  const reader = createInterface({input: processHandle.stdout});
  const pending = new Map<number, (frame: {result?: unknown; error?: unknown}) => void>();
  let sequence = 0;
  reader.on('line', line => {
    const frame = JSON.parse(line);
    if (frame.event === 'agent') events.push(frame.data);
    pending.get(frame.id)?.(frame);
  });
  function request<T>(method: string, params: object = {}): Promise<T> {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`${method} timed out: ${stderr}`));
      }, 10_000);
      pending.set(id, frame => {
        clearTimeout(timer);
        pending.delete(id);
        if (frame.error !== undefined) reject(new Error(JSON.stringify(frame.error)));
        else resolve(frame.result as T);
      });
      processHandle.stdin.write(JSON.stringify({id, method, params}) + '\n');
    });
  }
  async function close() {
    try { await request('shutdown'); }
    finally {
      processHandle.stdin.end();
      const stopped = await Promise.race([exit.then(() => true), delay(2_000, false)]);
      if (!stopped) processHandle.kill('SIGKILL');
      await exit;
    }
  }
  return {request, events, close};
}

async function reply(wire: ReturnType<typeof launch>, workspaceId: string, sessionId: string, revision: number) {
  await wire.request('chat.prompt', {sessionId, text: `Test reply ${revision}`});
  for (let count = 0; count < 120; count++) {
    const event = wire.events.find(event => event.type === 'message_end'
      && event.sessionId === sessionId && event.inboxReply?.revision === revision);
    if (event) {
      assert.equal(event.message?.role, 'assistant');
      assert.equal(event.inboxReply?.workspaceId, workspaceId);
      assert.ok(event.messageId);
      return {revision, messageId: event.messageId};
    }
    await delay(30);
  }
  assert.fail('Expected a finalized assistant reply with its exact revision');
}

let wire: ReturnType<typeof launch> | undefined;
try {
  await mkdir(join(root, 'pi'));
  wire = launch();
  await wire.request('initialize');
  const projectPath = join(root, 'project');
  await mkdir(projectPath);
  const workspace = await wire.request<{id: string}>('hive.request', {
    method: 'POST', path: '/api/workspaces', body: {path: projectPath, name: 'Inbox reply cursor', autostart_orchestrator: false},
  });
  const profileList = await wire.request<{storeVersion: number}>('agent.profiles.list');
  const {profile} = await wire.request<{profile: {ref: {id: string; revision: number}}}>('agent.profiles.create', {
    driver: 'acp', label: 'Controlled inbox peer', expectedStoreVersion: profileList.storeVersion,
    launch: {command: process.execPath, args: [peer, peerDir]},
  });
  const create = (current: ReturnType<typeof launch>) => current.request<{id: string}>('chat.createRecord', {
    workspaceId: workspace.id, backend: 'acp', profile: profile.ref,
  });
  const {id} = await create(wire);
  const other = await create(wire);
  const sessions = async (current: ReturnType<typeof launch>) => current.request<ChatRecord[]>('chat.list', {workspaceId: workspace.id});
  assert.deepEqual((await sessions(wire)).map(chat => chat.inboxReply), [undefined, undefined]);
  const first = await reply(wire, workspace.id, id, 1);
  assert.equal(wire.events.filter(event => event.inboxReply !== undefined && event.sessionId === id).length, 1);
  assert.ok(wire.events.some(event => event.type === 'message_end' && event.message?.role === 'user' && !event.inboxReply));
  await wire.request('chat.rename', {sessionId: id, title: 'Renamed'});
  assert.deepEqual((await sessions(wire)).find(chat => chat.id === id)?.inboxReply, first);
  await wire.close();
  wire = undefined;
  const protocol = join(peerDir, 'protocol.jsonl');
  const before = await readFile(protocol, 'utf8');

  wire = launch();
  await wire.request('initialize');
  const cold = await sessions(wire);
  assert.equal(cold.find(chat => chat.id === id)?.title, 'Renamed');
  assert.deepEqual(cold.find(chat => chat.id === id)?.inboxReply, first);
  assert.equal(cold.find(chat => chat.id === other.id)?.inboxReply, undefined);
  assert.equal(await readFile(protocol, 'utf8'), before, 'Listing cold chats must not launch a provider');
  const second = await reply(wire, workspace.id, id, 2);
  assert.notEqual(first.messageId, second.messageId);
  const final = await sessions(wire);
  assert.deepEqual(final.find(chat => chat.id === id)?.inboxReply, second);
  assert.equal(final.find(chat => chat.id === other.id)?.inboxReply, undefined);
  await assert.rejects(wire.request('chat.archive', {sessionId: id, archived: true,
    inbox: {workspaceId: workspace.id, replyRevision: first.revision}}), /ARCHIVE_INBOX_STALE/);
  assert.equal(Boolean((await sessions(wire)).find(chat => chat.id === id)?.archived), false);
  console.log('INBOX_REPLY_CURSOR_OK: exact finalized ACP replies, passive cold list, rename and later reply');
} finally {
  await wire?.close();
  await rm(root, {recursive: true, force: true});
}
