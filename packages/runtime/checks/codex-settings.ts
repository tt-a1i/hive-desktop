/** Production adapter + actual JSONL peer; native acknowledgement is deliberately before application. */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCodexSession } from '../src/agents/codex.js';
import { CodexRpcError } from '../src/agents/codex-transport.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-codex-settings-'));
const handle = await createCodexSession({ cwd: dir, sessionDir: join(dir, 'session'),
  command: fileURLToPath(new URL('./fixtures/codex-settings-peer.cjs', import.meta.url)), onEvent() {} });
const control = (value: unknown) => writeFile(join(dir, 'settings-control.json'), JSON.stringify(value));
try {
  await handle.setModel('openai', 'gpt-6.1-terra');
  assert.equal(handle.currentModel()?.id, 'gpt-6.1-terra');
  await handle.setModel('openai', 'gpt-6.1-terra');
  assert.equal((await handle.setReasoning!('high')).level, 'high', 'Readback must wait for native application');
  assert.equal((await handle.setReasoning!('high')).level, 'high', 'Identical effort emits no native event');
  await control({clamp:'medium'});
  assert.equal((await handle.setReasoning!('low')).level, 'medium', 'Return native applied effort rather than echoing requested effort');
  await control({reject:true});
  await assert.rejects(handle.setReasoning!('low'), error => error instanceof CodexRpcError && error.code === -32602);
  await control({exit:true});
  await assert.rejects(handle.setReasoning!('low'));
} finally {
  await handle.dispose();
  await rm(dir, {recursive:true,force:true});
}
console.log('CODEX_SETTINGS_OK: delayed native apply, same-value no-op, actual clamp, unrelated notification, RPC error and process exit. Synthetic peer; no inference.');
