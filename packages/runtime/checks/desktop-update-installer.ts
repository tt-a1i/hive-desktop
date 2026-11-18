import assert from 'node:assert/strict';
import { constants } from 'node:fs';
import { copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { desktopPackagePaths, hashDesktopFile, installDesktopUpdate } from '../src/desktop-update-installer.js';

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

try {
  await checkTransaction(true);
  await checkTransaction(false);
  console.log('DESKTOP_UPDATE_INSTALLER_OK: real process readiness, installation replacement, failed-start rollback, and unchanged user data.');
} finally {
  if (previousOptions === undefined) delete process.env.NODE_OPTIONS;
  else process.env.NODE_OPTIONS = previousOptions;
  await rm(fixture, { recursive: true, force: true });
}
