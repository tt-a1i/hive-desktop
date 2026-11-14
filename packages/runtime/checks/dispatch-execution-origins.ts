import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DispatchExecutionOrigins } from '../src/dispatch-execution-origins.js';

const root = mkdtempSync(join(tmpdir(), 'hive-dispatch-origins-'));
try {
  const directory = join(root, 'sources');
  const origins = new DispatchExecutionOrigins(directory);
  const source = {workspaceId: 'workspace/../safe', dispatchId: '../../dispatch', sourceSessionId: 'A', sourceAgentId: 'worker'};
  assert.equal(origins.get(source.workspaceId, source.dispatchId), undefined);
  origins.put(source); origins.put({...source});
  const files = readdirSync(directory); assert.equal(files.length, 1); assert.match(files[0]!, /^[a-f0-9]{64}\.json$/);
  const bytes = readFileSync(join(directory, files[0]!));
  assert.deepEqual(new DispatchExecutionOrigins(directory).get(source.workspaceId, source.dispatchId), source);
  assert.throws(() => origins.put({...source, sourceSessionId: 'B'}), /DISPATCH_ORIGIN_CONFLICT/);
  assert.deepEqual(readFileSync(join(directory, files[0]!)), bytes);
  assert.equal(origins.get('other-workspace', source.dispatchId), undefined);
  writeFileSync(join(directory, files[0]!), 'broken');
  assert.throws(() => origins.get(source.workspaceId, source.dispatchId), /DISPATCH_ORIGIN_UNREADABLE/);
  assert.throws(() => origins.put(source), /DISPATCH_ORIGIN_UNREADABLE/);
  writeFileSync(join(directory, files[0]!), bytes);
  renameSync(directory, directory + '.held'); writeFileSync(directory, 'not a directory');
  assert.throws(() => origins.put({...source, dispatchId: 'new'}), /DISPATCH_ORIGIN_(UNREADABLE|WRITE_FAILED)/);
  rmSync(directory); renameSync(directory + '.held', directory);
  assert.deepEqual(origins.get(source.workspaceId, source.dispatchId), source);
  console.log('DISPATCH_EXECUTION_ORIGINS_OK: immutable/cold/idempotent/hash-safe sources and fail-closed corrupt/write-failure handling.');
} finally { rmSync(root, {recursive: true, force: true}); }
