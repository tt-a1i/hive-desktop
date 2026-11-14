/** Isolated real Hive + controlled Codex protocol peer for native UI interaction checks. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile, chmod } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const evidence = resolve('../../.artifacts/functional-parity/ui-qa');
const data = join(evidence, 'state');
const project = join(evidence, 'project');
await mkdir(join(project, 'src'), {recursive: true});
// This fixture lives under the repository's ignored .artifacts directory. Give
// the selected project its own Git root so parent ignore rules do not hide it.
execFileSync('git', ['init', '--quiet', project]);
await writeFile(join(project, 'src', 'reference notes.txt'), 'UI_REFERENCE_MARKER=verified-snapshot\n');
await writeFile(join(project, 'src', '中文说明.txt'), '项目引用与草稿保持测试。\n');
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const peer = resolve('checks/fixtures/codex-queue-peer.cjs');
const command = join(evidence, 'codex-ui-peer');
await writeFile(command, `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`, {mode: 0o700});
await chmod(command, 0o700);
await writeFile(join(project, 'queue-peer-control.json'), JSON.stringify({complete: true}));
const runtime = await createDesktopRuntime(data, () => {}, {executableEnvironment: {HIVE_CODEX_COMMAND: command}});
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: project, name: 'Functional UI QA', autostart_orchestrator: false,
  }) as {id: string};
  const record = await runtime.chats.createRecord(workspace.id, 'codex');
  runtime.chats.rename(record.id, 'Queue and history verification');
  await runtime.chats.prompt(record.id, 'Historical body phrase: cedar searchable evidence. This is controlled protocol UI data.');
  const deadline = Date.now() + 15_000;
  while ((await runtime.chats.open(record.id)).busy) {
    assert.ok(Date.now() < deadline, 'Controlled peer did not complete initial turn');
    await delay(20);
  }
  await writeFile(join(project, 'queue-peer-control.json'), JSON.stringify({complete: false}));
  await runtime.chats.prompt(record.id, 'Hold this controlled turn until the test pauses it.');
  await runtime.chats.prompt(record.id, 'Editable queued original wording');
  await runtime.chats.prompt(record.id, 'Cancel this queued message');
  await runtime.chats.abort(record.id);
  const snapshot = await runtime.chats.open(record.id);
  assert.equal(snapshot.pendingUserInputs.length, 2);
  assert.ok(snapshot.pendingUserInputs.every(item => item.editable));
  runtime.hive.saveNavigation(JSON.stringify({workspace: workspace.id, sessions: {[workspace.id]: record.id}}));
  await writeFile(join(project, 'queue-peer-control.json'), JSON.stringify({complete: true}));
  await writeFile(join(evidence, 'seed.json'), JSON.stringify({data, project, command, node: process.execPath,
    runtime: resolve('dist/main.js'), sessionId: record.id, workspaceId: workspace.id,
    controlledProtocol: true, actualInference: false, pending: snapshot.pendingUserInputs}, null, 2));
  console.log(`FUNCTIONAL_UI_SEEDED ${join(evidence, 'seed.json')}`);
} finally { await runtime.close(); }
