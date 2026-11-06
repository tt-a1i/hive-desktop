import assert from 'node:assert/strict';
import { CodexRpcError, CodexTransport } from '../src/agents/codex-transport.js';

// Metadata only: no thread, turn, model inference, or credential-file access.
let closeCount = 0;
const transport = await CodexTransport.start({
  command: process.env.HIVE_CODEX_COMMAND ?? 'codex',
  cwd: process.cwd(),
  onClose: () => { closeCount++; },
});
const pid = transport.pid;
assert.ok(pid);
let terraAvailable = false;
try {
  let cursor: string | null = null;
  do {
    const result: { data: { id: string; model: string }[]; nextCursor: string | null } =
      await transport.sendRequest('model/list', { limit: 100, includeHidden: true, cursor });
    assert.ok(Array.isArray(result.data));
    terraAvailable ||= result.data.some((model) => model.model === 'gpt-6.1-terra' || model.id === 'gpt-6.1-terra');
    cursor = result.nextCursor;
  } while (cursor);
  await assert.rejects(transport.sendRequest('hive/nonexistentTransportCheck', {}), (error: unknown) => {
    assert.ok(error instanceof CodexRpcError);
    assert.equal(error.code, -32600);
    assert.equal(typeof error.message, 'string');
    return true;
  });
} finally {
  await transport.close();
}
assert.equal(closeCount, 1);
assert.throws(() => process.kill(pid, 0), (error: NodeJS.ErrnoException) => error.code === 'ESRCH');
await assert.rejects(transport.sendRequest('model/list', {}), /closed/);
await transport.close(); // idempotent
// Kill only our second child to check unexpected-exit cleanup with a request outstanding.
let unexpectedCloseCount = 0;
const interrupted = await CodexTransport.start({
  command: process.env.HIVE_CODEX_COMMAND ?? 'codex', cwd: process.cwd(),
  onClose: () => { unexpectedCloseCount++; },
});
assert.ok(interrupted.pid);
process.kill(interrupted.pid, 'SIGKILL');
await assert.rejects(interrupted.sendRequest('model/list', {}), Error);
await interrupted.close();
assert.equal(unexpectedCloseCount, 1);
assert.throws(() => process.kill(interrupted.pid!, 0), (error: NodeJS.ErrnoException) => error.code === 'ESRCH');
await assert.rejects(CodexTransport.start({ command: '/hive-nonexistent-codex-command', cwd: process.cwd() }),
  (error: NodeJS.ErrnoException) => error.code === 'ENOENT');
console.log(JSON.stringify({ initialized: true, metadataOnly: true, terraAvailable, invalidMethod: -32600, childReaped: true, requestsAfterCloseRejected: true, unexpectedExitRejected: true, missingCommandRejected: true, windowsCliLaunch: 'unverified' }));
