/** Actual Chats.open + managed Git worktrees. Pi uses its real SDK/SessionManager;
 * external transports and the official Claude SDK launch explicitly controlled
 * subprocess peers. The single Codex turn is synthetic materialization only. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatBackend, ChatRecord } from '../src/chat-index.js';
import type { Chats } from '../src/chats.js';
import type { PiSessionHandle } from '../src/agents/types.js';

assert.ok(!process.argv.includes('--live'), 'This check must never run native vendor CLIs or models');
const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-backend-worktree-cwd-')));
const repo = join(directory, 'repo'), project = join(repo, 'sub project'), data = join(directory, 'state');
const keys = ['PI_CODING_AGENT_DIR', 'PI_OFFLINE', 'CLAUDE_CONFIG_DIR', 'HIVE_CWD_FIXTURE_WIRE', 'GIT_CONFIG_GLOBAL', 'GIT_CONFIG_NOSYSTEM'] as const;
const previous = Object.fromEntries(keys.map(key => [key, process.env[key]]));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let requests = 0;
const server = createServer((_request, response) => { requests++; response.writeHead(500).end('Unexpected model request'); });
const live = (chats: Chats) => (chats as unknown as {live: Map<string, {handle: PiSessionHandle}>}).live;
const records = new Map<ChatBackend, {record: ChatRecord; root: string; nativeId: string; historyIds: string[]; file?: string}>();
type Wire = {backend: ChatBackend; cwd: string; event: string; args?: string[]; method?: string; params?: Record<string, unknown>; type?: string; action?: string; shouldQuery?: boolean};
try {
  await mkdir(project, {recursive: true}); await mkdir(join(directory, 'pi')); await writeFile(join(directory, 'gitconfig'), '');
  process.env.GIT_CONFIG_GLOBAL = join(directory, 'gitconfig'); process.env.GIT_CONFIG_NOSYSTEM = '1';
  process.env.PI_CODING_AGENT_DIR = join(directory, 'pi'); process.env.PI_OFFLINE = '1';
  process.env.CLAUDE_CONFIG_DIR = join(directory, 'claude'); process.env.HIVE_CWD_FIXTURE_WIRE = join(directory, 'wire.jsonl');
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const address = server.address(); assert.ok(address && typeof address !== 'string');
  await writeFile(join(directory, 'pi', 'models.json'), JSON.stringify({providers: {openai: {api: 'openai-responses', baseUrl: `http://127.0.0.1:${address.port}/v1`, apiKey: 'fixture-only', models: [{id: 'fixture-cwd', name: 'No model requests allowed', input: ['text']}]}}}));
  await writeFile(join(directory, 'pi', 'settings.json'), JSON.stringify({defaultProvider: 'openai', defaultModel: 'fixture-cwd'}));
  const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgSign=false', ...args], {cwd: repo, encoding: 'utf8', stdio: 'pipe'});
  git('init', '--template='); await writeFile(join(project, 'file.txt'), 'committed project file'); git('add', '.'); git('commit', '-qm', 'base');
  const peer = join(directory, 'controlled-peer.cjs');
  await writeFile(peer, await readFile(new URL('./fixtures/worktree-backend-cwd-peer.cjs', import.meta.url)), {mode: 0o700});
  const options = {executableEnvironment: {HIVE_CODEX_COMMAND: peer, HIVE_PI_COMMAND: peer, HIVE_CLAUDE_COMMAND: peer}};
  runtime = await createDesktopRuntime(data, () => {}, options);
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: project, name: 'Backend cwd fixture', autostart_orchestrator: false}) as {id: string};
  const backends: ChatBackend[] = ['pi', 'codex', 'pi-rpc', 'claude'];
  for (const backend of backends) {
    const record = await runtime.chats.createRecord(workspace.id, backend, {isolation: 'worktree'});
    runtime.chats.rename(record.id, `cwd-${backend}-title`);
    const execution = await runtime.chats.sessionExecution(record.id);
    assert.equal(execution.kind, 'worktree'); assert.notEqual(execution.path, project); assert.ok(execution.path.endsWith('/sub project'));
    const opened = await runtime.chats.open(record.id);
    assert.equal(opened.executionPath, execution.path); assert.equal(opened.execution?.path, execution.path); assert.equal(opened.workspaceId, workspace.id);
    const handle: PiSessionHandle = live(runtime.chats).get(record.id)!.handle;
    assert.equal(handle.hasContext('hive-desktop-execution'), true);
    if (backend === 'codex') {
      // Production materialization callback must save the native ID. No injected
      // ChatIndex native IDs or mocked createCodexSession are used here.
      await handle.prompt('Controlled protocol fixture: materialize this local thread without a model');
      assert.ok(runtime.chats.list(workspace.id).find(item => item.id === record.id)!.codexThreadId);
    }
    const current: ChatRecord = runtime.chats.list(workspace.id).find(item => item.id === record.id)!;
    const nativeId: string | undefined = backend === 'pi' ? current.piSessionId : backend === 'pi-rpc' ? current.externalPiSessionId : backend === 'codex' ? current.codexThreadId : current.nativeclaudeSessionId;
    assert.ok(nativeId); assert.equal(nativeId, handle.sessionId);
    if (backend === 'pi' || backend === 'pi-rpc') {
      assert.ok(current.sessionFile);
      const header = JSON.parse((await readFile(current.sessionFile, 'utf8')).split('\n')[0]!);
      assert.equal(header.cwd, execution.path); assert.equal(header.id, nativeId);
    }
    if (backend === 'claude') {
      const transcript = join(process.env.CLAUDE_CONFIG_DIR!, 'projects', execution.path.replace(/[^a-zA-Z0-9]/g, '-'), nativeId + '.jsonl');
      const rows = (await readFile(transcript, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
      assert.ok(rows.length > 0); assert.ok(rows.every(row => row.cwd === execution.path && row.shouldQuery === false));
      assert.ok(rows.some(row => row.message.content.includes(project) && row.message.content.includes(execution.path)));
    }
    records.set(backend, {record: current, root: execution.path, nativeId, historyIds: [...handle.snapshot().historyMessageIds], ...(current.sessionFile && {file: current.sessionFile})});
  }
  const source = await runtime.hive.request('GET', '/api/workspaces') as {id: string; path: string}[];
  assert.equal(source.find(item => item.id === workspace.id)!.path, project);
  assert.equal(source.length, 1, 'Managed worktrees never create additional Hive workspaces');
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  for (const backend of backends) {
    const saved = records.get(backend)!;
    const reopened = await runtime.chats.open(saved.record.id);
    assert.equal(reopened.executionPath, saved.root); assert.equal(reopened.execution?.path, saved.root); assert.equal(reopened.workspaceId, workspace.id);
    const handle: PiSessionHandle = live(runtime.chats).get(saved.record.id)!.handle;
    assert.equal(handle.sessionId, saved.nativeId); assert.equal(handle.hasContext('hive-desktop-execution'), true);
    assert.deepEqual(handle.snapshot().historyMessageIds.slice(0, saved.historyIds.length), saved.historyIds, `${backend} restores the original native history rather than only reusing an ID`);
    if (saved.file) assert.equal(reopened.sessionFile, saved.file);
    if (backend === 'pi') assert.equal(reopened.piSessionId, saved.nativeId);
    if (backend === 'codex') assert.equal(reopened.codexThreadId, saved.nativeId);
    if (backend === 'pi-rpc') assert.equal(reopened.externalPiSessionId, saved.nativeId);
    if (backend === 'claude') assert.equal(reopened.nativeclaudeSessionId, saved.nativeId);
  }
  const wire = (await readFile(process.env.HIVE_CWD_FIXTURE_WIRE!, 'utf8')).trim().split('\n').map(line => JSON.parse(line) as Wire);
  const argument = (args: string[], name: string) => args.find(value => value.startsWith(name + '='))?.slice(name.length + 1) ?? (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
  assert.equal(wire.some(row => row.event === 'fixture-error'), false);
  for (const backend of ['codex', 'pi-rpc', 'claude'] as const) {
    const saved = records.get(backend)!;
    const starts = wire.filter(row => row.backend === backend && row.event === 'start');
    assert.equal(starts.length, 2, `${backend} must launch once in each runtime`);
    assert.ok(starts.every(row => row.cwd === saved.root && row.cwd !== project));
    if (backend === 'pi-rpc') {
      assert.equal(starts[0]!.args!.includes('--session'), false);
      const at = starts[1]!.args!.indexOf('--session'); assert.ok(at >= 0); assert.equal(starts[1]!.args![at + 1], saved.file);
    }
    if (backend === 'claude') {
      assert.equal(argument(starts[0]!.args!, '--session-id'), saved.nativeId); assert.equal(argument(starts[1]!.args!, '--resume'), saved.nativeId);
      assert.equal(wire.some(row => row.backend === backend && row.type === 'user' && row.shouldQuery !== false), false);
    }
    if (backend === 'codex') {
      assert.equal(wire.find(row => row.backend === backend && row.method === 'thread/start')!.params!.cwd, saved.root);
      assert.equal(wire.find(row => row.backend === backend && row.method === 'thread/resume')!.params!.threadId, saved.nativeId);
      assert.equal(wire.filter(row => row.backend === backend && row.method === 'turn/start').length, 1, 'Exactly one synthetic fixture turn materializes the ID');
    }
  }
  // A missing source must be a per-chat search warning, while other title hits
  // remain usable. Search must not launch another backend to hide the problem.
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  const missing = records.get('pi')!; await rename(missing.root, missing.root + '.missing');
  try {
    const result = await runtime.chats.search('cwd-', false, 0, workspace.id);
    assert.ok(result.incompleteHistory.some(item => item.sessionId === missing.record.id));
    assert.ok(result.matches.some(item => item.session.id === records.get('codex')!.record.id));
    await assert.rejects(runtime.chats.open(missing.record.id), /worktree|工作树|ENOENT/i);
  } finally { await rename(missing.root + '.missing', missing.root); }
  assert.equal((await readFile(process.env.HIVE_CWD_FIXTURE_WIRE!, 'utf8')).trim().split('\n').filter(line => JSON.parse(line).event === 'start').length,
    wire.filter(row => row.event === 'start').length, 'Search and rejected missing-source open never launch another child');
  const after = await runtime.hive.request('GET', '/api/workspaces') as {id: string; path: string}[];
  assert.equal(after.length, 1); assert.equal(after[0]!.path, project); assert.equal(requests, 0);
  console.log('WORKTREE_BACKEND_CWD_OK: four actual Chats.open adapters, real Pi SDK header, controlled Codex/Pi-RPC child cwd+restore args and official Claude SDK transcript; cold native identity/path retained, one unchanged Hive workspace, missing-source search warning. Synthetic protocol only; zero model requests.');
} finally {
  await runtime?.close();
  if (server.listening) { server.close(); await once(server, 'close'); }
  for (const key of keys) { if (previous[key] === undefined) delete process.env[key]; else process.env[key] = previous[key]; }
  await rm(directory, {recursive: true, force: true, maxRetries: 5});
}
