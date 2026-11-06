/** Reopen a retained live transcript without inference to check presentation identifiers. */
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createExternalPiSession } from '../src/agents/external-pi.js';
const evidence = resolve('../../.artifacts/external-pi/session-live.json');
const receipt = JSON.parse(await readFile(evidence, 'utf8')) as { directory: string; sessionFile: string };
const session = await createExternalPiSession({ cwd: receipt.directory, sessionDir: receipt.directory, sessionFile: receipt.sessionFile,
  command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi', onEvent() {} });
try {
  const snapshot = session.snapshot();
  let results = 0;
  for (let index = 0; index < snapshot.history.length; index++) {
    const message = snapshot.history[index] as { role: string; toolCallId?: string; source: { entryId: string } };
    assert.equal(snapshot.historyMessageIds[index], message.role === 'toolResult' ? message.toolCallId : message.source.entryId);
    if (message.role === 'toolResult') results++;
  }
  assert.ok(results > 0);
  assert.deepEqual(Object.keys(session.currentModel()!).sort(), ['id', 'name', 'provider']);
  for (const model of session.listModels()) assert.deepEqual(Object.keys(model).sort(), ['available', 'id', 'name', 'provider']);
  await writeFile(resolve('../../.artifacts/external-pi/session-projection.json'), JSON.stringify({ pass: true, inference: false,
    sessionFile: receipt.sessionFile, toolResults: results, minimalModelFields: true }, null, 2));
  console.log('External Pi retained live transcript projection checked without inference');
} finally { await session.dispose(); }
