// Only a private repository and its freshly created owned worktrees are modified.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project'), pi = join(root, 'offline-pi');
Object.assign(process.env, { HIVE_DESKTOP_DATA_DIR: data, PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1',
  GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null', GIT_CONFIG_NOSYSTEM: '1' });
const git = (cwd, ...args) => execFileSync('git', ['-C', cwd, ...args], { env: process.env, encoding: 'utf8' }).trim();
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const exists = async path => { try { await access(path); return true; } catch (e) { if (e.code === 'ENOENT') return false; throw e; } };
const runs = () => { const db = new DatabaseSync(join(data, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get().count; } finally { db.close(); } };
const ownerFile = id => join(data, 'worktrees', id, 'intent.json');
if (mode === 'seed') {
  await Promise.all([project, data, pi].map(path => mkdir(path, { recursive: true })));
  git(project, 'init', '--template=', '-q');
  git(project, 'config', 'user.name', 'Fixture'); git(project, 'config', 'user.email', 'fixture@example.invalid');
  git(project, 'config', 'commit.gpgSign', 'false');
  await writeFile(join(project, 'identity.txt'), 'ROOT\n'); git(project, 'add', 'identity.txt'); git(project, 'commit', '-qm', 'fixture');
  const dist = join(resources, 'runtime', 'dist');
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const { ChatIndex } = await import(pathToFileURL(join(dist, 'chat-index.js')).href);
  let runtime, workspace, clean, other;
  try {
    runtime = await createDesktopRuntime(data, () => {});
    workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Worktree removal QA', autostart_orchestrator: false });
    async function make(title) {
      const chat = await runtime.chats.createRecord(workspace.id, 'pi', { isolation: 'worktree' });
      const record = runtime.chats.rename(chat.id, title);
      const execution = await runtime.chats.sessionExecution(chat.id);
      return { record, path: execution.path, branch: git(execution.path, 'branch', '--show-current'), head: git(execution.path, 'rev-parse', 'HEAD') };
    }
    clean = await make('Clean worktree QA'); other = await make('Dirty survivor QA');
    await writeFile(join(other.path, 'identity.txt'), 'SURVIVOR DIRTY\n');
    assert.notEqual(clean.path, other.path); assert.notEqual(clean.branch, other.branch);
    assert.equal(git(clean.path, 'status', '--porcelain'), '');
    const owner = JSON.parse(await readFile(ownerFile(clean.record.id), 'utf8'));
    assert.equal(owner.state, 'ready'); assert.equal(owner.record.chatId, clean.record.id);
    assert.equal(owner.record.executionPath, clean.path); assert.equal(owner.record.branch, clean.branch);
    assert.equal(owner.record.baseOid, clean.head); assert.ok(owner.record.ownerToken);
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: clean.record.id } }));
  } finally { await runtime?.close(); }
  const sessionFile = join(data, 'chats', `${clean.record.id}.jsonl`);
  const text = 'ORIGINAL WORKTREE REMOVAL HISTORY 9153';
  const entries = [
    { type: 'session', id: clean.record.id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: clean.path },
    { type: 'message', id: 'removal-original', parentId: null, timestamp: '2026-11-14T00:00:00Z', message: { role: 'user', timestamp: 1, content: [{ type: 'text', text }] } },
  ];
  const original = entries.map(entry => JSON.stringify(entry)).join('\n') + '\n';
  await writeFile(sessionFile, original); new ChatIndex(join(data, 'chats')).save({ ...clean.record, sessionFile });
  const draft = 'Removal draft remains intact'; await mkdir(join(data, 'desktop-drafts'), { recursive: true });
  await writeFile(join(data, 'desktop-drafts', `session-${clean.record.id}.json`), JSON.stringify({ key: `session:${clean.record.id}`, input: draft, failed: [] }));
  await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
  const agentRunsBefore = runs(); assert.equal(agentRunsBefore, 0);
  const owner = JSON.parse(await readFile(ownerFile(clean.record.id), 'utf8'));
  await writeFile(join(root, 'worktree-remove-seed.json'), JSON.stringify({ workspaceId: workspace.id, project,
    clean: { sessionId: clean.record.id, path: clean.path, branch: clean.branch, head: clean.head },
    other: { sessionId: other.record.id, path: other.path, branch: other.branch }, ownerRecord: owner.record,
    sessionFile, historySha256: hash(original), originalBytes: Buffer.byteLength(original), text, draft, agentRunsBefore }, null, 2));
} else {
  const seed = JSON.parse(await readFile(join(root, 'worktree-remove-seed.json'), 'utf8'));
  const owner = JSON.parse(await readFile(ownerFile(seed.clean.sessionId), 'utf8'));
  assert.deepEqual(owner.record, seed.ownerRecord); assert.equal(owner.removed, true); assert.equal(owner.state, 'ready');
  assert.equal(await exists(seed.clean.path), false);
  const branches = git(project, 'branch', '--format=%(refname:short)').split(/\r?\n/);
  assert.ok(branches.includes(seed.clean.branch)); assert.ok(branches.includes(seed.other.branch));
  assert.equal(git(project, 'rev-parse', `refs/heads/${seed.clean.branch}`), seed.clean.head);
  assert.equal(await readFile(join(project, 'identity.txt'), 'utf8'), 'ROOT\n');
  assert.equal(await readFile(join(seed.other.path, 'identity.txt'), 'utf8'), 'SURVIVOR DIRTY\n');
  assert.equal(git(seed.other.path, 'branch', '--show-current'), seed.other.branch);
  assert.ok(git(seed.other.path, 'status', '--porcelain').includes('identity.txt'));
  const current = await readFile(seed.sessionFile);
  assert.equal(hash(current.subarray(0, seed.originalBytes)), seed.historySha256);
  const messages = current.toString('utf8').split('\n').filter(Boolean).map(line => JSON.parse(line)).filter(entry => entry.type === 'message');
  assert.equal(messages.length, 1); assert.equal(messages[0].id, 'removal-original'); assert.equal(messages[0].message.content[0].text, seed.text);
  const draft = JSON.parse(await readFile(join(data, 'desktop-drafts', `session-${seed.clean.sessionId}.json`), 'utf8'));
  assert.equal(draft.input, seed.draft); assert.equal(runs(), seed.agentRunsBefore);
  await writeFile(join(root, `worktree-remove-${mode}.json`), JSON.stringify({ sessionId: seed.clean.sessionId,
    removedReceipt: true, ownerIdentityRetained: true, cleanBranch: seed.clean.branch, cleanHead: seed.clean.head,
    otherBranch: seed.other.branch, otherDirtyBytesRetained: true, rootUnchanged: true,
    originalHistorySha256: seed.historySha256, retainedHistoryPrefixSha256: hash(current.subarray(0, seed.originalBytes)),
    messageId: 'removal-original', draftRetained: true, agentRunsBefore: seed.agentRunsBefore, agentRunsAfter: runs() }, null, 2));
}
