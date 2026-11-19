import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { constants } from 'node:fs';
import { copyFile, lstat, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { desktopPackagePaths, hashDesktopFile, prepareDesktopRecoveryEntry, reclaimDesktopUpdateTransactions, type DesktopInstallPlan } from '../src/desktop-update-installer.js';

const fixture = await realpath(await mkdtemp(join(tmpdir(), 'hive-install-')));
const preload = join(fixture, 'startup.cjs');
await writeFile(preload, `
  const fs = require('node:fs');
  const path = require('node:path');
  if (process.argv[2] === '--cleanup' && process.env.HIVE_TEST_KEEP_ROLLBACK_TRANSACTION) process.exit(0);
  if (process.env.HIVE_TEST_INSTALL_RECEIPT) {
    const io = fs.promises;
    const rename = io.rename;
    io.rename = async (from, to) => {
      if (to === process.env.HIVE_TEST_INSTALL_RECEIPT) throw new Error('Injected terminal receipt failure');
      return rename(from, to);
    };
    require('node:module').syncBuiltinESMExports();
  }
  if (process.argv[2] === '--cleanup' && process.env.HIVE_TEST_LOCKED_UPDATE_ZIP) {
    const io = fs.promises;
    const rm = io.rm;
    io.rm = async (path, options) => {
      if (path === process.env.HIVE_TEST_LOCKED_UPDATE_ZIP) {
        fs.writeFileSync(path + '.locked-fixture', 'cleanup attempted');
        throw Object.assign(new Error('Injected locked update ZIP'), {code:'EBUSY'});
      }
      return rm(path, options);
    };
    require('node:module').syncBuiltinESMExports();
  }
  const fixture = path.join(path.dirname(process.execPath), 'fixture.json');
  if (!fs.existsSync(fixture)) return;
  const {version, healthy} = JSON.parse(fs.readFileSync(fixture));
  const directory = process.env.HIVE_DESKTOP_DATA_DIR;
  fs.appendFileSync(path.join(directory, 'started'), version + '\\n');
  if (!healthy) process.exit(37);
  if (process.env.HIVE_DESKTOP_UPDATE_READY) {
    const ready = process.env.HIVE_DESKTOP_UPDATE_READY;
    fs.writeFileSync(ready + '.tmp', JSON.stringify({version,pid:process.pid}));
    fs.renameSync(ready + '.tmp', ready);
  }
  setTimeout(() => fs.appendFileSync(path.join(directory, 'exited'), version + '\\n'), 750);
`);

const binary = await realpath(process.execPath);
const checksum = await hashDesktopFile(binary);
const previousOptions = process.env.NODE_OPTIONS;
process.env.NODE_OPTIONS = `--require "${preload.replaceAll('\\', '/')}"`;

async function copyNodeFixture(path: string) {
  if (process.platform === 'darwin') {
    // The packaged Node does not clone files with COPYFILE_FICLONE on macOS.
    await promisify(execFile)('/bin/cp', ['-c', binary, path]);
  } else {
    await copyFile(binary, path, constants.COPYFILE_FICLONE);
  }
}

async function executable(directory: string, version: string, healthy: boolean) {
  const path = desktopPackagePaths(directory).executable;
  await mkdir(dirname(path), { recursive: true });
  await copyNodeFixture(path);
  await writeFile(join(dirname(path), 'fixture.json'), JSON.stringify({ version, healthy }));
  const node = desktopPackagePaths(directory).node;
  await mkdir(dirname(node), { recursive: true });
  await copyNodeFixture(node);
  return path;
}

async function prepareTransaction(root: string, plan: DesktopInstallPlan) {
  const transaction = await mkdtemp(join(root, '.hive-desktop-install-'));
  const program = join(transaction, 'installer.mjs');
  const node = join(transaction, process.platform === 'win32' ? 'node.exe' : 'node');
  const packagedRuntime = process.env.HIVE_DESKTOP_TEST_RUNTIME;
  const compiled = packagedRuntime ? join(dirname(packagedRuntime), 'desktop-update-installer.js')
    : new URL('../dist/desktop-update-installer.js', import.meta.url);
  await copyFile(compiled, program);
  await copyNodeFixture(node);
  await writeFile(join(transaction, 'install-plan.json'), JSON.stringify(plan));
  return { transaction, program, node, entry: await prepareDesktopRecoveryEntry(transaction) };
}

async function waitForRemoval(path: string) {
  const deadline = Date.now() + 7000;
  for (;;) {
    if (!await lstat(path).then(() => true, error => { if (error.code === 'ENOENT') return false; throw error; })) return;
    assert.ok(Date.now() < deadline, `Settled transaction was not removed: ${path}`);
    await delay(50);
  }
}

async function waitForExit(directory: string, version: string) {
  for (let i = 0; i < 100; i++) {
    const output = await readFile(join(directory, 'exited'), 'utf8').catch(() => '');
    if (output.split('\n').includes(version)) return;
    await delay(30);
  }
  assert.fail(`Desktop fixture ${version} did not exit`);
}

async function checkTransaction(healthy: boolean, receiptFault = false, lockedCache = false, supersededReceipt = false) {
  const root = join(fixture, receiptFault ? 'receipt-failure' : healthy ? 'healthy' : 'broken');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'User draft must survive replacement.');
  const current = await executable(installation, '0.1.0', true);
  const archiveSha256 = 'f'.repeat(64);
  const {transaction, program, node, entry} = await prepareTransaction(root, {
    installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum, archiveSha256,
  });
  await executable(join(transaction, 'next'), '0.1.1', healthy);
  const releaseCache = join(dataDirectory, 'desktop-updates', `${archiveSha256}.zip`);
  const attestation = releaseCache.replace(/\.zip$/, '.manifest.json');
  await writeFile(releaseCache, 'Verified fixture ZIP');
  await writeFile(attestation, '{}');
  const receiptPath = join(dataDirectory, 'desktop-updates/install-result.json');
  const original = spawn(binary, ['-e', ''], { env: process.env, stdio: 'ignore' });
  await once(original, 'exit');
  const run = (mode: string, fault?: string) => promisify(execFile)(node, [program, mode], {
    windowsHide: true, timeout: 10000, env:{ ...process.env, HIVE_TEST_INSTALL_RECEIPT: fault ?? '',
      HIVE_TEST_LOCKED_UPDATE_ZIP: lockedCache ? releaseCache : '',
      HIVE_TEST_KEEP_ROLLBACK_TRANSACTION: !healthy && receiptFault ? '1' : '' },
  });
  const expected = healthy ? '0.1.1' : '0.1.0';
  const state = healthy ? 'installed' : 'rolledBack';
  const laterReceipt = { state: 'rolledBack', version: expected, requestedVersion: '0.1.2', transactionId: '.hive-desktop-install-later' };
  const startupFailure = 'Updated desktop exited before startup completed (37)';
  if (receiptFault) {
    await assert.rejects(run(String(original.pid), receiptPath), /Injected terminal receipt failure/);
    await assert.rejects(readFile(receiptPath), {code:'ENOENT'});
    assert.match(await readFile(entry, 'utf8'), /--recover/);
    const journal = JSON.parse(await readFile(join(transaction, 'install-status.json'), 'utf8'));
    assert.equal(journal.settledBy, undefined);
    assert.equal(journal.state, state);
    if (healthy) await lstat(desktopPackagePaths(join(transaction, 'previous')).executable);
    else {
      assert.equal(journal.error, startupFailure);
      await assert.rejects(run('--recover', receiptPath), /Injected terminal receipt failure/);
    }
    await waitForExit(dataDirectory, expected);
    if (supersededReceipt) await writeFile(receiptPath, JSON.stringify(laterReceipt));
    const recoveryMessage = healthy ? /0\.1\.1 is installed/ : /The original installation is already available/;
    assert.match((await run('--recover')).stdout, recoveryMessage);
    if (!healthy) {
      // Keep the real transaction around to exercise the settled recovery path too.
      assert.match((await run('--recover')).stdout, recoveryMessage);
      await reclaimDesktopUpdateTransactions(installation, dataDirectory);
    }
  } else await run(String(original.pid));

  const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
  assert.equal(receipt.state, state, JSON.stringify(receipt));
  await waitForExit(dataDirectory, expected);
  assert.equal(receipt.version, expected);
  if (supersededReceipt) assert.deepEqual(receipt, laterReceipt);
  else {
    assert.equal(receipt.requestedVersion, '0.1.1');
    if (receiptFault && !healthy) assert.equal(receipt.error, startupFailure);
  }
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, expected);
  assert.equal(await readFile(join(dataDirectory, 'started'), 'utf8'), healthy ? '0.1.1\n' : '0.1.1\n0.1.0\n');
  assert.equal(await readFile(draft, 'utf8'), 'User draft must survive replacement.');
  if (lockedCache) {
    const lockEvent = releaseCache + '.locked-fixture';
    const deadline = Date.now() + 7000;
    while (await readFile(lockEvent, 'utf8').catch(() => '') !== 'cleanup attempted') {
      assert.ok(Date.now() < deadline, 'The detached cleaner never attempted to release the locked archive');
      await delay(50);
    }
    assert.equal(await readFile(releaseCache, 'utf8'), 'Verified fixture ZIP');
    assert.equal(await readFile(attestation, 'utf8'), '{}');
    assert.ok(JSON.parse(await readFile(join(transaction, 'install-status.json'), 'utf8')).settledBy);
    while (await lstat(transaction).then(() => true, error => { if (error.code === 'ENOENT') return false; throw error; })) {
      assert.ok(Date.now() < deadline, 'The next check did not reclaim the transaction');
      await reclaimDesktopUpdateTransactions(installation, dataDirectory);
      await delay(50);
    }
  }
  await waitForRemoval(transaction);
  await assert.rejects(lstat(entry), {code:'ENOENT'});
  if (healthy) {
    await assert.rejects(readFile(releaseCache), {code:'ENOENT'});
    await assert.rejects(lstat(attestation), {code:'ENOENT'});
  } else assert.equal(await readFile(releaseCache, 'utf8'), 'Verified fixture ZIP');
  await rm(root, { recursive: true, maxRetries: 10, retryDelay: 100 });
}

async function checkInterruptedReplacement(beforeJournal = false) {
  const root = join(fixture, beforeJournal ? 'interrupted-before-journal' : 'interrupted-after-backup');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'Draft before interruption');
  const current = await executable(installation, '0.1.0', true);
  const digest = 'e'.repeat(64);
  const cache = join(dataDirectory, 'desktop-updates', `${digest}.zip`);
  const attestation = cache.replace(/\.zip$/, '.manifest.json');
  const receiptPath = join(dataDirectory, 'desktop-updates/install-result.json');
  const earlierReceipt = '{"transactionId":"earlier-transaction"}';
  await writeFile(cache, 'Verified fixture ZIP');
  await writeFile(attestation, '{}');
  await writeFile(receiptPath, earlierReceipt);
  const {transaction, program, node, entry} = await prepareTransaction(root, {
    installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum, archiveSha256: digest,
  });
  const next = await executable(join(transaction, 'next'), '0.1.1', true);
  const journal = join(transaction, 'install-status.json');
  const previous = join(transaction, 'previous');
  const gate = join(transaction, 'rename-gate.cjs');
  const marker = join(transaction, 'at-rename');
  // Pause only the test installer with the lock held, either before the journal
  // is committed or once the original has actually moved to the backup path.
  await writeFile(gate, `
    const fs = require('node:fs/promises');
    const {syncBuiltinESMExports} = require('node:module');
    const rename = fs.rename;
    fs.rename = async (from, to) => {
      if (to !== process.env.HIVE_TEST_RENAME_TARGET) return rename(from, to);
      if (!process.env.HIVE_TEST_BEFORE_RENAME) await rename(from, to);
      await fs.writeFile(process.env.HIVE_TEST_MARKER, 'paused');
      await new Promise(resolve => setTimeout(resolve, 30000));
    };
    syncBuiltinESMExports();
  `);
  const env: NodeJS.ProcessEnv = { ...process.env };
  delete env.NODE_OPTIONS;
  const original = spawn(binary, ['-e', ''], { env, stdio: 'ignore' });
  await once(original, 'exit');
  const installer = spawn(node, [program, String(original.pid)], {
    stdio: 'ignore',
    env: { ...env, NODE_OPTIONS: `--require "${gate.replaceAll('\\', '/')}"`,
      HIVE_TEST_RENAME_TARGET: beforeJournal ? journal : previous,
      HIVE_TEST_BEFORE_RENAME: beforeJournal ? '1' : '', HIVE_TEST_MARKER: marker },
  });
  const exited = once(installer, 'exit');
  const recover = () => promisify(execFile)(node, [program, '--recover'], { env, timeout: 10000 });
  try {
    for (let i = 0; i < 200; i++) {
      assert.equal(installer.exitCode, null, 'installer exited before reaching the rename gate');
      if (await readFile(marker, 'utf8').catch(() => '') === 'paused') break;
      await delay(25);
    }
    assert.equal(await readFile(marker, 'utf8'), 'paused');
    if (beforeJournal) {
      await assert.rejects(lstat(journal), { code: 'ENOENT' });
      assert.equal(await hashDesktopFile(current), checksum);
    } else await assert.rejects(lstat(current), { code: 'ENOENT' });
    await assert.rejects(recover(), /Another desktop installer is running/);
  } finally {
    if (installer.exitCode === null && installer.signalCode === null) installer.kill('SIGKILL');
    await exited;
  }
  assert.match(await readFile(entry, 'utf8'), /installer\.mjs.*--recover/);
  if (beforeJournal) {
    await mkdir(previous);
    await assert.rejects(recover(), /Cannot verify an unstarted desktop update/);
    await assert.rejects(lstat(journal), { code: 'ENOENT' });
    assert.equal(await hashDesktopFile(next), checksum);
    await rm(previous, { recursive: true });
    env.NODE_OPTIONS = process.env.NODE_OPTIONS;
    env.HIVE_TEST_KEEP_ROLLBACK_TRANSACTION = '1';
    assert.match((await recover()).stdout, /The pending update was cancelled/);
    assert.match((await recover()).stdout, /The pending update was cancelled/);
    await assert.rejects(promisify(execFile)(node, [program, String(original.pid)], { env, timeout: 10000 }), /This desktop update has already started/);
    assert.equal(await readFile(receiptPath, 'utf8'), earlierReceipt);
    await reclaimDesktopUpdateTransactions(installation, dataDirectory);
  } else {
    assert.match((await recover()).stdout, /0\.1\.0 was recovered/);
    const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
    assert.equal(receipt.state, 'rolledBack');
  }
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, '0.1.0');
  assert.equal(await hashDesktopFile(current), checksum);
  assert.equal(await readFile(cache, 'utf8'), 'Verified fixture ZIP');
  assert.equal(await readFile(attestation, 'utf8'), '{}');
  assert.equal(await readFile(draft, 'utf8'), 'Draft before interruption');
  await waitForRemoval(transaction);
  await assert.rejects(lstat(entry), {code:'ENOENT'});
}

try {
  await checkTransaction(true, false, true);
  await checkTransaction(false);
  await checkTransaction(true, true);
  await checkTransaction(false, true);
  await checkTransaction(false, true, false, true);
  await checkInterruptedReplacement();
  await checkInterruptedReplacement(true);
  console.log('DESKTOP_UPDATE_INSTALLER_OK: replacement, failed-start rollback, interrupted-installer recovery, and unchanged user data.');
} finally {
  if (previousOptions === undefined) delete process.env.NODE_OPTIONS;
  else process.env.NODE_OPTIONS = previousOptions;
  await rm(fixture, { recursive: true, force: true });
}
