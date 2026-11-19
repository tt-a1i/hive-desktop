// All Git and runtime operations use a newly created private fixture directory.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project'), pi = join(root, 'offline-pi');
Object.assign(process.env, {
  HIVE_DESKTOP_DATA_DIR: data, PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1',
  GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null', GIT_CONFIG_NOSYSTEM: '1',
});
const git = (path, ...args) => execFileSync('git', ['-C', path, ...args], { env: process.env, encoding: 'utf8' }).trim();
const runs = () => {
  const db = new DatabaseSync(join(data, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get().count; }
  finally { db.close(); }
};
if (mode === 'seed') {
  await Promise.all([project, data, pi].map(path => mkdir(path, { recursive: true })));
  git(project, 'init', '--template=', '-q');
  git(project, 'config', 'user.name', 'Fixture');
  git(project, 'config', 'user.email', 'fixture@example.invalid');
  git(project, 'config', 'commit.gpgSign', 'false');
  await writeFile(join(project, 'identity.txt'), 'ROOT\n');
  git(project, 'add', 'identity.txt');
  git(project, 'commit', '-qm', 'fixture');
  const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime', 'dist', 'desktop-runtime.js')).href);
  let runtime;
  try {
    runtime = await createDesktopRuntime(data, () => {});
    const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Worktree ownership QA', autostart_orchestrator: false });
    async function make(title, content) {
      const chat = await runtime.chats.create(workspace.id, 'pi', { isolation: 'worktree' });
      runtime.chats.rename(chat.id, title);
      const execution = await runtime.chats.sessionExecution(chat.id);
      await writeFile(join(execution.path, 'identity.txt'), content);
      return { sessionId: chat.id, path: execution.path, branch: git(execution.path, 'branch', '--show-current'), content };
    }
    const first = await make('Dirty worktree QA', 'FIRST DIRTY\n');
    const second = await make('Other worktree QA', 'SECOND DIRTY\n');
    assert.notEqual(first.path, second.path);
    assert.notEqual(first.branch, second.branch);
    assert.notEqual(first.path, project);
    assert.notEqual(second.path, project);
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: first.sessionId } }));
    await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
    const agentRunsBefore = runs();
    assert.equal(agentRunsBefore, 0);
    await writeFile(join(root, 'worktree-dirty-seed.json'), JSON.stringify({ workspaceId: workspace.id, project, first, second, agentRunsBefore }, null, 2));
  } finally { await runtime?.close(); }
} else {
  const seed = JSON.parse(await readFile(join(root, 'worktree-dirty-seed.json'), 'utf8'));
  assert.equal(await readFile(join(seed.project, 'identity.txt'), 'utf8'), 'ROOT\n');
  const common = git(seed.project, 'rev-parse', '--git-common-dir');
  const branches = git(seed.project, 'branch', '--format=%(refname:short)').split(/\r?\n/);
  for (const item of [seed.first, seed.second]) {
    assert.equal(await readFile(join(item.path, 'identity.txt'), 'utf8'), item.content);
    assert.equal(git(item.path, 'branch', '--show-current'), item.branch);
    assert.ok(branches.includes(item.branch));
    assert.ok(git(item.path, 'status', '--porcelain').includes('identity.txt'));
    // Compare Git's resolved common directory across both isolated checkouts.
    assert.equal(git(item.path, 'rev-parse', '--path-format=absolute', '--git-common-dir'),
      git(seed.project, 'rev-parse', '--path-format=absolute', '--git-common-dir'));
  }
  assert.equal(runs(), seed.agentRunsBefore);
  await writeFile(join(root, `worktree-dirty-${mode}.json`), JSON.stringify({
    workspaceId: seed.workspaceId, first: seed.first, second: seed.second,
    rootUnchanged: true, bothChangesRetained: true, bothBranchesRetained: true,
    commonDirectory: common, agentRunsBefore: seed.agentRunsBefore, agentRunsAfter: runs(),
  }, null, 2));
}
