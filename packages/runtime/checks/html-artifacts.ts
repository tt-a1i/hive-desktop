import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readdir, realpath, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-html-artifacts-')));
const data = join(root, 'state');
const project = join(root, 'Project with spaces');
const other = join(root, 'Other project');
const agent = join(root, 'pi');
for (const directory of [project, other, agent]) await mkdir(directory);
const gitConfig = join(root, 'gitconfig');
await writeFile(gitConfig, '');
process.env.PI_CODING_AGENT_DIR = agent;
process.env.PI_OFFLINE = '1';
process.env.GIT_CONFIG_GLOBAL = gitConfig;
process.env.GIT_CONFIG_NOSYSTEM = '1';
const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
  '-c', 'commit.gpgSign=false', ...args], { cwd: project, stdio: 'pipe' });
const artifact = '产物.HTML';
const projectHtml = '<button>Project report</button>';
const worktreeHtml = '<button onclick="this.textContent=\'updated\'">Worktree report</button>';
await writeFile(join(project, artifact), projectHtml);
git('init', '--template=');
git('add', '.');
git('commit', '-qm', 'base');

let runtime = await createDesktopRuntime(data, () => {});
try {
  const createWorkspace = (path: string, name: string) => runtime.hive.request('POST', '/api/workspaces', {
    path, name, autostart_orchestrator: false,
  }) as Promise<{id: string}>;
  const workspace = await createWorkspace(project, 'HTML provenance');
  const otherWorkspace = await createWorkspace(other, 'Other');
  const shared = await runtime.chats.createRecord(workspace.id, 'codex');
  const isolated = await runtime.chats.createRecord(workspace.id, 'codex', { isolation: 'worktree' });
  const execution = await runtime.chats.sessionExecution(isolated.id);
  const worktreeFile = join(execution.path, artifact);
  await writeFile(worktreeFile, worktreeHtml);
  const request = { path: artifact, sourceSessionId: isolated.id };
  const open = () => runtime.chats.artifactHtmlSnapshot(workspace.id, request);
  const [first, second] = await Promise.all([open(), open()]);
  assert.deepEqual(second, first);
  assert.equal(first.html, worktreeHtml);
  assert.equal(first.sha256, createHash('sha256').update(worktreeHtml).digest('hex'));
  assert.equal(first.sourceSessionId, isolated.id);
  assert.equal((await runtime.chats.artifactHtmlSnapshot(workspace.id, { ...request, sourceSessionId: shared.id })).html, projectHtml);
  await assert.rejects(runtime.chats.artifactHtmlSnapshot(otherWorkspace.id, request), /ARTIFACT_SOURCE_MISMATCH/);
  await assert.rejects(runtime.chats.artifactHtmlSnapshot(workspace.id, { ...request, path: join(other, 'outside.html') }), /ARTIFACT_OUTSIDE_SOURCE/);
  await assert.rejects(runtime.chats.artifactHtmlSnapshot(workspace.id, { ...request, path: 'not-html.txt' }), /HTML_ARTIFACT_INVALID_TYPE/);
  await writeFile(join(execution.path, 'large.html'), Buffer.alloc(2 * 1024 * 1024 + 1, 65));
  await assert.rejects(runtime.chats.artifactHtmlSnapshot(workspace.id, { ...request, path: 'large.html' }), /HTML_ARTIFACT_TOO_LARGE/);
  await writeFile(join(execution.path, 'invalid.html'), Buffer.from([0xff, 0x00]));
  await assert.rejects(runtime.chats.artifactHtmlSnapshot(workspace.id, { ...request, path: 'invalid.html' }), /HTML_ARTIFACT_ENCODING/);
  await writeFile(worktreeFile, '<p>Changed live artifact</p>');
  assert.deepEqual(await open(), first);

  await rm(join(execution.path, 'large.html'));
  await rm(join(execution.path, 'invalid.html'));
  git('-C', execution.path, 'add', '--', artifact);
  git('-C', execution.path, 'commit', '-qm', 'Keep the later HTML on the worktree branch');
  const remote = join(root, 'local-backup.git');
  git('init', '--bare', '--template=', remote);
  git('remote', 'add', 'local-backup', remote);
  git('-C', execution.path, 'push', '--set-upstream', 'local-backup', 'HEAD');
  await runtime.close();
  runtime = await createDesktopRuntime(data, () => {});
  const removal = await runtime.chats.previewWorktreeRemoval(workspace.id, isolated.id, async () => false);
  assert.equal(removal.verifiedRemote?.remote, 'local-backup');
  assert.deepEqual(await runtime.chats.applyWorktreeRemoval(workspace.id, isolated.id, removal.version, async () => false),
    {ok: true, workspaceId: workspace.id, sessionId: isolated.id});
  await assert.rejects(stat(execution.path), {code: 'ENOENT'});
  await runtime.close();
  runtime = await createDesktopRuntime(data, () => {});
  assert.deepEqual(await open(), first);
  await assert.rejects(runtime.chats.resolveArtifact(workspace.id, request), /ARTIFACT_SOURCE_UNAVAILABLE/);

  const workspaceDigest = createHash('sha256').update(workspace.id).digest('hex');
  const directory = join(data, 'html-artifacts', workspaceDigest);
  const entries = await readdir(directory);
  assert.equal(entries.length, 2);
  assert.ok(entries.every(name => /^[a-f\d]{64}\.json$/.test(name)));
  const snapshotKey = createHash('sha256').update(JSON.stringify([artifact, undefined, undefined, isolated.id])).digest('hex');
  await writeFile(join(directory, `${snapshotKey}.json`), JSON.stringify({ version: 1, ...first, html: '<p>Corrupted</p>' }));
  await assert.rejects(open(), /HTML_ARTIFACT_SNAPSHOT_INVALID/);
  console.log('HTML_ARTIFACTS_OK: real worktree provenance, workspace isolation, immutable snapshots, cold recovery, limits and integrity; no model requests.');
} finally {
  await runtime.close();
  await rm(root, { recursive: true, force: true, maxRetries: 4, retryDelay: 200 });
}
