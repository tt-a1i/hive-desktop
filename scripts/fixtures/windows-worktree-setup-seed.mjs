// Private Git/SQLite fixture. Uses the installed bundle's unmodified runtime.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { access, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project'), pi = join(root, 'offline-pi');
const seedFile = join(root, 'worktree-setup-seed.json');
const attempts = join(root, 'attempts'), draft = 'Original worktree setup draft 7531';
const dist = join(resources, 'runtime', 'dist');
const worker = fileURLToPath(new URL('./windows-worktree-setup-worker.cjs', import.meta.url));
const environment = { GIT_CONFIG_GLOBAL: join(root, 'empty.gitconfig'), GIT_CONFIG_NOSYSTEM: '1', GIT_OPTIONAL_LOCKS: '0' };
Object.assign(process.env, environment, { HIVE_DESKTOP_DATA_DIR: data, PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1' });
const git = (cwd, ...args) => execFileSync('git', ['-C', cwd, ...args], { encoding: 'utf8' }).trim();
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const readJson = async path => JSON.parse(await readFile(path, 'utf8'));
const exists = async path => { try { await access(path); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } };
const samePath = async (left, right) => assert.equal(await realpath(left), await realpath(right));
const isWindows = process.platform === 'win32';
const quote = arg => {
  if (isWindows) {
    assert.ok(!/["%!\r\n]/.test(arg), 'Path cannot be safely quoted in cmd.exe');
    return `"${arg}"`;
  }
  return "'" + arg.replaceAll("'", "'\\''") + "'";
};

async function makeProject(path) {
  await mkdir(path);
  git(path, 'init', '--template=', '-b', 'main');
  await writeFile(join(path, 'fixture.txt'), 'hive setup fixture\n');
  git(path, 'add', 'fixture.txt');
  git(path, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgSign=false', 'commit', '-qm', 'Private setup fixture');
}

function agentRuns(directory) {
  const db = new DatabaseSync(join(directory, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT run_id, agent_id, pid, status FROM agent_runs').all(); }
  finally { db.close(); }
}

function commandFor(directory) {
  const cmd = [process.execPath, worker, directory].map(quote).join(' ');
  // Keep the cmd payload from starting with a quoted executable.
  return isWindows ? 'ver >nul && ' + cmd : cmd;
}

async function until(read, accept, description) {
  const deadline = Date.now() + 15000;
  do {
    const value = await read();
    if (accept(value)) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  throw new Error('Timed out waiting for ' + description);
}

async function checkWire() {
  const directory = join(root, 'wire'), wireProject = join(directory, 'project'), wireAttempts = join(directory, 'attempts');
  await mkdir(directory); await mkdir(wireAttempts); await makeProject(wireProject);
  const wireData = join(directory, 'data');
  const child = spawn(process.execPath, [join(dist, 'main.js')], {
    cwd: wireProject,
    env: { ...process.env, HIVE_DESKTOP_DATA_DIR: wireData },
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });
  const exited = once(child, 'close');
  void exited.catch(() => {});
  const out = createInterface({ input: child.stdout });
  const responses = new Map();
  let stderr = '', sequence = 0;
  child.stderr.setEncoding('utf8').on('data', chunk => { stderr += chunk; });
  child.stdin.on('error', () => {}); // A broken pipe is reported by the child exit check.
  out.on('line', line => {
    const response = JSON.parse(line);
    if (typeof response.id === 'number') responses.set(response.id, response);
  });
  async function call(method, params = {}) {
    const id = ++sequence;
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
    const response = await until(() => {
      if (!responses.has(id) && child.exitCode !== null) throw new Error(`Runtime exited during ${method}\n${stderr}`);
      return responses.get(id);
    }, Boolean, method);
    responses.delete(id);
    return response;
  }
  async function rpc(method, params) {
    const response = await call(method, params);
    assert.equal(response.error, undefined, `${method}: ${JSON.stringify(response.error)}`);
    return response.result;
  }
  try {
    assert.ok((await rpc('initialize')).hivePort > 0);
    const workspace = await rpc('hive.request', { method: 'POST', path: '/api/workspaces', body: { path: wireProject, name: 'Setup RPC QA', autostart_orchestrator: false } });
    await rpc('script.settings.save', { workspaceId: workspace.id, expectedRevision: '0', script: { name: 'Wire setup', command: commandFor(wireAttempts), runOnWorktreeCreate: true } });
    const chat = await rpc('chat.createRecord', { workspaceId: workspace.id, backend: 'pi', isolation: 'worktree' });
    const params = { workspaceId: workspace.id, sessionId: chat.id };
    const setup = () => rpc('worktree.initialization.get', params);
    const reportFile = join(wireAttempts, '1', 'process.json');
    await until(() => exists(reportFile), Boolean, 'running saved command');
    const report = await readJson(reportFile);
    await samePath(report.cwd, chat.execution.path);
    const outcomes = {};
    let nativeOwnership;
    for (const status of ['running', 'failed']) {
      await until(setup, value => value?.status === status, 'setup ' + status);
      const opened = await rpc('chat.open', { sessionId: chat.id });
      assert.equal(opened.readOnly, true);
      assert.equal(opened.identityError.code, 'WORKTREE_INITIALIZATION_BLOCKED');
      assert.equal(opened.initialization.status, status);
      assert.equal(opened.pending, 0);
      outcomes[status] = {};
      for (const method of ['chat.prompt', 'chat.resume']) {
        const response = await call(method, { sessionId: chat.id, ...(method === 'chat.prompt' && { text: 'Blocked prompt 7119' }) });
        assert.equal(response.error?.code, 'WORKTREE_INITIALIZATION_BLOCKED', `${status} ${method}: ${JSON.stringify(response)}`);
        outcomes[status][method] = response.error.code;
      }
      if (status === 'running') {
        if (isWindows) {
          const [run] = (await rpc('script.runs', { workspaceId: workspace.id })).runs;
          const native = await rpc('hive.request', { method: 'GET', path: `/api/runtime/runs/${run.nativeRunId}` });
          assert.equal(native.agent_id, `${workspace.id}:script:${run.runId}:shell`);
          assert.ok(Number.isSafeInteger(report.pid) && report.pid > 0);
          const query = `$row=Get-CimInstance Win32_Process -Filter "ProcessId=${report.pid}"; if (!$row) {exit 72}; [Console]::Write($row.ParentProcessId)`;
          const shellPid = Number(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', query], {
            encoding: 'utf8', windowsHide: true, timeout: 20000,
          }).trim());
          assert.equal(native.pid, shellPid);
          nativeOwnership = { nativeRunId: run.nativeRunId, shellPid, workerPid: report.pid };
        }
        await writeFile(join(wireAttempts, '1', 'release'), '', { flag: 'wx' });
      }
    }
    assert.deepEqual(await readdir(wireAttempts), ['1']);
    const runs = (await rpc('script.runs', { workspaceId: workspace.id })).runs;
    assert.equal(runs.length, 1);
    assert.equal(runs[0].exitCode, 23);
    assert.deepEqual(agentRuns(wireData).filter(run => !run.agent_id.endsWith(':shell')), []);
    await rpc('shutdown');
    assert.deepEqual(await exited, [0, null]);
    return { outcomes, setupExitCode: 23, normalShutdown: true, agentRuns: 0, nativeOwnership };
  } finally {
    child.stdin.end();
    out.close();
    // Let EOF invoke the production shutdown path if an assertion failed.
    const timer = setTimeout(() => { if (child.exitCode === null) child.kill(); }, 20000);
    try { await exited; } finally { clearTimeout(timer); }
  }
}

if (mode === 'seed') {
  await Promise.all([data, pi, attempts].map(path => mkdir(path, { recursive: true })));
  await writeFile(environment.GIT_CONFIG_GLOBAL, '');
  await makeProject(project);
  await writeFile(join(pi, 'models.json'), JSON.stringify({ providers: { 'local-fixture': {
    api: 'openai-responses', baseUrl: 'http://127.0.0.1:9/v1', apiKey: 'fixture-placeholder', models: [{ id: 'setup-qa', name: 'Local setup fixture', input: ['text'] }],
  } } }));
  await writeFile(join(pi, 'settings.json'), JSON.stringify({ defaultProvider: 'local-fixture', defaultModel: 'setup-qa' }));
  const wire = await checkWire();
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const runtime = await createDesktopRuntime(data, () => {});
  try {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Worktree setup QA', autostart_orchestrator: false });
    const settings = await runtime.projectScripts.save(workspace.id, '0', {
      name: 'Windows setup fixture', command: commandFor(attempts), runOnWorktreeCreate: true,
    });
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: {} }));
    const drafts = join(data, 'desktop-drafts');
    await mkdir(drafts, { recursive: true });
    await writeFile(join(drafts, `workspace-${workspace.id}.json`), JSON.stringify({ key: `workspace:${workspace.id}`, input: draft, failed: [] }));
    await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
    await writeFile(seedFile, JSON.stringify({ root, project, workspaceId: workspace.id, script: settings.scripts[0],
      settingsRevision: settings.revision, head: git(project, 'rev-parse', 'HEAD'), draft, environment, wire }));
  } finally { await runtime.close(); }
} else {
  const fixture = await readJson(seedFile);
  const worktrees = join(data, 'worktrees'), ledgers = join(data, 'project-script-runs');
  const names = async path => await readdir(path).catch(error => { if (error.code === 'ENOENT') return []; throw error; });
  const intents = await Promise.all((await names(worktrees)).map(id => readJson(join(worktrees, id, 'intent.json'))));
  const ledgersJson = await Promise.all((await names(ledgers)).filter(name => name.endsWith('.json')).map(name => readJson(join(ledgers, name))));
  const runs = ledgersJson.sort((a, b) => a.startedAt - b.startedAt);
  const receipt = { mode, worktrees: intents.map(x => x.record?.chatId), runs: runs.map(({ status, exitCode, runId, requestId }) => ({ status, exitCode, runId, requestId })), attempts: await names(attempts), agentRuns: agentRuns(data) };
  if (mode !== 'inspect') {
    assert.equal(intents.length, 1);
    const owner = intents[0], record = owner.record, worktree = record.executionPath, sessionId = record.chatId;
    assert.equal(owner.state, 'ready'); assert.ok(record.ownerToken);
    assert.equal(record.baseOid, fixture.head);
    assert.notEqual(resolve(project), resolve(worktree));
    assert.equal(git(worktree, 'branch', '--show-current'), record.branch);
    assert.equal(git(worktree, 'rev-parse', 'HEAD'), fixture.head);
    assert.equal(git(project, 'rev-parse', 'HEAD'), fixture.head);
    // Hive opens the private workspace by writing its managed .hive files.
    assert.equal(git(project, 'status', '--porcelain', '--', '.', ':(exclude).hive'), '');
    assert.equal(await readFile(join(project, 'fixture.txt'), 'utf8'), 'hive setup fixture\n');
    assert.equal(await readFile(join(worktree, 'fixture.txt'), 'utf8'), 'hive setup fixture\n');
    const preserved = await readJson(join(data, 'desktop-drafts', `session-${sessionId}.json`));
    assert.equal(preserved.input, draft);
    assert.deepEqual(preserved.failed, []);
    const expected = mode === 'create' ? [23] : [23, 0];
    assert.deepEqual(runs.map(run => run.exitCode), expected);
    assert.deepEqual(receipt.attempts.sort(), expected.map((_, i) => String(i + 1)));
    for (const [index, run] of runs.entries()) {
      assert.equal(run.workspaceId, fixture.workspaceId); assert.equal(run.sessionId, sessionId);
      assert.deepEqual(run.script, fixture.script);
      assert.equal(run.status, index === 0 ? 'failed' : 'succeeded');
      await samePath(run.cwd, worktree);
      const attempt = join(attempts, String(index + 1));
      const workerReport = await readJson(join(attempt, 'process.json'));
      await samePath(workerReport.cwd, worktree);
      if (isWindows) {
        const processObservation = await readJson(join(attempt, 'observed.json'));
        assert.equal(processObservation.workerPid, workerReport.pid);
        assert.notEqual(processObservation.cmdPid, processObservation.workerPid);
        assert.equal(processObservation.started.length, 2);
      }
      assert.ok(run.nativeRunId);
      const log = await readFile(join(ledgers, run.runId + '.log'), 'utf8');
      assert.match(log, index === 0 ? /FIXTURE_SETUP_FAILED_23/ : /FIXTURE_SETUP_SUCCEEDED/);
    }
    const result = join(worktree, '.hive-setup-fixture-result');
    if (mode === 'create') {
      assert.equal(await exists(result), false);
      // Simulate a later saved edit before the cold reopen. Retry must run the frozen command.
      const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
      const runtime = await createDesktopRuntime(data, () => {});
      try {
        const { revision, scripts } = await runtime.projectScripts.save(fixture.workspaceId, fixture.settingsRevision, {
          id: fixture.script.id, name: fixture.script.name, command: isWindows ? 'exit /b 35' : 'exit 35', runOnWorktreeCreate: true,
        });
        assert.notEqual(revision, fixture.settingsRevision);
        assert.notEqual(scripts[0].version, fixture.script.version);
      } finally { await runtime.close(); }
    } else {
      assert.notEqual(runs[1].requestId, runs[0].requestId);
      assert.equal(await readFile(result, 'utf8'), 'completed once\n');
    }
    const activeSetup = await readJson(join(data, 'worktree-initialization', digest(JSON.stringify([fixture.workspaceId, sessionId])) + '.json'));
    assert.equal(activeSetup.status, runs.at(-1).status);
    assert.equal(activeSetup.runId, runs.at(-1).runId);
    assert.deepEqual(receipt.agentRuns.filter(run => !run.agent_id.endsWith(':shell')), []);
    Object.assign(receipt, { sessionId, branch: record.branch, head: fixture.head, originalDraftPreserved: true, capturedCommandVerified: true, rootUnchanged: true });
  }
  await writeFile(join(root, `worktree-setup-${mode}.json`), JSON.stringify(receipt, null, 2));
}
