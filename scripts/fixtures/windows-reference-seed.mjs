// Local Git and bundled runtime only. No model or remote is used.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'),
  pi = join(root, 'offline-pi'),
  project = join(root, 'project');
const relativePath = 'shared-reference.txt',
  mainContent = 'MAIN_ROOT_CONTENT\n',
  snapshotContent = 'WORKTREE_SNAPSHOT_CONTENT\n';
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
Object.assign(process.env, {
  HIVE_DESKTOP_DATA_DIR: data,
  PI_CODING_AGENT_DIR: pi,
  PI_OFFLINE: '1',
  GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null',
  GIT_CONFIG_NOSYSTEM: '1',
});
const git = (...args) =>
  execFileSync('git', ['-C', project, ...args], { stdio: 'pipe', env: process.env });
const { createDesktopRuntime } = await import(
  pathToFileURL(join(resources, 'runtime', 'dist', 'desktop-runtime.js')).href
);
let runtime;
try {
  if (mode === 'seed') {
    await Promise.all([project, data, pi].map((path) => mkdir(path, { recursive: true })));
    git('init', '--template=', '-q');
    git('config', 'user.name', 'Fixture');
    git('config', 'user.email', 'fixture@example.invalid');
    git('config', 'commit.gpgSign', 'false');
    await writeFile(join(project, relativePath), mainContent);
    git('add', relativePath);
    git('commit', '-qm', 'fixture');
    runtime = await createDesktopRuntime(data, () => {});
    const workspace = await runtime.hive.request('POST', '/api/workspaces', {
      path: project,
      name: 'Reference fixture',
      autostart_orchestrator: false,
    });
    const chat = await runtime.chats.create(workspace.id, 'pi', { isolation: 'worktree' });
    runtime.chats.rename(chat.id, 'Worktree reference');
    const execution = await runtime.chats.sessionExecution(chat.id);
    assert.notEqual(execution.path, project);
    await writeFile(join(execution.path, relativePath), snapshotContent);
    runtime.hive.saveNavigation(
      JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: chat.id } }),
    );
    await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
    await writeFile(
      join(root, 'reference-seed.json'),
      JSON.stringify({
        workspaceId: workspace.id,
        sessionId: chat.id,
        project,
        worktree: execution.path,
        relativePath,
        sha256: digest(snapshotContent),
      }),
    );
  } else {
    const seed = JSON.parse(await readFile(join(root, 'reference-seed.json'), 'utf8'));
    const draft = JSON.parse(
      await readFile(join(data, 'desktop-drafts', `session-${seed.sessionId}.json`), 'utf8'),
    );
    assert.equal(draft.key, `session:${seed.sessionId}`);
    assert.equal(draft.attachments.length, 1);
    const attachment = draft.attachments[0];
    assert.equal(attachment.projectFile.relativePath, relativePath);
    assert.equal(attachment.projectFile.sourceSessionId, seed.sessionId);
    assert.equal(attachment.projectFile.sha256, seed.sha256);
    assert.equal(await readFile(join(project, relativePath), 'utf8'), mainContent);
    runtime = await createDesktopRuntime(data, () => {});
    const [copy] = await runtime.chats.attachments.get(seed.workspaceId, [attachment.id]);
    assert.equal(await readFile(copy.filePath, 'utf8'), snapshotContent);
    const status = await runtime.chats.projectFileStatus(seed.workspaceId, attachment.id);
    assert.equal(status.state, mode === 'after' ? 'missing' : 'unchanged');
    if (mode === 'before')
      await rename(join(seed.worktree, relativePath), join(seed.worktree, 'moved-reference.txt'));
    await writeFile(
      join(root, `reference-${mode}.json`),
      JSON.stringify({
        sessionId: seed.sessionId,
        reference: attachment.projectFile,
        sha256: digest(await readFile(copy.filePath)),
        rootFilePreserved: true,
        status: status.state,
      }),
    );
  }
} finally {
  await runtime?.close();
}
