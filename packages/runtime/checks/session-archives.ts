/** Real Hive HTTP/SQLite, native Pi queues and the actual team CLI. Loopback inference only. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { ChatIndex, type ChatBackend } from '../src/chat-index.js';
import type { ChatEvent } from '../src/chats.js';
import { createTeamTool } from '../src/team-tool.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-session-archives-'));
const state = join(directory, 'state');
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
let requests = 0;
const server = createServer(async (request, response) => {
  for await (const _ of request) {
  }
  requests++;
  response.writeHead(400, { 'content-type': 'application/json' });
  response.end(
    JSON.stringify({
      error: { type: 'invalid_request_error', message: 'Intentional local archive check rejection' },
    }),
  );
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const address = server.address();
assert.ok(address && typeof address !== 'string');
await writeFile(
  join(agentDir, 'models.json'),
  JSON.stringify({
    providers: {
      'archive-local': {
        api: 'openai-responses',
        baseUrl: `http://127.0.0.1:${address.port}/v1`,
        apiKey: 'fixture-only',
        models: [{ id: 'gpt-6.1-terra', name: 'Terra local archive check', input: ['text'] }],
      },
    },
  }),
);
await writeFile(
  join(agentDir, 'settings.json'),
  JSON.stringify({ defaultProvider: 'archive-local', defaultModel: 'gpt-6.1-terra' }),
);
const previousDir = process.env.PI_CODING_AGENT_DIR;
const previousOffline = process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR = agentDir;
process.env.PI_OFFLINE = '1';
const { createDesktopRuntime } = await import('../src/desktop-runtime.js');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const events: ChatEvent[] = [];
const file = join(state, 'chats/index.json');
try {
  runtime = await createDesktopRuntime(state, (event) => events.push(event));
  async function workspace(name: string) {
    const path = join(directory, name);
    await mkdir(path);
    return (await runtime!.hive.request('POST', '/api/workspaces', {
      path,
      name,
      autostart_orchestrator: false,
    })) as { id: string };
  }
  const alpha = await workspace('Alpha');
  const beta = await workspace('Beta');
  const legacy = await runtime.chats.createRecord(beta.id, 'codex');
  const archivedIds = new Set<string>();
  for (const backend of ['pi', 'codex', 'pi-rpc', 'claude'] satisfies ChatBackend[]) {
    const chat = await runtime.chats.createRecord(alpha.id, backend);
    const original = runtime.chats.setPinned(chat.id, true);
    const archived = await runtime.chats.setArchived(chat.id, true);
    assert.deepEqual(archived, { ...original, archived: true });
    assert.equal((await runtime.chats.pinned()).length, 0);
    assert.equal(
      (await runtime.chats.search(backend, true)).matches.filter((hit) => hit.session.id === chat.id).length,
      1,
    );
    assert.equal(
      (await runtime.chats.search('')).matches.some((hit) => hit.session.id === chat.id),
      false,
    );
    const restored = await runtime.chats.setArchived(chat.id, false);
    assert.deepEqual(restored, { ...original, archived: false });
    assert.equal((await runtime.chats.pinned())[0]?.session.id, chat.id);
    await runtime.chats.setArchived(chat.id, true);
    archivedIds.add(chat.id);
  }
  for (let i = 0; i < 104; i++) {
    const chat = await runtime.chats.createRecord(alpha.id, 'codex');
    runtime.chats.rename(chat.id, '同名历史会话');
    await runtime.chats.setArchived(chat.id, true);
    archivedIds.add(chat.id);
  }
  const first = await runtime.chats.search('', true);
  const second = await runtime.chats.search('', true, 100);
  assert.equal(first.matches.length, 100);
  assert.equal(second.matches.length, 8);
  assert.equal(first.total, 108);
  assert.deepEqual(new Set([...first.matches, ...second.matches].map((hit) => hit.session.id)), archivedIds);
  assert.deepEqual(
    (await runtime.chats.search('Beta')).matches.map((hit) => hit.session.id),
    [legacy.id],
  );
  assert.ok(events.every((event) => event.type === 'session_navigation'));
  assert.deepEqual(await readdir(join(state, 'chats/pi')), []);
  assert.equal(requests, 0);
  const target = first.matches[0]!.session;
  const before = await readFile(file, 'utf8');
  const eventCount = events.length;
  for (const invalid of [undefined, null, 'true', 0, 1, {}, []]) {
    await assert.rejects(runtime.chats.setArchived(target.id, invalid), TypeError);
  }
  await assert.rejects(runtime.chats.setArchived('missing', true));
  await assert.rejects(runtime.chats.search('', 'true'), TypeError);
  for (const offset of [-1, 0.5, NaN, '100'])
    await assert.rejects(runtime.chats.search('', true, offset), TypeError);
  assert.equal(await readFile(file, 'utf8'), before);
  await rename(file, file + '.retained');
  await mkdir(file);
  try {
    await assert.rejects(runtime.chats.setArchived(target.id, false));
    assert.equal(runtime.chats.list(alpha.id).find((chat) => chat.id === target.id)?.archived, true);
    assert.equal(
      events.length,
      eventCount,
      'A failed metadata commit must not emit a successful navigation change',
    );
    assert.deepEqual(
      (await readdir(join(state, 'chats'))).filter((name) => name.endsWith('.tmp')),
      [],
    );
  } finally {
    await rm(file, { recursive: true });
    await rename(file + '.retained', file);
  }
  await runtime.close();
  runtime = undefined;
  const stored = JSON.parse(await readFile(file, 'utf8'));
  delete stored.find((item: any) => item.id === legacy.id).archived;
  await writeFile(file, JSON.stringify(stored));
  assert.equal(new ChatIndex(join(state, 'chats')).get(legacy.id).archived, undefined);
  stored[0].archived = 'yes';
  await writeFile(file, JSON.stringify(stored));
  assert.throws(() => new ChatIndex(join(state, 'chats')));
  await writeFile(file, before);
  runtime = await createDesktopRuntime(state, (event) => events.push(event));
  assert.equal((await runtime.chats.search('', true)).total, 108);

  // A live but idle structured session may archive without stopping or rebinding.
  const liveWorkspace = await workspace('Live');
  const live = await runtime.chats.create(liveWorkspace.id, 'pi');
  assert.equal(live.model?.id, 'gpt-6.1-terra');
  assert.equal(live.model?.provider, 'archive-local');
  await runtime.chats.resume(live.id);
  const opened = await runtime.chats.open(live.id);
  assert.equal(opened.busy, false);
  assert.equal(opened.pending, 0);
  await runtime.chats.setArchived(live.id, true);
  const archivedRun = await runtime.chats.open(live.id);
  assert.equal(archivedRun.runId, opened.runId);
  assert.equal(archivedRun.agentId, opened.agentId);
  assert.deepEqual(archivedRun.history, opened.history);
  const relevantStart = events.length;
  await runtime.chats.prompt(live.id, '归档后收到的新消息，不使用工具');
  const deadline = Date.now() + 10_000;
  for (;;) {
    const snapshot = await runtime.chats.open(live.id);
    if (!snapshot.busy && snapshot.pending === 1) break;
    assert.ok(Date.now() < deadline);
    await new Promise((resolve) => setTimeout(resolve, 15));
  }
  assert.equal(runtime.chats.list(liveWorkspace.id)[0]?.archived, false);
  assert.ok(
    events.slice(relevantStart).some((event) => event.type === 'session_navigation' && !event.archived),
  );
  await assert.rejects(runtime.chats.setArchived(live.id, true));
  assert.equal(requests, 1);
  await runtime.close();
  runtime = undefined;
  runtime = await createDesktopRuntime(state, (event) => events.push(event));
  await assert.rejects(
    runtime.chats.setArchived(live.id, true),
    'Cold persisted pending deliveries must block archiving without loading the Agent',
  );
  assert.equal(requests, 1);

  // Actual Hive dispatch to an archived worker wakes it; the owning lead cannot
  // archive while that real dispatch remains unclosed, even with no own queue.
  const teamWorkspace = await workspace('Team');
  const lead = await runtime.chats.create(teamWorkspace.id, 'pi');
  await runtime.chats.resume(lead.id);
  const leadId = `${teamWorkspace.id}:orchestrator`;
  const token = runtime.agents.peekAgentToken(leadId)!;
  assert.ok(token);
  const team = createTeamTool({
    workspaceId: teamWorkspace.id,
    agentId: leadId,
    token,
    hivePort: String(runtime.hive.port),
    cwd: join(directory, 'Team'),
  });
  await team.execute(
    'archive-spawn',
    { args: ['spawn', 'coder', '--name', 'archive-worker'] },
    undefined,
    undefined,
    undefined as never,
  );
  const members = (await runtime.hive.request('GET', `/api/ui/workspaces/${teamWorkspace.id}/team`)) as {
    id: string;
    name: string;
  }[];
  const worker = members.find((member) => member.name === 'archive-worker')!;
  assert.ok(worker);
  await runtime.hive.request(
    'POST',
    `/api/workspaces/${teamWorkspace.id}/agents/${encodeURIComponent(worker.id)}/start`,
  );
  const workerChat = runtime.chats.list(teamWorkspace.id).find((chat) => chat.agentId === worker.id)!;
  assert.ok(workerChat);
  await runtime.chats.abort(workerChat.id);
  await runtime.chats.setArchived(workerChat.id, true);
  await team.execute(
    'archive-send',
    { args: ['send', 'archive-worker', '检查归档成员收到真实派单后自动恢复'] },
    undefined,
    undefined,
    undefined as never,
  );
  assert.equal(
    runtime.chats.list(teamWorkspace.id).find((chat) => chat.id === workerChat.id)?.archived,
    false,
  );
  const workerSnapshot = await runtime.chats.open(workerChat.id);
  assert.equal(workerSnapshot.pending, 1);
  assert.equal((await runtime.chats.open(lead.id)).pending, 0);
  const dispatches = (await runtime.hive.request(
    'GET',
    `/api/ui/workspaces/${teamWorkspace.id}/dispatches`,
  )) as any[];
  assert.equal(dispatches.length, 1);
  assert.equal(dispatches[0].state, 'submitted');
  await assert.rejects(runtime.chats.setArchived(workerChat.id, true));
  await assert.rejects(runtime.chats.setArchived(lead.id, true));
  assert.equal(requests, 1);
  console.log(
    'SESSION_ARCHIVES_OK: four backends, old records, atomic disk rollback, 108 paginated archives, live identity/history preservation, actual user input and Hive dispatch auto-restore, live/cold pending and owning-lead dispatch guards. Real SDK + loopback only.',
  );
} finally {
  await runtime?.close();
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  if (previousDir === undefined) delete process.env.PI_CODING_AGENT_DIR;
  else process.env.PI_CODING_AGENT_DIR = previousDir;
  if (previousOffline === undefined) delete process.env.PI_OFFLINE;
  else process.env.PI_OFFLINE = previousOffline;
  await rm(directory, { recursive: true, force: true });
}
