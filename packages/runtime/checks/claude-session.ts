/** Real original CLI + official SDK init/control/MCP; sends only shouldQuery:false context. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createClaudeSession } from '../src/agents/claude.js';
import type { PiEvent } from '../src/agents/types.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-claude-session-'));
const events: PiEvent[] = [];
let bindingCalls = 0;
let exits = 0;
const options = { cwd: directory, sessionDir: directory, command: process.env.HIVE_CLAUDE_COMMAND ?? '/Users/tsk/.npm-global/bin/claude',
  onEvent: (event: PiEvent) => { events.push(event); }, teamBinding: () => { bindingCalls++; return undefined; }, onUnexpectedExit: () => { exits++; } };
let session = await createClaudeSession(options);
let sessionId: string;
let ids: string[];
try {
  sessionId = session.sessionId;
  assert.equal(session.history().length, 0);
  assert.equal(bindingCalls, 0);
  assert.ok(session.catalog().models.length > 0);
  assert.ok(session.catalog().mcpServers.some(server => server.status === 'connected'));
  const model = session.catalog().models[0]!;
  await session.setModel('claude', model); // Control-only, never sends a query.
  assert.equal(session.currentModel()?.id, model);
  await session.abort();
  await session.addContext('Hidden Hive context\nUnicode \u2028 and \u2029', 'hive-context');
  assert.equal(session.hasContext('hive-context'), true);
  assert.equal(session.history().some(message => (message as { role?: string }).role === 'assistant'), false);
  ids = session.snapshot().historyMessageIds;
  assert.equal(ids.length, 1);
  assert.equal(bindingCalls, 0);
  assert.deepEqual(session.completedDeliveryIds(), []);
} finally { await session.dispose(); }
assert.equal(exits, 0);
session = await createClaudeSession({ ...options, sessionId: sessionId! });
try {
  assert.equal(session.sessionId, sessionId!);
  assert.equal(session.hasContext('hive-context'), true);
  assert.deepEqual(session.snapshot().historyMessageIds, ids!);
  assert.equal(bindingCalls, 0);
} finally { await session.dispose(); }
assert.equal(exits, 0);
await mkdir(resolve('../../.artifacts/claude'), { recursive: true });
await writeFile(resolve('../../.artifacts/claude/session.json'), JSON.stringify({ pass: true, inference: false, sdkVersion: '0.3.317', cliVersion: '2.1.317', directory,
  sessionId: sessionId!, ids: ids!, bindingCalls, unexpectedExits: exits, events }, null, 2));
console.log(JSON.stringify({ pass: true, inference: false, directory, sessionId: sessionId! }));
