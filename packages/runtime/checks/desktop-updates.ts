import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { once } from 'node:events';
import { constants } from 'node:fs';
import { cp, copyFile, mkdir, mkdtemp, readFile, readdir, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';
import { DesktopUpdates } from '../src/desktop-updates.js';
import { desktopPackagePaths, hashDesktopFile } from '../src/desktop-update-archive.js';
import { installDesktopUpdate } from '../src/desktop-update-installer.js';

if (!['darwin', 'win32'].includes(process.platform)) {
  console.log('Desktop staging requires a supported package target');
  process.exit(0);
}

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-update-channel-')));
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
    files: mac ? 5 : 4, installerProtocol: 2,
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

  // The running Node must belong to the installed bundle. The update is inert
  // in this test, but its verification, preparation and removal use real files.
  const parent = join(root, 'installed-packages');
  const installation = join(parent, mac ? 'Hive Desktop.app' : 'portable');
  const installed = desktopPackagePaths(installation);
  for (const path of [installed.executable, installed.node, installed.runtime]) {
    await mkdir(dirname(path), { recursive: true });
  }
  await copyFile(paths.executable, installed.executable);
  await copyFile(paths.runtime, installed.runtime);
  await copyFile(process.execPath, installed.node, constants.COPYFILE_FICLONE);
  const installedPolicy = join(installed.resources, 'desktop-update-channel.json');
  await copyFile(policyPath, installedPolicy);
  if (mac) {
    const plist = await readFile(join(packageRoot, 'Contents/Info.plist'), 'utf8');
    await writeFile(join(installation, 'Contents/Info.plist'), plist.replaceAll('0.1.3', '0.1.0'));
  }
  await writeFile(join(installed.resources, 'desktop-update-identity.json'), JSON.stringify({
    schemaVersion: 1, app: 'com.hive.desktop.dev', version: '0.1.0', platform: process.platform, arch: process.arch,
    executableSha256, files: mac ? 6 : 5, installerProtocol: 2,
  }));
  const compiledRuntime = process.env.HIVE_DESKTOP_TEST_RUNTIME;
  const updateModule = compiledRuntime
    ? pathToFileURL(join(dirname(compiledRuntime), 'desktop-updates.js'))
    : new URL('../dist/desktop-updates.js', import.meta.url);
  const runOwnershipCheck = join(root, 'installer-ownership.mjs');
  const obsolete = Buffer.from('obsolete, signed archive');
  const obsoleteSha = createHash('sha256').update(obsolete).digest('hex');
  const unsigned = Buffer.from('not a signed archive');
  const unsignedSha = createHash('sha256').update(unsigned).digest('hex');
  await writeFile(join(staging, obsoleteSha + '.zip'), obsolete);
  await writeFile(join(staging, obsoleteSha + '.manifest.json'),
    JSON.stringify(envelope('0.1.0', [{...asset, sha256:obsoleteSha,bytes:obsolete.length}])));
  await writeFile(join(staging, unsignedSha + '.zip'), unsigned);
  await writeFile(join(staging, unsignedSha + '.manifest.json'), '{}');
  await writeFile(runOwnershipCheck, `
    import assert from 'node:assert/strict';
    import {readFile,readdir} from 'node:fs/promises';
    import {dirname,join} from 'node:path';
    import {DesktopUpdates} from ${JSON.stringify(updateModule.href)};
    const directory = ${JSON.stringify(root)};
    const parent = ${JSON.stringify(parent)};
    const candidates = async () => (await readdir(parent)).filter(name =>
      name.startsWith('.hive-desktop-install-') || name.startsWith('Recover Hive Desktop '));
    const make = () => new DesktopUpdates(directory, new URL(${JSON.stringify(pathToFileURL(installedPolicy).href)}));
    const prepare = owner => owner.prepareInstaller('0.1.3', ${JSON.stringify(zippedHash)});

    const cache = join(directory, 'desktop-updates');
    const obsolete = join(cache, ${JSON.stringify(obsoleteSha + '.zip')});
    const unsigned = join(cache, ${JSON.stringify(unsignedSha + '.zip')});
    const updates = make();
    process.env.HIVE_DESKTOP_UPDATE_AUTHORIZE = '1';
    process.env.HIVE_DESKTOP_UPDATE_READY = join(parent, '.hive-desktop-install-Future', 'ready.json');
    await updates.check();
    assert.equal((await readFile(obsolete, 'utf8')), 'obsolete, signed archive');
    delete process.env.HIVE_DESKTOP_UPDATE_AUTHORIZE;
    delete process.env.HIVE_DESKTOP_UPDATE_READY;
    await updates.check();
    assert.equal((await readFile(unsigned, 'utf8')), 'not a signed archive');
    assert.equal((await readFile(join(cache, ${JSON.stringify(zippedHash + '.zip')}))).length, ${zippedBytes.length});
    await assert.rejects(readFile(obsolete), {code:'ENOENT'});
    await assert.rejects(readFile(obsolete.replace(/\\.zip$/, '.manifest.json')), {code:'ENOENT'});
    await updates.close();

    const ordinaryExit = make();
    await prepare(ordinaryExit);
    assert.equal((await candidates()).length, 2);
    await ordinaryExit.close();
    assert.deepEqual(await candidates(), [], 'ordinary exit must release the unused installer and recovery entry');

    const owner = make();
    const cancelled = await prepare(owner);
    await assert.rejects(owner.discardInstaller('an older installer id'), /installer changed/);
    assert.equal((await candidates()).length, 2);
    await owner.discardInstaller(cancelled.id);
    assert.deepEqual(await candidates(), []);
    const abandoned = await prepare(owner);
    const replacement = await prepare(owner);
    assert.notEqual(abandoned.id, replacement.id);
    assert.equal((await candidates()).length, 2);
    await assert.rejects(readFile(abandoned.program), {code:'ENOENT'});
    await owner.close();
    assert.deepEqual(await candidates(), []);

    const closing = make();
    const handedOff = await prepare(closing);
    await closing.close(handedOff.id);
    assert.equal((await candidates()).length, 2);
    assert.equal(JSON.parse(await readFile(join(dirname(handedOff.program), 'install-plan.json'), 'utf8')).toVersion, '0.1.3');
    const stranger = make();
    await assert.rejects(stranger.close(handedOff.id), /installer is no longer available/);
    assert.equal((await candidates()).length, 2);
    assert.equal((await readFile(join(directory, 'desktop-updates', ${JSON.stringify(zippedHash + '.zip')}))).length, ${zippedBytes.length});
  `);
  await promisify(execFile)(installed.node, [runOwnershipCheck], { timeout: 25000, windowsHide: true });

  if (mac) {
    const sealed = join(root, 'signed-fixture', 'Hive Desktop.app');
    await mkdir(dirname(sealed));
    await cp(packageRoot, sealed, { recursive: true });
    const seal = async (bundle: string, version: string, files: number) => {
      const paths = desktopPackagePaths(bundle);
      await copyFile('/usr/bin/true', paths.executable);
      await writeFile(join(paths.resources, 'desktop-update-identity.json'), JSON.stringify({
        schemaVersion: 2, app: 'com.hive.desktop.dev', version, platform: process.platform,
        arch: process.arch, files, installerProtocol: 2,
      }));
      execFileSync('/usr/bin/codesign', ['--force', '--sign', '-', bundle]);
    };
    const stageSealed = async (name: string) => {
      const archive = join(root, name);
      execFileSync('/usr/bin/zip', ['-q', '-y', '-r', archive, 'Hive Desktop.app'], { cwd: dirname(sealed) });
      const bytes = await readFile(archive);
      const hash = createHash('sha256').update(bytes).digest('hex');
      manifest = envelope('0.1.3', [{ ...signedAsset, bytes: bytes.length, sha256: hash }]);
      serve = response => response.end(bytes);
      await updates.stage('0.1.3', hash);
      return hash;
    };
    await seal(sealed, '0.1.3', 5);
    const sealedHash = await stageSealed('signed.zip');
    const sealedPackage = await updates.prepare('0.1.3', sealedHash);
    const verifiedExecutable = await hashDesktopFile(desktopPackagePaths(sealed).executable);
    assert.equal(await hashDesktopFile(desktopPackagePaths(sealedPackage.packageRoot).executable), verifiedExecutable);
    await rm(sealedPackage.directory, { recursive: true, force: true });

    // Even a valid channel attestation cannot substitute for a valid macOS
    // seal when the package uses the signed identity format.
    const sealedRuntime = desktopPackagePaths(sealed).runtime;
    const changed = await readFile(sealedRuntime);
    changed[0] = changed[0]! ^ 1;
    await writeFile(sealedRuntime, changed);
    const alteredHash = await stageSealed('signed-altered.zip');
    await assert.rejects(updates.prepare('0.1.3', alteredHash), /code signature verification failed/);

    const preflight = join(root, 'signed-installer-preflight.mjs');
    await writeFile(preflight, `
      import assert from 'node:assert/strict';
      import {readFile} from 'node:fs/promises';
      import {dirname,join} from 'node:path';
      import {DesktopUpdates} from ${JSON.stringify(updateModule.href)};
      const updates = new DesktopUpdates(${JSON.stringify(root)}, new URL(${JSON.stringify(pathToFileURL(installedPolicy).href)}));
      const signedSource = process.argv.includes('--signed-source');
      if (signedSource) {
        await assert.rejects(updates.prepareInstaller('0.1.3', ${JSON.stringify(zippedHash)}), /requires a code-signed update/);
      }
      const installer = await updates.prepareInstaller('0.1.3', ${JSON.stringify(sealedHash)});
      const directory = dirname(installer.program);
      const plan = JSON.parse(await readFile(join(directory, 'install-plan.json'), 'utf8'));
      assert.equal(plan.oldMacSignature, signedSource ? true : undefined);
      assert.equal(plan.newMacSignature, true);
      if (signedSource) {
        await updates.close(installer.id);
        console.log(directory);
      } else {
        await updates.close();
      }
    `);
    await promisify(execFile)(installed.node, [preflight], { timeout: 25000 });
    await seal(installation, '0.1.0', 6);
    const { stdout } = await promisify(execFile)(installed.node, [preflight, '--signed-source'], { timeout: 25000 });
    const transaction = stdout.trim();
    const plan = JSON.parse(await readFile(join(transaction, 'install-plan.json'), 'utf8'));
    assert.equal(plan.oldMacSignature, true);
    assert.equal(plan.newMacSignature, true);
    assert.equal(plan.newExecutableSha256, verifiedExecutable);
    const postPreparation = desktopPackagePaths(join(transaction, 'next'));
    await writeFile(postPreparation.runtime, changed);
    assert.equal(await hashDesktopFile(postPreparation.executable), verifiedExecutable);
    await installDesktopUpdate(plan, transaction);
    const result = JSON.parse(await readFile(join(staging, 'install-result.json'), 'utf8'));
    assert.equal(result.state, 'failed');
    assert.match(result.error, /code signature verification failed/);
    assert.equal(await hashDesktopFile(installed.executable), plan.oldExecutableSha256);
    assert.deepEqual(await readFile(installed.runtime), await readFile(paths.runtime));
    console.log('MAC_SIGNED_UPDATE_OK: signed archive, sealed resources, legacy transition, signed-only continuation, installer recheck.');
  }

  serve = response => response.end(zippedBytes);
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
  console.log('DESKTOP_UPDATE_STAGING_OK: signed download, offline preparation, installer ownership and cleanup, tamper and version rejection, failure retry and shutdown.');
} finally {
  await updates.close();
  server.closeAllConnections();
  await new Promise<void>(resolve => server.close(() => resolve()));
  await rm(root, { recursive: true, force: true });
}
