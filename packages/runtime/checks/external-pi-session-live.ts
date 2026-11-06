/** One external Pi Terra session: streamed tool execution, exact delivery completion, reload. */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createExternalPiSession, type ExternalPiTeamBinding } from '../src/agents/external-pi.js';
import type { PiEvent } from '../src/agents/types.js';
if (!process.argv.includes('--live')) throw new Error('Pass --live for native openai/gpt-6.1-terra');
const directory = await mkdtemp(join(tmpdir(), 'hive-external-session-live-'));
const events: PiEvent[] = [];
let binding: ExternalPiTeamBinding | undefined;
let calls = 0;
let onToolStart: (() => void) | undefined;
const token = randomUUID();
const options = { cwd: directory, sessionDir: directory, command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi',
  model: { provider: 'openai', id: 'gpt-6.1-terra' }, onEvent: (event: PiEvent) => { events.push(event); if (event.type === 'tool_start' && event.toolName === 'bash') onToolStart?.(); },
  teamBinding: () => { calls++; return binding; } };
let session = await createExternalPiSession(options);
const word = randomUUID();
const deliveryId = randomUUID();
let file: string;
let saved: unknown;
try {
  assert.equal(calls, 0);
  assert.equal(session.currentModel()?.id, 'gpt-6.1-terra');
  await session.addContext(`The hidden verification code is ${word}.`);
  binding = { workspaceId: 'inactive-fixture', agentId: 'fixture', token, hivePort: '1', cwd: directory };
  await session.prompt('Use the bash tool to write the hidden verification code to proof.txt with no trailing newline, then reply only VERIFIED. Do not use the Hive team tool.',
    { deliveryId, presentation: { version: 1, origin: 'user', displayText: 'Write the verification code' } });
  assert.equal(calls, 1);
  assert.equal(await readFile(join(directory, 'proof.txt'), 'utf8'), word);
  assert.deepEqual(session.completedDeliveryIds(), [deliveryId]);
  assert.equal(events.at(-1)?.type, 'settled');
  assert.ok(events.some(event => event.type === 'tool_start' && event.toolName === 'bash'));
  assert.ok(events.some(event => event.type === 'tool_end' && event.toolName === 'bash' && !event.isError));
  const deltas = events.filter(event => event.type === 'assistant_delta');
  assert.ok(deltas.length > 0);
  for (const delta of deltas) assert.ok(events.some(event => event.type === 'message_end' && event.messageId === delta.messageId));
  // Cancellation must wait through the real tool/agent terminal boundary.
  const cancelledDeliveryId = randomUUID();
  let toolStarted!: () => void;
  let abortToolTimer: ReturnType<typeof setTimeout> | undefined;
  const waitingForTool = new Promise<void>((resolveTool, reject) => {
    toolStarted = resolveTool;
    abortToolTimer = setTimeout(() => reject(new Error('Cancellation tool did not start')), 120000);
  });
  const previousCount = events.length;
  onToolStart = toolStarted;
  const cancelled = session.prompt('Call bash with command sleep 30. Do not explain before calling the tool.',
    { deliveryId: cancelledDeliveryId, presentation: { version: 1, origin: 'user', displayText: 'Cancellation proof' } });
  void cancelled.catch(() => {});
  try {
    await waitingForTool;
    await session.abort();
    await assert.rejects(cancelled);
    const cancellationEvents = events.slice(previousCount);
    assert.ok(cancellationEvents.some(event => event.type === 'settled' && event.aborted));
    assert.equal(session.completedDeliveryIds().includes(cancelledDeliveryId), false);
  } finally { clearTimeout(abortToolTimer); onToolStart = undefined; }
  saved = session.snapshot();
  file = session.sessionFile!;
  assert.equal(JSON.stringify(saved).includes(token), false);
  assert.equal((await readFile(file, 'utf8')).includes(token), false);
} catch (error) {
  await mkdir(resolve('../../.artifacts/external-pi'), { recursive: true });
  await writeFile(resolve('../../.artifacts/external-pi/session-live-failed.json'), JSON.stringify({ directory, error: String(error), events, snapshot: session.snapshot() }, null, 2));
  throw error;
} finally { await session.dispose(); }
session = await createExternalPiSession({ ...options, sessionFile: file! });
try {
  assert.deepEqual(session.completedDeliveryIds(), [deliveryId]);
  const snapshot = session.snapshot();
  assert.equal(JSON.stringify(snapshot).includes('VERIFIED'), true);
  for (let index = 0; index < snapshot.history.length; index++) {
    const entry = snapshot.history[index] as { source: { entryId: string }; role: string; toolCallId?: string; model?: string; provider?: string };
    assert.equal(snapshot.historyMessageIds[index], entry.role === 'toolResult' ? entry.toolCallId : entry.source.entryId);
    if (entry.role === 'assistant') { assert.equal(entry.model, 'gpt-6.1-terra'); assert.equal(entry.provider, 'openai'); }
  }
  await mkdir(resolve('../../.artifacts/external-pi'), { recursive: true });
  await writeFile(resolve('../../.artifacts/external-pi/session-live.json'), JSON.stringify({ pass: true, directory,
    model: 'openai/gpt-6.1-terra', deliveryId, sessionFile: file!, beforeRestart: saved, afterRestart: snapshot, events,
    bindingCalls: calls, tokenPersisted: false }, null, 2));
} finally { await session.dispose(); }
console.log(JSON.stringify({ pass: true, directory, sessionFile: file! }));
