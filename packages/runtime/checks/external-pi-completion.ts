/** Deterministic real Pi session branches; no model or mock protocol. */
import assert from 'node:assert/strict';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createDesktopSessionManager } from '../src/agents/pi-session-manager.js';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';
import { SessionManager } from '@earendil-works/pi-coding-agent';
import { assertExternalPiCompletion } from '../src/agents/external-pi-completion.js';
const manager = SessionManager.inMemory();
const delivery = (id: string) => manager.appendCustomMessageEntry('hive-delivery', `raw ${id}`, false, { deliveryId: id });
const assistant = (stopReason: 'stop' | 'error' | 'aborted') => manager.appendMessage({ role: 'assistant', content: [{ type: 'text', text: stopReason }],
  api: 'openai-responses', provider: 'openai', model: 'gpt-6.1-terra', timestamp: Date.now(), stopReason,
  usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } });
for (const outcome of ['error', 'aborted'] as const) {
  const a = `A-${outcome}`;
  delivery(a);
  const failed = assistant(outcome);
  assert.throws(() => assertExternalPiCompletion(manager.getBranch(), a, failed), /matching successful/);
  const b = `B-${outcome}`;
  delivery(b);
  const success = assistant('stop');
  assert.throws(() => assertExternalPiCompletion(manager.getBranch(), a, success), /later input/);
  assertExternalPiCompletion(manager.getBranch(), b, success);
}
const branchPoint = delivery('branch');
const abandonedAssistant = assistant('stop');
assertExternalPiCompletion(manager.getBranch(), 'branch', abandonedAssistant);
manager.branch(branchPoint);
assert.throws(() => assertExternalPiCompletion(manager.getBranch(), 'branch', abandonedAssistant), /matching successful/);
const activeAssistant = assistant('stop');
assert.throws(() => assertExternalPiCompletion(manager.getBranch(), 'branch', abandonedAssistant), /matching successful/);
assertExternalPiCompletion(manager.getBranch(), 'branch', activeAssistant);
manager.appendMessage({ role: 'user', content: 'Different user input', timestamp: Date.now() });
const laterAssistant = assistant('stop');
assert.throws(() => assertExternalPiCompletion(manager.getBranch(), 'branch', laterAssistant), /later input/);
manager.branch(branchPoint);
delivery('sibling');
const siblingAssistant = assistant('stop');
manager.branch(branchPoint);
assert.throws(() => assertExternalPiCompletion(manager.getBranch(), 'sibling', siblingAssistant), /outside the active branch/);
// Load deterministic SDK-authored histories into the real external extension/RPC path.
const directory = await mkdtemp(join(tmpdir(), 'hive-external-completion-'));
for (const scenario of ['error', 'aborted', 'abandoned'] as const) {
  const fixture = createDesktopSessionManager(directory, directory);
  const a = fixture.appendCustomMessageEntry('hive-delivery', 'A', false, { deliveryId: 'A' });
  const message = (reason: 'error' | 'aborted' | 'stop') => fixture.appendMessage({ role: 'assistant', content: [{ type: 'text', text: reason }],
    api: 'openai-responses', provider: 'openai', model: 'gpt-6.1-terra', timestamp: Date.now(), stopReason: reason,
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } });
  const first = message(scenario === 'abandoned' ? 'stop' : scenario);
  let candidate = first;
  if (scenario === 'abandoned') {
    fixture.branch(a);
    // Persist the active leaf on the alternative branch without a successful assistant.
    fixture.appendCustomEntry('fixture-active-branch', {});
  } else {
    fixture.appendCustomMessageEntry('hive-delivery', 'B', false, { deliveryId: 'B' });
    candidate = message('stop');
  }
  const transport = await ExternalPiTransport.start({ cwd: directory, sessionDir: directory, sessionFile: fixture.getSessionFile()!,
    command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi', extensionPath: resolve('dist/agents/external-pi-extension.js') });
  try {
    await assert.rejects(transport.bridge({ action: 'complete', deliveryId: 'A', assistantEntryId: candidate }), scenario === 'abandoned' ? /matching successful/ : /later input/);
    const entries = await transport.request<{ entries: { customType?: string }[] }>('get_entries');
    assert.equal(entries.entries.some(entry => entry.customType === 'hive-delivery-completed'), false);
    if (scenario !== 'abandoned') {
      await transport.bridge({ action: 'complete', deliveryId: 'B', assistantEntryId: candidate });
      const accepted = await transport.request<{ entries: { customType?: string; data?: { id?: string } }[] }>('get_entries');
      assert.ok(accepted.entries.some(entry => entry.customType === 'hive-delivery-completed' && entry.data?.id === 'B'));
    }
  } finally { await transport.close(); }
}
console.log('External Pi completion identity and active branch checks passed (real RPC, no model requests)');
