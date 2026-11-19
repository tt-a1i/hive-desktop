import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, realpath, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const [root, resources, mode = 'seed'] = process.argv.slice(2);
const state = join(root, 'state'), project = join(root, 'project');
const info = join(root, 'archived-seed.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const runs = () => {
  const db = new DatabaseSync(join(state, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get().count; }
  finally { db.close(); }
};
if (mode === 'verify') {
  const seed = JSON.parse(await readFile(info, 'utf8'));
  assert.equal(hash(await readFile(seed.sessionFile)), seed.historySha256, 'Source history changed');
  const draft = JSON.parse(await readFile(join(state, 'desktop-drafts', `session-${seed.sessionId}.json`), 'utf8'));
  assert.equal(draft.input, seed.draft, 'Draft changed');
  assert.equal(runs(), seed.agentRunsBefore, 'Unexpected agent run');
  const index = JSON.parse(await readFile(join(state, 'chats', 'index.json'), 'utf8'));
  assert.deepEqual(index.map(x => x.id), seed.sessionIds, 'Preview created or removed a session');
  await writeFile(join(root, 'archived-disk.json'), JSON.stringify({ sourceUnchanged: true, draftUnchanged: true,
    sessionIdsUnchanged: true, agentRunsBefore: seed.agentRunsBefore, agentRunsAfter: runs() }, null, 2));
} else {
  assert.equal(mode, 'seed');
  await mkdir(project, { recursive: true });
  await mkdir(join(root, 'offline-pi'), { recursive: true });
  const git = (...args) => execFileSync('git', args, { cwd: project, stdio: 'pipe' });
  git('init', '--template=', '-b', 'main');
  await writeFile(join(project, 'tracked.txt'), 'HANDOFF FIXTURE\n');
  git('add', 'tracked.txt');
  git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgSign=false', 'commit', '-m', 'fixture');
  const dist = join(resources, 'runtime', 'dist');
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const { ChatIndex } = await import(pathToFileURL(join(dist, 'chat-index.js')).href);
  let runtime, source, workspace, profile;
  try {
    runtime = await createDesktopRuntime(state, () => {}, { executableEnvironment: {} });
    workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Handoff QA', autostart_orchestrator: false });
    const sourceProfile = (await runtime.profiles.create({ driver: 'acp', label: 'Offline source QA', launch: { command: join(root, 'never-run-agent'), args: [] } }, (await runtime.profiles.list()).storeVersion)).profile;
    source = await runtime.chats.createRecord(workspace.id, 'acp', { profile: sourceProfile.ref });
    source = runtime.chats.rename(source.id, 'Handoff source QA');
    profile = (await runtime.profiles.create({ driver: 'pi', label: 'Offline target QA', launch: { args: [], configDir: join(root, 'offline-pi') } }, (await runtime.profiles.list()).storeVersion)).profile;
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: source.id } }));
  } finally { if (runtime) await runtime.close(); }
  const index = new ChatIndex(join(state, 'chats'));
  const nativeId = 'handoff-fixture-thread';
  const sessionFile = join(state, 'chats', 'handoff-fixture-acp.json');
  index.save({ ...index.get(source.id), identityState: 'known', originAgentId: `${workspace.id}:orchestrator`, acpSessionId: nativeId, sessionFile });
  const history = [
    { role: 'user', content: 'BEFORE excluded private text' },
    { role: 'assistant', content: 'SELECTED first statement' },
    { role: 'user', content: 'SELECTED last question' },
    { role: 'assistant', content: 'AFTER excluded private text' },
  ];
  await writeFile(sessionFile, JSON.stringify({ version: 1, sessionId: nativeId, history,
    historyIds: ['before', 'first', 'last', 'after'], contexts: [], deliveries: {} }));
  const draft = 'Preserve this handoff draft';
  await mkdir(join(state, 'desktop-drafts'), { recursive: true });
  await writeFile(join(state, 'desktop-drafts', `session-${source.id}.json`), JSON.stringify({ key: `session:${source.id}`, input: draft, failed: [] }));
  await writeFile(join(state, 'desktop-language.json'), JSON.stringify('en'));
  const sessionIds = JSON.parse(await readFile(join(state, 'chats', 'index.json'), 'utf8')).map(x => x.id);
  const canonicalProject = await realpath(project);
  const [originalDirectory, canonicalDirectory] = await Promise.all([stat(project), stat(canonicalProject)]);
  assert.equal(originalDirectory.dev, canonicalDirectory.dev);
  assert.equal(originalDirectory.ino, canonicalDirectory.ino);
  await writeFile(info, JSON.stringify({ sessionId: source.id, originAgentId: `${workspace.id}:orchestrator`, messageId: 'last', worktree: canonicalProject, sessionFile,
    historySha256: hash(await readFile(sessionFile)), sessionIds, draft, agentRunsBefore: runs(), profile: profile.ref }));
}
