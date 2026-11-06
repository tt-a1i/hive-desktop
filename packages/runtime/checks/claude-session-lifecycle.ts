/** Real original Claude process death; no user input and no inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createClaudeSession } from '../src/agents/claude.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-claude-lifecycle-'));
let callback!: (error: Error) => void;
let count = 0;
const exited = new Promise<Error>(resolve => { callback = resolve; });
const session = await createClaudeSession({ cwd: directory, sessionDir: directory,
  command: process.env.HIVE_CLAUDE_COMMAND ?? '/Users/tsk/.npm-global/bin/claude', onEvent() {},
  onUnexpectedExit(error) { count++; callback(error); } });
try {
  assert.ok(session.processId);
  process.kill(session.processId, 'SIGKILL');
  let timer: ReturnType<typeof setTimeout> | undefined;
  const error = await Promise.race([exited, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Claude death callback missing')), 5000); })]).finally(() => clearTimeout(timer));
  assert.ok(error instanceof Error);
  await assert.rejects(session.prompt('Must never be submitted'), candidate => candidate === error);
  assert.equal(count, 1);
} finally { await session.dispose(); }
assert.equal(count, 1);
await mkdir(resolve('../../.artifacts/claude'), { recursive: true });
await writeFile(resolve('../../.artifacts/claude/lifecycle.json'), JSON.stringify({ pass: true, inference: false, sentUserMessages: 0, directory, unexpectedExitCallbacks: count }, null, 2));
console.log('Claude actual process-death/owned-close checks passed without inference');
