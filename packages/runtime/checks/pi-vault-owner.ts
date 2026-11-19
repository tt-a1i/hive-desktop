import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { PiVault } from '../src/pi-vault.js';

assert.equal(process.platform, 'darwin');
assert(process.env.HIVE_DESKTOP_SHELL_OWNER, 'Pass the native owner executable');
if (process.argv[2] === '--read') {
  const vault = new PiVault(process.argv[3]!);
  const [binding] = await vault.bindings(vault.agentDirectory);
  const value = await vault.resolve(binding!);
  assert.equal(value, 'fixture-value');
  await vault.close();
  process.exit(0);
}
const dir = await mkdtemp(join(tmpdir(), 'hive-vault-owner-'));
const pids = new Set<number>();
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
async function until(test: () => Promise<boolean> | boolean, label: string, ms = 5000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (await test()) return; await delay(20); }
  throw new Error(`Timed out: ${label}`);
}
const unrelated = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], {detached: true, stdio: 'ignore'});
try {
  const agent = join(dir, 'agent'), data = join(dir, 'data'), bin = join(dir, 'bin'), record = join(dir, 'pids');
  await Promise.all([agent, data, bin].map(path => mkdir(path)));
  await writeFile(join(agent, 'models.json'), JSON.stringify({providers: {fixture: {api: 'openai-responses', baseUrl: 'https://example.test', models: [{id: 'fixture', name: 'Fixture', contextWindow: 1000, maxTokens: 100}]}}}));
  process.env.PI_CODING_AGENT_DIR = agent;
  process.env.PI_OFFLINE = '1';
  process.env.PATH = `${bin}${delimiter}${process.env.PATH ?? ''}`;
  const vault = new PiVault(data);
  const view = await vault.view();
  await vault.change(view.revision, view.configRevision, 'fixture', 'op://fixture/item/field');
  await vault.close();
  const opScript = join(dir, 'fake-op.cjs');
  await writeFile(opScript, `
const fs = require('node:fs');
const {spawn} = require('node:child_process');
const helper = spawn(process.execPath, ['-e', 'process.on("SIGTERM", () => {}); setInterval(() => {}, 1000)'], {
  stdio: ['ignore', 'inherit', 'ignore']
});
helper.unref();
fs.writeFileSync(process.env.FIXTURE_RECORD, JSON.stringify({
  root: process.pid, helper: helper.pid, parent: process.ppid
}));
if (process.env.FIXTURE_MODE === 'exit') process.stdout.write('fixture-value');
else setInterval(() => {}, 1000);
`);
  // A packaged executable lives below "Hive Desktop.app". A shebang cannot
  // represent that interpreter path; the shell forwards each argument intact.
  await writeFile(join(bin, 'op'), '#!/bin/sh\nexec "$HIVE_TEST_NODE" "$HIVE_TEST_OP_SCRIPT" "$@"\n', {mode: 0o700});
  process.env.HIVE_TEST_NODE = process.execPath;
  process.env.HIVE_TEST_OP_SCRIPT = opScript;
  for (const mode of ['exit', 'kill']) {
    await rm(record, {force: true});
    const node = spawn(process.execPath, [...process.execArgv, fileURLToPath(import.meta.url), '--read', data], {detached: true, stdio: 'ignore', env: {...process.env, FIXTURE_RECORD: record, FIXTURE_MODE: mode}});
    pids.add(node.pid!);
    const closed = new Promise<number | null>(resolve => node.once('close', resolve));
    let scope: {root:number;helper:number;parent:number} | undefined;
    await until(async () => {
      try { scope = JSON.parse(await readFile(record, 'utf8')); return true; }
      catch (error) {
        if (node.exitCode !== null || node.signalCode) throw new Error('Fixture reader exited before publishing process IDs', {cause: error});
        return false;
      }
    }, 'fixture started');
    const owned = scope!; pids.add(owned.root); pids.add(owned.helper);
    if (owned.parent !== node.pid) pids.add(owned.parent);
    if (mode === 'kill') { process.kill(node.pid!, 'SIGKILL'); await closed; }
    else assert.equal(await Promise.race([closed, delay(3000, 'blocked')]), 0, 'read should complete after root exits');
    pids.delete(node.pid!);
    await until(() => !alive(owned.root) && !alive(owned.helper) && (owned.parent === node.pid || !alive(owned.parent)), 'owned processes gone');
    assert(alive(unrelated.pid!), 'unrelated process survives');
    for (const pid of [owned.root, owned.helper, owned.parent]) pids.delete(pid);
    console.log(`${mode}: owned processes gone, unrelated process alive`);
  }
} finally {
  for (const pid of [...pids, unrelated.pid!]) { try { process.kill(pid, 'SIGKILL'); } catch {} }
  await rm(dir, {recursive: true, force: true});
}
