import assert from 'node:assert/strict';
import {mkdtemp, mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
const root = await mkdtemp(join(tmpdir(), 'hive-admission-'));
const project = join(root, 'project'); await mkdir(project);
const runtime = await createDesktopRuntime(join(root, 'data'), () => {}, {executableEnvironment: {}});
try {
  const ws = await runtime.hive.request('POST', '/api/workspaces', {path: project, name: 'admission', autostart_orchestrator: false}) as {id: string};
  const before = runtime.hive.executionLedgerRevision();
  const release = runtime.executionMutations.acquire(ws.id);
  assert.throws(() => runtime.executionMutations.begin(ws.id));
  await assert.rejects(runtime.hive.request('POST', `/api/workspaces/${ws.id}/shell/start`));
  assert.equal(runtime.hive.executionLedgerRevision(), before);
  release();
  const shell = await runtime.terminal.start(ws.id);
  assert.equal(await runtime.terminal.hasAnyActiveShell(), true);
  await runtime.terminal.stop({workspaceId: ws.id, runId: shell.runId});
  console.log('EXECUTION_ADMISSION_OK: real Hive HTTP shell gate, ledger snapshot and active shell inventory');
} finally { await runtime.close(); }
