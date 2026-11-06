/** Real external Pi session handle checks without inference. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createExternalPiSession } from '../src/agents/external-pi.js';
import type { PiEvent } from '../src/agents/types.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-external-session-'));
const events: PiEvent[] = [];
let callbacks = 0;
const options = { cwd: directory, sessionDir: directory, command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi',
  onEvent: (event: PiEvent) => { events.push(event); }, teamBinding: () => { callbacks++; return undefined; } };
let session = await createExternalPiSession(options);
const id = session.sessionId;
let file: string;
let historyIds: string[];
try {
  assert.ok(session.teamToolName.startsWith('hive_team_'));
  assert.equal(callbacks, 0);
  await session.addContext('Hidden routing\nUnicode \u2028 \u2029', 'hive-context');
  assert.equal(session.hasContext('hive-context'), true);
  assert.equal(callbacks, 0);
  const before = session.snapshot();
  assert.equal(before.history.length, 1);
  assert.equal(before.historyMessageIds.length, 1);
  const item = before.history[0] as { role: string; source: { backend: string; entryId: string }; rawBackendItem: { id: string } };
  assert.equal(item.role, 'custom'); assert.equal(item.source.backend, 'pi-rpc');
  assert.equal(item.source.entryId, item.rawBackendItem.id);
  assert.equal(before.historyMessageIds[0], item.source.entryId);
  historyIds = before.historyMessageIds;
  assert.deepEqual(session.completedDeliveryIds(), []);
  file = session.sessionFile!;
  assert.ok((await readFile(file, 'utf8')).includes('Hidden routing'));
  await session.abort();
} finally { await session.dispose(); }
session = await createExternalPiSession({ ...options, sessionFile: file! });
try {
  assert.equal(session.sessionId, id);
  assert.equal(session.hasContext('hive-context'), true);
  assert.deepEqual(session.snapshot().historyMessageIds, historyIds!);
  assert.equal(callbacks, 0);
} finally { await session.dispose(); }
const receipt = { pass: true, inference: false, directory, sessionId: id, sessionFile: file!, historyIds: historyIds!, events };
await mkdir(resolve('../../.artifacts/external-pi'), { recursive: true });
await writeFile(resolve('../../.artifacts/external-pi/session.json'), JSON.stringify(receipt, null, 2));
console.log(JSON.stringify({ pass: true, directory, sessionFile: file! }));
