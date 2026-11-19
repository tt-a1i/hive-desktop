import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { generateKeyPairSync, sign } from 'node:crypto';
import { once } from 'node:events';
import { createReadStream } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, readdir, realpath, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs, promisify } from 'node:util';

const { values: args } = parseArgs({ options: {
  bundle: { type: 'string' },
  'candidate-executable': { type: 'string' },
  'build-candidate': { type: 'boolean' },
  receipt: { type: 'string' },
} });
assert.equal(process.platform, 'win32', 'This check requires an interactive Windows desktop');
assert.ok(args.bundle && args.receipt && (Boolean(args['candidate-executable']) !== Boolean(args['build-candidate'])),
  'Pass --bundle, --receipt, and either --candidate-executable or --build-candidate');
const exec = promisify(execFile);
const repo = fileURLToPath(new URL('../', import.meta.url));
const controls = fileURLToPath(new URL('./windows-update-ui.ps1', import.meta.url));
const bundle = await realpath(args.bundle);
const baseExe = join(bundle, 'hive-desktop.exe');
const fromBundle = name => import(pathToFileURL(join(bundle, 'resources', 'runtime', 'dist', name)));
const { createDesktopRuntime } = await fromBundle('desktop-runtime.js');
const { hashDesktopFile, verifyDesktopPackage } = await fromBundle('desktop-update-archive.js');
const work = await realpath(await mkdtemp(join(tmpdir(), 'Hive native update ')));
const receipt = resolve(args.receipt);
const outcomes = [];
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + '\n');

async function nativeIdentity(executable) {
  const { stdout } = await exec(executable, ['--desktop-update-identity'], { timeout: 10_000, windowsHide: true });
  const identity = JSON.parse(stdout);
  assert.equal(identity.app, 'com.hive.desktop.dev');
  assert.equal(identity.schemaVersion, 1);
  assert.equal(identity.platform, process.platform);
  assert.equal(identity.arch, process.arch);
  assert.equal(identity.installerProtocol, 2);
  return identity;
}

async function buildCandidate(fromVersion) {
  assert.match(fromVersion, /^\d+\.\d+\.\d+$/);
  const parts = fromVersion.split('.');
  const nextVersion = [...parts.slice(0, 2), Number(parts[2]) + 1].join('.');
  const manifest = join(repo, 'apps', 'desktop', 'Cargo.toml');
  const lockfile = join(repo, 'apps', 'desktop', 'Cargo.lock');
  const manifestBytes = await readFile(manifest);
  const lockBytes = await readFile(lockfile);
  const versionLine = `version = "${fromVersion}"`;
  const nextLine = `version = "${nextVersion}"`;
  const manifestText = manifestBytes.toString();
  const lockText = lockBytes.toString();
  const lockEntry = `name = "hive-desktop"${lockText.includes('\r\n') ? '\r\n' : '\n'}${versionLine}`;
  assert.ok(manifestText.indexOf(versionLine) >= 0 && manifestText.indexOf(versionLine) < manifestText.indexOf('[dependencies]'));
  assert.equal(lockText.split(lockEntry).length, 2);
  const target = resolve(process.env.CARGO_TARGET_DIR ?? join(dirname(manifest), 'target'));
  const buildOutput = join(target, 'release', 'hive-desktop.exe');
  const savedOutput = join(work, 'original-build.exe');
  const hasOutput = await stat(buildOutput).then(() => true, error => {
    if (error.code === 'ENOENT') return false;
    throw error;
  });
  if (hasOutput) await cp(buildOutput, savedOutput);
  try {
    await writeFile(manifest, manifestText.replace(versionLine, nextLine));
    await writeFile(lockfile, lockText.replace(lockEntry, lockEntry.replace(versionLine, nextLine)));
    // Reuse CI's existing release dependency build and compile only one job at a time.
    console.log(`Building private update fixture ${nextVersion}`);
    const build = spawn('cargo', ['build', '--release', '--locked', '--jobs', '1', '--manifest-path', manifest, '--bin', 'hive-desktop'],
      { cwd: repo, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit' });
    const [code, signal] = await once(build, 'exit');
    assert.deepEqual([code, signal], [0, null]);
    const executable = join(work, 'candidate.exe');
    await cp(buildOutput, executable);
    return executable;
  } finally {
    await writeFile(manifest, manifestBytes);
    await writeFile(lockfile, lockBytes);
    if (hasOutput) await cp(savedOutput, buildOutput);
    else await rm(buildOutput, { force: true });
  }
}

async function countFiles(root) {
  let count = 0;
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (entry.isDirectory()) count += await countFiles(join(root, entry.name));
    else count++;
  }
  return count;
}

const keys = generateKeyPairSync('ed25519');
const publicKey = keys.publicKey.export({ format: 'der', type: 'spki' }).toString('base64');
const releases = new Map();
const server = createServer((request, response) => {
  const [, name, file] = (request.url ?? '').split('/');
  const release = releases.get(name);
  if (request.method === 'GET' && release && file === 'manifest.json') {
    response.end(release.manifest);
  } else if (request.method === 'GET' && release && file === 'update.zip') {
    response.writeHead(200, { 'content-length': release.bytes });
    const stream = createReadStream(release.archive);
    stream.on('error', error => response.destroy(error));
    response.on('close', () => stream.destroy());
    stream.pipe(response);
  } else response.writeHead(404).end();
});

async function preparePackage(root, executable, identity, feedUrl) {
  await cp(bundle, root, { recursive: true, verbatimSymlinks: true });
  const program = join(root, 'hive-desktop.exe');
  if (executable !== baseExe) await cp(executable, program);
  const resources = join(root, 'resources');
  await save(join(resources, 'desktop-update-channel.json'),
    { schemaVersion: 1, feedUrl, publicKey, installedVersion: identity.version });
  const identityPath = join(resources, 'desktop-update-identity.json');
  await rm(identityPath, { force: true });
  const executableSha256 = await hashDesktopFile(program);
  await save(identityPath, { ...identity, files: await countFiles(root) + 1, executableSha256 });
  await verifyDesktopPackage(root, identity.version, AbortSignal.timeout(60_000));
  return executableSha256;
}

async function archiveRelease(name, packageRoot, candidateIdentity) {
  const archive = join(work, name + '.zip');
  await exec('tar.exe', ['-a', '-cf', archive, '-C', packageRoot, 'hive-desktop.exe', 'resources'], { timeout: 150_000 });
  const bytes = (await stat(archive)).size;
  const sha256 = await hashDesktopFile(archive);
  const payload = Buffer.from(JSON.stringify({ schemaVersion: 1, version: candidateIdentity.version,
    assets: [{ platform: process.platform, arch: process.arch, file: 'update.zip', bytes, sha256 }] }));
  releases.set(name, { archive, bytes, sha256,
    manifest: JSON.stringify({ payload: payload.toString('base64'), signature: sign(null, payload, keys.privateKey).toString('base64') }) });
}

function ui(action, parameters, timeout = 35_000) {
  return exec('powershell.exe', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File',
    controls, '-Action', action, ...parameters], { timeout, windowsHide: true });
}
async function processes(directory) {
  const { stdout } = await ui('Processes', ['-Directory', directory]);
  return JSON.parse(stdout);
}
async function until(description, operation, timeout = 120_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const value = await operation();
    if (value) return value;
    await delay(200);
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function runCase(name, expectedState, current, candidate, candidateSha, baseUrl) {
  const directory = join(work, name), installation = join(directory, 'Hive Desktop');
  const data = join(directory, 'state'), project = join(directory, 'project'), pi = join(directory, 'pi');
  for (const folder of [project, pi, data]) await mkdir(folder, { recursive: true });
  const executable = join(installation, 'hive-desktop.exe');
  const oldSha = await preparePackage(installation, baseExe, current, `${baseUrl}/${name}/manifest.json`);
  process.env.PI_CODING_AGENT_DIR = pi;
  process.env.PI_OFFLINE = '1';
  let draftFile, session;
  const draft = `Unsaved input retained after the ${name} update`;
  const runtime = await createDesktopRuntime(data, () => {}, { executableEnvironment: {} });
  try {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Update verification', autostart_orchestrator: false });
    session = (await runtime.chats.createRecord(workspace.id, 'pi')).id;
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: session } }));
    const draftDirectory = join(data, 'desktop-drafts');
    await mkdir(draftDirectory, { recursive: true });
    draftFile = join(draftDirectory, `session-${session}.json`);
    await save(draftFile, { key: `session:${session}`, input: draft, failed: [] });
  } finally { await runtime.close(); }
  await save(join(data, 'desktop-language.json'), 'zh');
  const env = { ...process.env, HIVE_DESKTOP_DATA_DIR: data, HIVE_DESKTOP_TRACE_RPC: '1' };
  for (const key of ['HIVE_DESKTOP_RUNTIME', 'HIVE_DESKTOP_NODE', 'HIVE_DESKTOP_UPDATE_AUTHORIZE', 'HIVE_DESKTOP_UPDATE_READY']) delete env[key];
  const log = [];
  const original = spawn(executable, [], { env, cwd: project, stdio: ['ignore', 'ignore', 'pipe'] });
  original.stderr.on('data', chunk => log.push(chunk));
  const exit = once(original, 'exit');
  const details = { name, expectedState, originalPid: original.pid, phase: 'native update controls' };
  outcomes.push(details);
  try {
    await ui('Install', ['-AppProcessId', String(original.pid), '-Executable', executable, '-Draft', draft], 240_000);
    details.phase = 'installer receipt';
    const resultFile = join(data, 'desktop-updates', 'install-result.json');
    const result = await until(`${name} install receipt`, () => readFile(resultFile, 'utf8').then(JSON.parse, error => {
      if (error.code !== 'ENOENT') throw error;
    }));
    assert.deepEqual(await exit, [0, null]);
    const version = expectedState === 'installed' ? candidate.version : current.version;
    assert.equal(result.state, expectedState, JSON.stringify(result));
    assert.equal(result.version, version);
    assert.equal(result.requestedVersion, candidate.version);
    assert.equal((await nativeIdentity(executable)).version, version);
    const installedSha = await hashDesktopFile(executable);
    assert.equal(installedSha, expectedState === 'installed' ? candidateSha : oldSha);
    const transactions = (await readdir(directory)).filter(entry => entry.startsWith('.hive-desktop-install-'));
    assert.equal(transactions.length, 1);
    const plan = JSON.parse(await readFile(join(directory, transactions[0], 'install-plan.json'), 'utf8'));
    assert.equal(plan.oldExecutableSha256, oldSha);
    assert.equal(plan.newExecutableSha256, candidateSha);
    assert.equal(JSON.parse(await readFile(draftFile, 'utf8')).input, draft);
    details.phase = 'relaunched window and shutdown';
    const replacement = await until('relaunched native window', async () =>
      (await processes(directory)).find(item => item.pid !== original.pid && item.path.toLowerCase() === executable.toLowerCase() && item.window));
    await ui('VerifyAndClose', ['-AppProcessId', String(replacement.pid), '-Executable', executable,
      '-Draft', draft, '-ReceiptLabel', `${expectedState === 'installed' ? '已安装桌面更新' : '安装失败，已恢复旧版本'} · ${version}`]);
    await until('owned desktop and installer processes to exit', async () => !(await processes(directory)).length, 20_000);
    const trace = Buffer.concat(log).toString();
    assert.match(trace, /send .*"desktop\.updates\.check"/);
    assert.match(trace, /send .*"desktop\.updates\.stage"/);
    assert.match(trace, /send .*"desktop\.updates\.prepareInstaller"/);
    assert.match(trace, /reply 0 ok/);
    assert.doesNotMatch(trace, /"chat\.prompt"/);
    const release = releases.get(name);
    Object.assign(details, { phase: 'verified', result, oldSha, installedSha, archiveSha: release.sha256,
      archiveBytes: release.bytes, draftRetained: true, sameSession: session, allOwnedProcessesExited: true });
  } finally {
    details.trace = Buffer.concat(log).toString();
    await ui('StopFixture', ['-Directory', directory]).catch(error => console.error(error.message));
    if (original.exitCode === null && original.signalCode === null) original.kill();
  }
}

try {
  await mkdir(dirname(receipt), { recursive: true });
  const current = await nativeIdentity(baseExe);
  const executable = args['candidate-executable'] ? await realpath(args['candidate-executable']) : await buildCandidate(current.version);
  const candidate = await nativeIdentity(executable);
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const endpoint = `http://127.0.0.1:${server.address().port}`;
  const candidateRoot = join(work, 'candidate');
  const candidateSha = await preparePackage(candidateRoot, executable, candidate, `${endpoint}/healthy/manifest.json`);
  assert.notEqual(candidateSha, await hashDesktopFile(baseExe), 'The candidate must be a different native build');
  await archiveRelease('healthy', candidateRoot, candidate);
  await runCase('healthy', 'installed', current, candidate, candidateSha, endpoint);
  await rm(join(work, 'healthy'), { recursive: true, force: true });
  await rm(releases.get('healthy').archive);
  await writeFile(join(candidateRoot, 'resources', 'runtime', 'dist', 'main.js'), 'process.exit(79);\n');
  await archiveRelease('broken', candidateRoot, candidate);
  await rm(candidateRoot, { recursive: true, force: true });
  await runCase('broken', 'rolledBack', current, candidate, candidateSha, endpoint);
  await save(receipt, { ok: true, outcomes });
} catch (error) {
  await save(receipt, { ok: false, outcomes, error: error.stack ?? String(error) });
  throw error;
} finally {
  if (server.listening) {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
  await rm(work, { recursive: true, force: true, maxRetries: 5, retryDelay: 250 });
}
