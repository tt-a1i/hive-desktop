/** Real Pi SDK transcript/context round trip; never triggers a model turn. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { convertToLlm, createAgentSession, SessionManager } from '@earendil-works/pi-coding-agent';
import { createPiSession } from '../src/agents/pi.js';
import { createDesktopSessionManager } from '../src/agents/pi-session-manager.js';
import type { PiEvent, PiSessionHandle } from '../src/agents/types.js';
import { isDeliveryPresentation, type DeliveryPresentation } from '../src/delivery-presentation.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-pi-delivery-'));
let adapter: PiSessionHandle | undefined;
try {
  // Fresh sessions must persist custom-only history before any model reply.
  const freshManager = createDesktopSessionManager(directory, directory);
  const freshId = freshManager.getSessionId();
  const freshFile = freshManager.getSessionFile()!;
  const freshDetails = { version: 1, origin: 'user', displayText: 'fresh input', deliveryId: 'fresh-delivery' };
  const { session: freshSession } = await createAgentSession({ cwd: directory, agentDir: join(directory, 'agent'), sessionManager: freshManager });
  try {
    await freshSession.sendCustomMessage({ customType: 'hive-delivery', content: 'fresh raw input', display: false,
      details: freshDetails }, { triggerTurn: false });
    assert.equal(existsSync(freshFile), true);
    const diskDelivery = (await readFile(freshFile, 'utf8')).split('\n').filter(Boolean).map(line => JSON.parse(line))
      .find(entry => entry.type === 'custom_message' && entry.customType === 'hive-delivery');
    assert.equal(diskDelivery.content, 'fresh raw input');
    assert.deepEqual(diskDelivery.details, freshDetails);
  } finally { freshSession.dispose(); }
  const reopened = SessionManager.open(freshFile, directory);
  assert.equal(reopened.getSessionId(), freshId);
  assert.deepEqual(reopened.buildSessionContext().messages.map(message => message.role), ['custom']);
  const freshEntry = reopened.buildSessionContext().messages[0] as { content: string; details: unknown };
  assert.equal(freshEntry.content, 'fresh raw input');
  assert.deepEqual(freshEntry.details, freshDetails);
  assert.deepEqual(reopened.getEntries().filter(entry => entry.type === 'custom' && entry.customType === 'hive-delivery-completed'), []);
  const manager = SessionManager.create(directory, directory);
  // A real legacy user entry also makes Pi flush custom entries to disk.
  const oldText = '<hive-message kind="dispatch">old user text is still user</hive-message>';
  manager.appendMessage({ role: 'user', content: oldText, timestamp: Date.now() });
  const { session } = await createAgentSession({ cwd: directory, agentDir: join(directory, 'agent'), sessionManager: manager });
  const raw = '<hive-message kind="dispatch">full raw envelope\nsecret routing instructions</hive-message>';
  const presentation: DeliveryPresentation = { version: 1, origin: 'dispatch', displayText: 'Visible task', dispatchId: 'dispatch-one', workerName: 'Worker' };
  const details = { ...presentation, deliveryId: 'delivery-one' };
  try {
    await session.sendCustomMessage({ customType: 'hive-delivery', content: raw, display: false, details }, { triggerTurn: false });
    await session.sendCustomMessage({ customType: 'hive-delivery', content: 'invalid metadata', display: false,
      details: { ...details, version: 2, deliveryId: 'invalid' } }, { triggerTurn: false });
    const context = convertToLlm(session.messages);
    assert.deepEqual(context[1], { role: 'user', content: [{ type: 'text', text: raw }], timestamp: session.messages[1]!.timestamp });
    assert.deepEqual(manager.buildSessionContext().messages[1], session.messages[1]);
  } finally { session.dispose(); }
  const file = manager.getSessionFile()!;
  const entry = (await readFile(file, 'utf8')).split('\n').filter(Boolean).map(line => JSON.parse(line))
    .find(value => value.type === 'custom_message' && value.customType === 'hive-delivery');
  assert.equal(entry.content, raw);
  assert.deepEqual(entry.details, details);
  const events: PiEvent[] = [];
  adapter = await createPiSession({ cwd: directory, sessionDir: directory, agentDir: join(directory, 'agent'), sessionFile: file, onEvent: event => events.push(event) });
  const snapshot = adapter.snapshot();
  const rendered = snapshot.history[1] as Record<string, unknown>;
  assert.equal(rendered.role, 'custom');
  assert.equal(rendered.content, raw);
  assert.deepEqual(rendered.presentation, details);
  assert.equal(snapshot.historyMessageIds[1], details.deliveryId);
  assert.equal('presentation' in (adapter.history()[1] as object), false);
  assert.notEqual(rendered, adapter.history()[1]);
  assert.equal((snapshot.history[0] as { content: string }).content, oldText);
  assert.equal('presentation' in (snapshot.history[0] as object), false);
  assert.equal('presentation' in (snapshot.history[2] as object), false);
  assert.notEqual(snapshot.historyMessageIds[2], 'invalid');
  await adapter.addContext('startup context');
  assert.equal(events.filter(event => event.type === 'message_end').length, 0);
  assert.deepEqual(adapter.completedDeliveryIds(), []);
  for (const invalid of [null, [], {}, { ...presentation, version: 2 }, { ...presentation, origin: 'forged' },
    { ...presentation, displayText: null }, { ...presentation, dispatchId: 123 }, { ...presentation, workerName: false }]) {
    assert.equal(isDeliveryPresentation(invalid), false);
  }
  assert.equal(isDeliveryPresentation(presentation), true);
  console.log('Pi delivery transcript/context, stable ID, snapshot copy, legacy and invalid metadata checks passed. No model request sent.');
} finally {
  adapter?.dispose();
  await rm(directory, { recursive: true, force: true });
}
