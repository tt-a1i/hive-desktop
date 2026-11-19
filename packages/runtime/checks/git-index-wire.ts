/** Actual desktop JSONL, Hive workspace binding and Git index; no Agent/model. */
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const directory = await mkdtemp(join(tmpdir(), 'hive-git-index-wire-'));
const repo = join(directory, '项目 repo'), outside = join(directory, 'other repo');
const path = process.platform === 'win32' ? '文件 quoted - name.txt' : '文件\nquoted - name.txt';
await Promise.all([repo, outside, join(directory, 'agent')].map(path => mkdir(path)));
const config = join(directory, 'gitconfig'); await writeFile(config, '');
const env = {...process.env};
for (const key of Object.keys(env)) if (key.startsWith('GIT_')) delete env[key];
Object.assign(env, {GIT_CONFIG_GLOBAL: config, GIT_CONFIG_NOSYSTEM: '1', GIT_TERMINAL_PROMPT: '0'});
const git = (cwd: string, ...args: string[]) => execFileSync('git', ['-C', cwd,
  '-c', `core.hooksPath=${join(directory, 'no-hooks')}`, '-c', 'core.fsmonitor=false', '-c', 'core.autocrlf=false',
  '-c', 'user.name=Hive fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {env, encoding: 'utf8'});
for (const cwd of [repo, outside]) {
  git(cwd, 'init', '--quiet', '--initial-branch=main');
  await writeFile(join(cwd, '.gitignore'), '.hive/\n');
  await writeFile(join(cwd, 'keep.txt'), 'baseline keep\n');
  await writeFile(join(cwd, path), 'baseline selected\n');
  git(cwd, 'add', '.gitignore', 'keep.txt', path);
  git(cwd, 'commit', '--quiet', '-m', 'baseline');
}
await writeFile(join(repo, path), 'first review\n');
await writeFile(join(repo, 'keep.txt'), 'unselected edit\n');
const originalIndex = await readFile(join(repo, '.git/index'));
const outsideIndex = await readFile(join(outside, '.git/index'));
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const child = spawn(process.execPath, [entry], {cwd: directory, stdio: ['pipe', 'pipe', 'pipe'],
  env: {...env, HIVE_DESKTOP_DATA_DIR: join(directory, 'state'), PI_CODING_AGENT_DIR: join(directory, 'agent'), PI_OFFLINE: '1'}});
const exited = once(child, 'exit');
let diagnostics = '', sequence = 0;
child.stderr.on('data', chunk => {diagnostics += String(chunk);});
const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
const reader = createInterface({input: child.stdout});
reader.on('line', line => {
  const response = JSON.parse(line), request = pending.get(response.id);
  if (request) {pending.delete(response.id); request.resolve(response);}
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + diagnostics));
  pending.clear();
});
async function rpc(method: string, params: Record<string, unknown> = {}) {
  const response = await new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => {pending.delete(id); reject(new Error(`Timed out: ${method}`));}, 20_000);
    pending.set(id, {resolve: value => {clearTimeout(timer); resolve(value);}, reject: error => {clearTimeout(timer); reject(error);}});
    child.stdin.write(JSON.stringify({id, method, params}) + '\n');
  });
  if (response.error) throw new Error(response.error.message);
  return response.result;
}
try {
  await rpc('initialize');
  const workspace = await rpc('hive.request', {method: 'POST', path: '/api/workspaces',
    body: {path: repo, name: 'Git index wire', autostart_orchestrator: false}});
  const params = {workspaceId: workspace.id, operation: 'stage', paths: [path], projectRoot: outside, gitExecutable: 'ignored'};
  const first = await rpc('git.index.preview', params);
  assert.equal(first.ok, true); assert.equal(first.preview.workspace.projectRoot, await realpath(repo));
  assert.match(first.preview.version, /^[a-f0-9]{64}$/);
  for (const bad of [{...params, paths: undefined}, {...params, paths: ['../escape']}, {...params, operation: null}]) {
    const invalid = await rpc('git.index.preview', bad);
    assert.equal(invalid.ok, false);
    assert.ok(['INVALID_SELECTION', 'INVALID_ARGUMENT'].includes(invalid.error.code));
  }
  assert.deepEqual(await readFile(join(repo, '.git/index')), originalIndex, 'Previews and malformed requests must not stage anything');
  await writeFile(join(repo, path), 'changed after preview\n');
  const stale = await rpc('git.index.apply', {...params, expectedVersion: first.preview.version});
  assert.equal(stale.ok, false); assert.equal(stale.error.code, 'STALE_PREVIEW'); assert.equal(stale.applied, false);
  assert.deepEqual(await readFile(join(repo, '.git/index')), originalIndex);
  const fresh = await rpc('git.index.preview', params);
  const staged = await rpc('git.index.apply', {...params, expectedVersion: fresh.preview.version});
  assert.equal(staged.ok, true);
  const offPage = await rpc('git.status', {workspaceId: workspace.id, offset: 100, limit: 1});
  assert.equal(offPage.ok, true);
  assert.deepEqual(offPage.status.entries, []);
  const stagedPaths = git(repo, 'diff', '--cached', '--name-only', '-z').split('\0').filter(Boolean);
  assert.ok(stagedPaths.includes(path));
  assert.equal(offPage.status.repositoryStagedTotal, stagedPaths.length);

  assert.equal(git(repo, 'show', `:${path}`), 'changed after preview\n');
  assert.equal(git(repo, 'show', ':keep.txt'), 'baseline keep\n');
  const repeated = await rpc('git.index.apply', {...params, expectedVersion: fresh.preview.version});
  assert.equal(repeated.ok, false); assert.equal(repeated.error.code, 'STALE_PREVIEW');
  const unstage = {...params, operation: 'unstage'};
  const reviewed = await rpc('git.index.preview', unstage);
  assert.equal((await rpc('git.index.apply', {...unstage, expectedVersion: reviewed.preview.version})).ok, true);
  assert.equal(git(repo, 'show', `:${path}`), 'baseline selected\n');
  assert.equal(await readFile(join(repo, path), 'utf8'), 'changed after preview\n');
  assert.equal(await readFile(join(repo, 'keep.txt'), 'utf8'), 'unselected edit\n');
  assert.deepEqual(await readFile(join(outside, '.git/index')), outsideIndex, 'Caller-provided roots cannot redirect writes');
  assert.deepEqual(await rpc('chat.list', {workspaceId: workspace.id}), []);
  for (const [key, value] of [['user.name', 'Hive wire fixture'], ['user.email', 'fixture@example.test'],
    ['commit.gpgSign', 'false'], ['core.hooksPath', join(directory, 'no-hooks')]]) git(repo, 'config', key!, value!);
  const toCommit = await rpc('git.index.preview', params);
  assert.equal((await rpc('git.index.apply', {...params, expectedVersion: toCommit.preview.version})).ok, true);
  const commit = await rpc('git.commit.preview', {workspaceId: workspace.id});
  assert.equal(commit.ok, true); assert.ok(commit.preview.changes.some((item: any) => item.repositoryPath === path));
  assert.deepEqual(commit.preview.outsideProjectPaths, []);
  const summary = await rpc('git.commit.preview', {workspaceId: workspace.id, includePatch: false});
  assert.equal(summary.ok, true);
  assert.deepEqual(summary.preview, {...commit.preview, patch: ''});
  const patch = await rpc('git.commit.preview', {workspaceId: workspace.id, includePatch: true, expectedVersion: summary.preview.version});
  assert.equal(patch.ok, true);
  assert.equal(patch.preview.version, summary.preview.version);
  assert.equal(patch.preview.patch, git(repo, 'diff', '--cached', '--no-color', '--no-ext-diff', '--no-textconv', 'HEAD', '--'));
  const oldHead = git(repo, 'rev-parse', 'HEAD').trim();
  const malformed = await rpc('git.commit.apply', {workspaceId: workspace.id, expectedVersion: commit.preview.version,
    message: 'must not commit', includeOutsideProject: 'yes'});
  assert.equal(malformed.ok, false); assert.equal(malformed.committed, false); assert.equal(malformed.applied, false);
  assert.equal(git(repo, 'rev-parse', 'HEAD').trim(), oldHead);
  await writeFile(join(repo, path), 'new selected stage\n');
  const newStage = await rpc('git.index.preview', params);
  assert.equal((await rpc('git.index.apply', {...params, expectedVersion: newStage.preview.version})).ok, true);
  const obsoletePatch = await rpc('git.commit.preview', {workspaceId: workspace.id, includePatch: true, expectedVersion: summary.preview.version});
  assert.equal(obsoletePatch.ok, false);
  assert.equal(obsoletePatch.error.code, 'STALE_PREVIEW');
  assert.equal(obsoletePatch.preview, undefined);
  const obsolete = await rpc('git.commit.apply', {workspaceId: workspace.id, expectedVersion: commit.preview.version, message: 'stale commit'});
  assert.equal(obsolete.ok, false); assert.equal(obsolete.committed, false); assert.equal(obsolete.error.code, 'STALE_PREVIEW');
  assert.equal(git(repo, 'rev-parse', 'HEAD').trim(), oldHead);
  const reviewedCommit = await rpc('git.commit.preview', {workspaceId: workspace.id});
  const created = await rpc('git.commit.apply', {workspaceId: workspace.id, expectedVersion: reviewedCommit.preview.version, message: 'Wire fixture commit\n\n中文说明'});
  assert.equal(created.ok, true); assert.equal(created.committed, true); assert.equal(created.applied, true); assert.equal(created.indexUpdated, true);
  assert.equal(created.commitOid, git(repo, 'rev-parse', 'HEAD').trim());
  assert.equal(git(repo, 'show', `HEAD:${path}`), 'new selected stage\n');
  assert.equal(git(repo, 'show', 'HEAD:keep.txt'), 'baseline keep\n');
  assert.equal(await readFile(join(repo, 'keep.txt'), 'utf8'), 'unselected edit\n');
  const isolated = await rpc('chat.createRecord', {workspaceId: workspace.id, backend: 'codex', isolation: 'worktree'});
  const isolatedPath = isolated.execution.path;
  await writeFile(join(isolatedPath, 'keep.txt'), 'isolated staged file\n');
  const parentIndex = await readFile(join(repo, '.git/index'));
  const scoped = {workspaceId: workspace.id, sessionId: isolated.id, operation: 'stage', paths: ['keep.txt']};
  const scopedPreview = await rpc('git.index.preview', scoped);
  assert.equal(scopedPreview.ok, true); assert.equal(scopedPreview.preview.workspace.projectRoot, await realpath(isolatedPath));
  assert.equal((await rpc('git.index.apply', {...scoped, expectedVersion: scopedPreview.preview.version})).ok, true);
  assert.equal(git(isolatedPath, 'show', ':keep.txt'), 'isolated staged file\n');
  assert.deepEqual(await readFile(join(repo, '.git/index')), parentIndex, 'Session-bound staging must preserve the team root index');
  assert.deepEqual(await readFile(join(outside, '.git/index')), outsideIndex);
  await rpc('shutdown');
  assert.equal((await exited)[0], 0, diagnostics);
  console.log('GIT_INDEX_WIRE_OK: actual JSONL/Hive/Git, guarded stage/unstage/native commit, full result fields, authoritative session worktree, untouched disk/other index; no Agent.');
} finally {
  if (child.exitCode === null && child.signalCode === null) {child.kill(); await exited;}
  reader.close();
  await rm(directory, {recursive: true, force: true});
}
