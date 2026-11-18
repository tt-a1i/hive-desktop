import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { constants } from 'node:fs';
import { copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { desktopPackagePaths, hashDesktopFile, installDesktopUpdate, prepareDesktopRecoveryEntry } from '../src/desktop-update-installer.js';

const fixture = await realpath(await mkdtemp(join(tmpdir(), 'hive-install-')));
const preload = join(fixture, 'startup.cjs');
await writeFile(preload, `
  const fs = require('node:fs');
  const path = require('node:path');
  const {version, healthy} = JSON.parse(fs.readFileSync(path.join(path.dirname(process.execPath), 'fixture.json')));
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

async function executable(directory: string, version: string, healthy: boolean) {
  const path = desktopPackagePaths(directory).executable;
  await mkdir(dirname(path), { recursive: true });
  await copyFile(binary, path, constants.COPYFILE_FICLONE);
  await writeFile(join(dirname(path), 'fixture.json'), JSON.stringify({ version, healthy }));
  return path;
}

async function waitForExit(directory: string, version: string) {
  for (let i = 0; i < 100; i++) {
    const output = await readFile(join(directory, 'exited'), 'utf8').catch(() => '');
    if (output.split('\n').includes(version)) return;
    await delay(30);
  }
  assert.fail(`Desktop fixture ${version} did not exit`);
}

async function checkTransaction(healthy: boolean) {
  const root = join(fixture, healthy ? 'healthy' : 'broken');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  const transaction = join(root, 'transaction');
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'User draft must survive replacement.');
  const current = await executable(installation, '0.1.0', true);
  await executable(join(transaction, 'next'), '0.1.1', healthy);
  await installDesktopUpdate({ installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum }, transaction);

  const expected = healthy ? '0.1.1' : '0.1.0';
  const receipt = JSON.parse(await readFile(join(dataDirectory, 'desktop-updates/install-result.json'), 'utf8'));
  assert.equal(receipt.state, healthy ? 'installed' : 'rolledBack', JSON.stringify(receipt));
  await waitForExit(dataDirectory, expected);
  assert.equal(receipt.version, expected);
  assert.equal(receipt.requestedVersion, '0.1.1');
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, expected);
  assert.equal(await readFile(join(dataDirectory, 'started'), 'utf8'), healthy ? '0.1.1\n' : '0.1.1\n0.1.0\n');
  assert.equal(await readFile(draft, 'utf8'), 'User draft must survive replacement.');
}

async function checkInterruptedReplacement() {
  const root = join(fixture, 'interrupted');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'Draft before interruption');
  const current = await executable(installation, '0.1.0', true);
  const transaction = await mkdtemp(join(root, '.hive-desktop-install-'));
  await executable(join(transaction, 'next'), '0.1.1', true);
  const program = join(transaction, 'installer.mjs');
  const node = join(transaction, process.platform === 'win32' ? 'node.exe' : 'node');
  const packagedRuntime = process.env.HIVE_DESKTOP_TEST_RUNTIME;
  const compiledInstaller = packagedRuntime
    ? join(dirname(packagedRuntime), 'desktop-update-installer.js')
    : new URL('../dist/desktop-update-installer.js', import.meta.url);
  await copyFile(compiledInstaller, program);
  await copyFile(binary, node, constants.COPYFILE_FICLONE);
  await writeFile(join(transaction, 'install-plan.json'), JSON.stringify({
    installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum,
  }));
  const entry = await prepareDesktopRecoveryEntry(transaction);
  const gate = join(transaction, 'after-first-rename.cjs');
  const marker = join(transaction, 'backup-moved');
  // The real first rename has happened when the process reaches this barrier.
  // Pause only this test child so termination lands before the second rename.
  await writeFile(gate, `
    const fs = require('node:fs/promises');
    const {syncBuiltinESMExports} = require('node:module');
    const rename = fs.rename;
    fs.rename = async (from, to) => {
      await rename(from, to);
      if (to === process.env.HIVE_TEST_PREVIOUS) {
        await fs.writeFile(process.env.HIVE_TEST_MARKER, 'moved');
        await new Promise(resolve => setTimeout(resolve, 30000));
      }
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
      HIVE_TEST_PREVIOUS: join(transaction, 'previous'), HIVE_TEST_MARKER: marker },
  });
  const exited = once(installer, 'exit');
  const recover = () => promisify(execFile)(node, [program, '--recover'], { env, timeout: 10000 });
  try {
    for (let i = 0; i < 200; i++) {
      assert.equal(installer.exitCode, null, 'installer exited before first rename');
      if (await readFile(marker, 'utf8').catch(() => '') === 'moved') break;
      await delay(25);
    }
    assert.equal(await readFile(marker, 'utf8'), 'moved');
    await assert.rejects(readFile(current), { code: 'ENOENT' });
    await assert.rejects(recover(), /Another desktop installer is running/);
  } finally {
    if (installer.exitCode === null && installer.signalCode === null) installer.kill('SIGKILL');
    await exited;
  }
  assert.match(await readFile(entry, 'utf8'), /installer\.mjs.*--recover/);
  const { stdout } = await recover();
  assert.match(stdout, /0\.1\.0 was recovered/);
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, '0.1.0');
  const receipt = JSON.parse(await readFile(join(dataDirectory, 'desktop-updates/install-result.json'), 'utf8'));
  assert.equal(receipt.state, 'rolledBack');
  assert.equal(await readFile(draft, 'utf8'), 'Draft before interruption');
  assert.match((await recover()).stdout, /already available/);
}

try {
  await checkTransaction(true);
  await checkTransaction(false);
  await checkInterruptedReplacement();
  console.log('DESKTOP_UPDATE_INSTALLER_OK: replacement, failed-start rollback, interrupted-installer recovery, and unchanged user data.');
} finally {
  if (previousOptions === undefined) delete process.env.NODE_OPTIONS;
  else process.env.NODE_OPTIONS = previousOptions;
  await rm(fixture, { recursive: true, force: true });
}
