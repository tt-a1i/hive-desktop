import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const flag = process.argv.indexOf('--bundle');
assert.ok(flag >= 0 && process.argv[flag + 1], 'Pass --bundle followed by the extracted package directory');
const bundle = resolve(process.argv[flag + 1]);
const mac = process.platform === 'darwin';
const contents = join(bundle, 'Hive Desktop.app', 'Contents');
const resources = mac ? join(contents, 'Resources') : join(bundle, 'resources');
const executable = mac ? join(contents, 'MacOS', 'hive-desktop') : join(bundle, 'hive-desktop.exe');
const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime/dist/desktop-runtime.js')).href);
const directory = await mkdtemp(join(tmpdir(), 'hive-native-restore-'));
const state = join(directory, 'state');
let child;
let exited;
let reader;
try {
  const runtime = await createDesktopRuntime(state, () => {});
  let workspaceId;
  let chatId;
  try {
    for (const name of ['Alpha', 'Beta']) {
      const path = join(directory, name);
      await mkdir(path);
      await runtime.hive.request('POST', '/api/workspaces', { path, name, autostart_orchestrator: false });
    }
    const projects = await runtime.hive.request('GET', '/api/workspaces');
    workspaceId = projects[1].id;
    chatId = (await runtime.chats.create(workspaceId)).id;
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspaceId, sessions: { [workspaceId]: 'removed-chat' } }));
  } finally {
    await runtime.close();
  }

  // The native window must select the second project, successfully open its chat
  // and replace the obsolete selection. Booting just the sidecar cannot do this.
  const env = { ...process.env, HIVE_DESKTOP_DATA_DIR: state, HIVE_DESKTOP_TRACE_RPC: '1' };
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  child = spawn(executable, [], { env, cwd: directory, stdio: 'inherit' });
  exited = once(child, 'exit');
  reader = new DatabaseSync(join(state, 'hive/runtime.sqlite'), { readOnly: true });
  const read = reader.prepare("SELECT value FROM app_state WHERE key = 'desktop_navigation'");
  const deadline = Date.now() + 60_000;
  let restored;
  while (Date.now() < deadline) {
    const selected = JSON.parse(read.get().value);
    if (selected.workspace === workspaceId && selected.sessions[workspaceId] === chatId) { restored = selected; break; }
    if (child.exitCode !== null || child.signalCode !== null) break;
    await delay(200);
  }
  assert.ok(restored, `Native window did not restore the chat (exit ${child.exitCode}, signal ${child.signalCode})`);
  console.log('Native desktop opened the saved project and recovered its missing chat selection from the extracted bundle.');
} finally {
  reader?.close();
  if (child && child.exitCode === null && child.signalCode === null) { child.kill(); await exited; }
  await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 250 });
}
