import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DesktopUpdates } from '../src/desktop-updates.js';

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
let serve = (response: import('node:http').ServerResponse) => response.end(archive);
const server = createServer((request, response) => {
  if (request.url === '/feed/manifest.json') response.end(JSON.stringify(manifest));
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

  let startDownload!: () => void;
  const downloadStarted = new Promise<void>(resolve => { startDownload = resolve; });
  serve = response => { response.write(archive.subarray(0, 2)); startDownload(); return response; };
  const pending = assert.rejects(updates.stage('0.1.2', sha256));
  await downloadStarted;
  assert.throws(() => updates.stage('0.1.2', sha256), /already downloading/);
  await updates.close();
  await pending;
  assert.deepEqual(await readdir(staging), [`${sha256}.zip`]);
  assert.deepEqual(await readFile(join(staging, `${sha256}.zip`)), archive);
  console.log('DESKTOP_UPDATE_STAGING_OK: local HTTP, signed release, platform and version selection, bounded verified staging, failure retry and shutdown.');
} finally {
  await updates.close();
  server.closeAllConnections();
  await new Promise<void>(resolve => server.close(() => resolve()));
  await rm(root, { recursive: true, force: true });
}
