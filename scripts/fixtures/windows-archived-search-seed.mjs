import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { access, mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const [root, resources, mode = 'seed'] = process.argv.slice(2);
const state = join(root, 'state');
const project = join(root, 'project');
const pi = join(root, 'offline-pi');
const infoPath = join(root, 'archived-seed.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const exists = async path => { try { await access(path); return true; } catch { return false; } };
const countRuns = () => {
  const db = new DatabaseSync(join(state, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get().count; }
  finally { db.close(); }
};

if (mode === 'verify') {
  const seed = JSON.parse(await readFile(infoPath, 'utf8'));
  const data = await readFile(seed.sessionFile);
  assert.equal(hash(data), seed.historySha256, 'Original history must not be rewritten or appended');
  assert.equal(await exists(seed.worktree), false, 'The missing worktree must not be recreated');
  assert.equal(await readFile(join(seed.movedWorktree, 'tracked.txt'), 'utf8'), 'ISOLATED WORKTREE SENTINEL\n');
  assert.equal(await readFile(join(project, 'tracked.txt'), 'utf8'), 'PROJECT ROOT SENTINEL\n');
  const entries = JSON.parse(await readFile(join(state, 'chats', 'index.json'), 'utf8'));
  const session = entries.find(entry => entry.id === seed.sessionId);
  assert.equal(session?.archived, true, 'Opening the archived conversation must not restore it');
  assert.equal(session?.sessionFile, seed.sessionFile);
  assert.deepEqual(session?.execution, seed.execution);
  assert.equal(countRuns(), seed.agentRunsBefore, 'No agent run may start');
  const draft = JSON.parse(await readFile(join(state, 'desktop-drafts', `session-${seed.sessionId}.json`), 'utf8'));
  assert.equal(draft.input, seed.draft);
  await writeFile(join(root, 'archived-disk.json'), JSON.stringify({
    sessionId: seed.sessionId, messageId: seed.messageId, historySha256: hash(data),
    sourceUnchanged: true, archived: true, worktreeStillMissing: true, sentinelsUnchanged: true,
    draftUnchanged: true, agentRunsBefore: seed.agentRunsBefore, agentRunsAfter: countRuns(),
  }, null, 2) + '\n');
} else {
  assert.equal(mode, 'seed');
  await mkdir(project, { recursive: true });
  await mkdir(pi, { recursive: true });
  process.env.PI_CODING_AGENT_DIR = pi;
  process.env.PI_OFFLINE = '1';
  const git = (...args) => execFileSync('git', args, { cwd: project, stdio: 'pipe' });
  git('init', '--template=', '-b', 'main');
  await writeFile(join(project, 'tracked.txt'), 'PROJECT ROOT SENTINEL\n');
  git('add', 'tracked.txt');
  git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgSign=false', 'commit', '-m', 'fixture');
  const dist = join(resources, 'runtime', 'dist');
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const { ChatIndex } = await import(pathToFileURL(join(dist, 'chat-index.js')).href);
  let runtime;
  let workspace, ordinary, session, worktree;
  try {
    runtime = await createDesktopRuntime(state, () => {});
    workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Archived worktree QA', autostart_orchestrator: false });
    ordinary = await runtime.chats.createRecord(workspace.id, 'pi');
    runtime.chats.rename(ordinary.id, 'Root QA');
    const created = await runtime.chats.createRecord(workspace.id, 'pi', { isolation: 'worktree' });
    session = runtime.chats.rename(created.id, 'Independent history QA');
    ({ path: worktree } = await runtime.chats.sessionExecution(created.id));
    await writeFile(join(worktree, 'tracked.txt'), 'ISOLATED WORKTREE SENTINEL\n');
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: ordinary.id } }));
  } finally { if (runtime) await runtime.close(); }
  const sessionFile = join(state, 'chats', `${session.id}.jsonl`);
  const messageId = 'qa-worktree-user';
  const text = 'The durable transcript records Cerulean Recovery 8DA7 after the checkout was moved.';
  const entries = [
    { type: 'session', id: session.id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: worktree },
    { type: 'message', id: messageId, parentId: null, timestamp: '2026-11-14T00:00:00Z',
      message: { role: 'user', content: [{ type: 'text', text }], timestamp: 1 } },
  ];
  await writeFile(sessionFile, entries.map(entry => JSON.stringify(entry)).join('\n') + '\n');
  new ChatIndex(join(state, 'chats')).save({ ...session, sessionFile, archived: true });
  const draft = 'Offline archive draft preserved';
  const draftDir = join(state, 'desktop-drafts');
  await mkdir(draftDir, { recursive: true });
  await writeFile(join(draftDir, `session-${session.id}.json`), JSON.stringify({ key: `session:${session.id}`, input: draft, failed: [] }));
  const movedWorktree = worktree + '.offline-qa';
  await rename(worktree, movedWorktree);
  await writeFile(join(state, 'desktop-language.json'), JSON.stringify('en'));
  const beforeSearch = await readFile(sessionFile);
  const runsBeforeSearch = countRuns();
  assert.equal(runsBeforeSearch, 0, 'Fixture setup must not start any agent run');
  let searchRuntime;
  try {
    searchRuntime = await createDesktopRuntime(state, () => {});
    const result = await searchRuntime.chats.search('Cerulean Recovery 8DA7', true, 0, workspace.id);
    assert.equal(result.matches.length, 1);
    assert.equal(result.matches[0].session.id, session.id);
    assert.equal(result.matches[0].bodyMatch?.messageId, messageId);
    assert.ok(result.matches[0].bodyMatch.snippet.includes('Cerulean Recovery 8DA7'));
  } finally { if (searchRuntime) await searchRuntime.close(); }
  assert.ok((await readFile(sessionFile)).equals(beforeSearch));
  assert.equal(await exists(worktree), false);
  assert.equal(countRuns(), runsBeforeSearch);
  await writeFile(infoPath, JSON.stringify({
    sessionId: session.id, messageId, text, sessionFile, execution: session.execution,
    worktree, movedWorktree, draft, historySha256: hash(await readFile(sessionFile)), agentRunsBefore: countRuns(),
  }, null, 2) + '\n');
}
