import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DesktopUpdates } from '../src/desktop-updates.js';
import { desktopPackagePaths } from '../src/desktop-update-archive.js';

if (!['darwin', 'win32'].includes(process.platform)) {
  console.log('Desktop staging requires a supported package target');
  process.exit(0);
}

const root = await mkdtemp(join(tmpdir(), 'hive-update-channel-'));
const policyPath = pathToFileURL(join(root, 'channel.json'));
const staging = join(root, 'desktop-updates');
const archive = Buffer.from('bytes of the update fixture');
const sha256 = createHash('sha256').update(archive).digest('hex');
const asset = { platform: process.platform, arch: process.arch, file: 'fixture.zip', bytes: archive.length, sha256 };
const { publicKey, privateKey } = generateKeyPairSync('ed25519');
const envelope = (version: string, assets = [asset]) => {
  const payload = Buffer.from(JSON.stringify({ schemaVersion: 1, version, assets }));
  return { payload: payload.toString('base64'), signature: sign(null, payload, privateKey).toString('base64') };
};
let manifest = envelope('0.1.1');
let manifestsRequested = 0;
let unavailable = false;
let serve = (response: import('node:http').ServerResponse) => response.end(archive);
const server = createServer((request, response) => {
  if (request.url === '/feed/manifest.json') {
    manifestsRequested++;
    if (unavailable) return response.writeHead(503).end();
    response.end(JSON.stringify(manifest));
  }
  else if (request.url === '/feed/fixture.zip') serve(response);
  else response.writeHead(404).end();
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const socket = server.address();
assert.ok(socket && typeof socket !== 'string');
const updates = new DesktopUpdates(root, policyPath);
try {
  assert.deepEqual(await updates.check(), { state: 'disabled' });
  await writeFile(policyPath, JSON.stringify({ schemaVersion: 1, installedVersion: '0.1.0',
    feedUrl: `http://127.0.0.1:${socket.port}/feed/manifest.json`, publicKey: publicKey.export({type: 'spki', format: 'der'}).toString('base64') }));
  assert.deepEqual(await updates.check(), { state: 'available', installedVersion: '0.1.0', version: '0.1.1', asset });
  manifest = envelope('0.1.0');
  assert.deepEqual(await updates.check(), { state: 'current', installedVersion: '0.1.0' });
  manifest = envelope('0.2.0', []);
  assert.deepEqual(await updates.check(), { state: 'unsupported', installedVersion: '0.1.0', version: '0.2.0' });
  manifest = envelope('0.1.1', [asset, asset]);
  await assert.rejects(updates.check(), /duplicate targets/);
  manifest = envelope('0.1.1', [{ ...asset, file: '../foreign.zip' }]);
  await assert.rejects(updates.check(), /Invalid desktop update asset/);
  manifest = { ...envelope('0.1.1'), signature: Buffer.alloc(64).toString('base64') };
  await assert.rejects(updates.check(), /signature verification failed/);

  manifest = envelope('0.1.2');
  await assert.rejects(updates.stage('0.1.1', sha256), /changed/);
  let responses = 0;
  serve = response => { responses++; return response.end(Buffer.alloc(archive.length)); };
  await assert.rejects(updates.stage('0.1.2', sha256), /archive verification failed/);
  assert.equal(responses, 1);
  assert.deepEqual(await readdir(staging), []);
  serve = response => response.end(Buffer.alloc(archive.length + 1));
  await assert.rejects(updates.stage('0.1.2', sha256), /exceeded its expected size/);
  assert.deepEqual(await readdir(staging), []);
  serve = response => response.end(archive);
  assert.deepEqual(await updates.stage('0.1.2', sha256), { version: '0.1.2', sha256 });
  assert.deepEqual(await readFile(join(staging, `${sha256}.zip`)), archive);
  unavailable = true;
  const cold = new DesktopUpdates(root, policyPath);
  try {
    assert.deepEqual(await cold.check(), {state:'available', installedVersion:'0.1.0', version:'0.1.2', asset, staged:true});
  } finally {
    unavailable = false;
    await cold.close();
  }

  const fixture = join(root, 'fixture');
  const mac = process.platform === 'darwin';
  const packageRoot = mac ? join(fixture, 'Hive Desktop.app') : fixture;
  const paths = desktopPackagePaths(packageRoot);
  await mkdir(paths.resources, { recursive: true });
  for (const path of [paths.executable, paths.node, paths.runtime]) {
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, 'Owned inert preparation fixture.\n', { mode: 0o755 });
  }
  if (mac) await writeFile(join(packageRoot, 'Contents/Info.plist'), `<plist version="1.0"><dict>
    <key>CFBundleIdentifier</key><string>com.hive.desktop.dev</string>
    <key>CFBundleExecutable</key><string>hive-desktop</string>
    <key>CFBundleShortVersionString</key><string>0.1.3</string>
    <key>CFBundleVersion</key><string>0.1.3</string>
  </dict></plist>`);
  const executableSha256 = createHash('sha256').update(await readFile(paths.executable)).digest('hex');
  await writeFile(join(paths.resources, 'desktop-update-identity.json'), JSON.stringify({
    schemaVersion: 1, app: 'com.hive.desktop.dev', version: '0.1.3', platform: process.platform, arch: process.arch, executableSha256,
    files: mac ? 5 : 4,
  }));
  const zipped = join(root, 'fixture.zip');
  if (mac) execFileSync('/usr/bin/zip', ['-q', '-y', '-r', zipped, 'Hive Desktop.app'], { cwd: fixture });
  else execFileSync('tar.exe', ['-a', '-cf', zipped, '-C', packageRoot, 'hive-desktop.exe', 'resources'], {windowsHide: true});
  const zippedBytes = await readFile(zipped);
  const zippedHash = createHash('sha256').update(zippedBytes).digest('hex');
  const signedAsset = { ...asset, bytes: zippedBytes.length, sha256: zippedHash };
  manifest = envelope('0.1.3', [signedAsset]);
  serve = response => response.end(zippedBytes);
  await updates.stage('0.1.3', zippedHash);
  const beforePrepare = manifestsRequested;
  const prepared = await updates.prepare('0.1.3', zippedHash);
  assert.deepEqual(await readFile(desktopPackagePaths(prepared.packageRoot).executable), await readFile(paths.executable));
  assert.equal(manifestsRequested, beforePrepare, 'an attested staged update can be prepared offline');
  const original = await readFile(join(staging, `${zippedHash}.zip`));
  try {
    const tampered = Buffer.from(original);
    tampered[0] = tampered[0]! ^ 1;
    await writeFile(join(staging, `${zippedHash}.zip`), tampered);
    await assert.rejects(updates.prepare('0.1.3', zippedHash), /archive verification failed/);
  } finally {
    await writeFile(join(staging, `${zippedHash}.zip`), original);
  }
  assert.deepEqual((await readdir(staging)).filter(name => name.startsWith('prepare-')), [prepared.directory.split(/[\\/]/).at(-1)]);
  manifest = envelope('0.1.4', [signedAsset]);
  await updates.stage('0.1.4', zippedHash);
  await assert.rejects(updates.prepare('0.1.4', zippedHash), /identity does not match/);
  await assert.rejects(updates.prepare('0.1.3', zippedHash), /does not match the selected release/);
  if (mac) {
    await symlink('../../../../unowned.txt', join(paths.resources, 'unsafe-link'));
    const unsafeZip = join(root, 'unsafe.zip');
    execFileSync('/usr/bin/zip', ['-q', '-y', '-r', unsafeZip, 'Hive Desktop.app'], { cwd: fixture });
    const bytes = await readFile(unsafeZip);
    const hash = createHash('sha256').update(bytes).digest('hex');
    manifest = envelope('0.1.3', [{ ...signedAsset, bytes: bytes.length, sha256: hash }]);
    serve = response => response.end(bytes);
    await updates.stage('0.1.3', hash);
    await assert.rejects(updates.prepare('0.1.3', hash), /link escapes/);
    assert.deepEqual((await readdir(staging)).filter(name => name.startsWith('prepare-')), [prepared.directory.split(/[\\/]/).at(-1)]);
  }

  manifest = envelope('0.1.2');

  let startDownload!: () => void;
  const downloadStarted = new Promise<void>(resolve => { startDownload = resolve; });
  serve = response => { response.write(archive.subarray(0, 2)); startDownload(); return response; };
  const pending = assert.rejects(updates.stage('0.1.2', sha256));
  await downloadStarted;
  assert.throws(() => updates.stage('0.1.2', sha256), /already downloading/);
  await updates.close();
  await pending;
  assert.ok((await readdir(staging)).every(name => !name.endsWith('.part') && !name.endsWith('.part.manifest')));
  assert.deepEqual(await readFile(join(staging, `${sha256}.zip`)), archive);
  console.log('DESKTOP_UPDATE_STAGING_OK: signed download, verified offline package preparation, tamper and version rejection, failure retry and shutdown.');
} finally {
  await updates.close();
  server.closeAllConnections();
  await new Promise<void>(resolve => server.close(() => resolve()));
  await rm(root, { recursive: true, force: true });
}
