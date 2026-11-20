import assert from 'node:assert/strict';
import { mkdtemp, open, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { AcpHistory, openSavedAcpHistory } from '../src/agents/acp-history.js';
import { CodexProjection, openSavedCodexProjection } from '../src/agents/codex-projection.js';
import { codexMetadataFile, writeCodexMetadata } from '../src/agents/codex-metadata.js';
import { StaleHistorySnapshotError } from '../src/history-cursor.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-saved-history-pages-'));
const probeFile = join(dir, 'probe');
const probe = await open(probeFile, 'w');
const handlePrototype = Object.getPrototypeOf(probe);
const nativeRead = handlePrototype.read;
let bytesRead = 0;
handlePrototype.read = async function (...args: unknown[]) {
  const result = await nativeRead.apply(this, args);
  bytesRead += result.bytesRead;
  return result;
};
await probe.close();
try {
  const acpPath = join(dir, 'acp.json');
  const history = new AcpHistory(acpPath, {version: 1, sessionId: 'original-acp', history: [], historyIds: [], contexts: [], deliveries: {}});
  const large = '猫𐀀\\"\n{}[]'.repeat(10_000);
  await history.write(state => {
    state.history = Array.from({length: 125}, (_, i) => ({role: 'user', content: [{type: 'text', text: i < 110 ? large : `acp-${i}`}]}));
    state.historyIds = state.history.map((_, i) => `acp-row-${i}`);
    state.historyIds[25] = state.historyIds[24]!;
    state.history[18] = {role: 'user', content: large, presentation: {deliveryId: 'original-delivery'}};
    state.history[19] = {role: 'assistant', content: [{type: 'toolCall', id: 'unresolved-tool'}]};
    state.deliveries['original-delivery'] = 'sent';
    // Metadata also contains full text, but it must not enter the page cache.
    state.inputs = [{id: 'original-delivery', text: large, nativeText: large}];
    state.contexts = [{customType: 'context', text: large}];
  });
  const openAcp = (sessionId = 'original-acp', cacheKey = 'desktop-acp') => openSavedAcpHistory(acpPath, sessionId, {cacheKey});
  const acp = await openAcp();
  assert.equal(acp.deliveries['original-delivery'], 'sent');
  assert.equal((await acp.page({limit: 1, aroundMessageId: 'original-delivery'})).historyMessageIds[0], 'acp-row-18');
  assert.equal((await acp.page({limit: 1, aroundMessageId: 'unresolved-tool'})).historyMessageIds[0], 'acp-row-19');
  assert.deepEqual(await acp.byId('acp-row-18'), history.state.history[18]);
  assert.equal(await acp.byId('acp-row-24', {unique: true}), undefined);
  assert.deepEqual(await acp.byId('acp-row-24'), history.state.history[24]);
  await assert.rejects(acp.page({beforeIndex: 25, beforeMessageId: 'acp-row-25', snapshot: acp.snapshot}), StaleHistorySnapshotError);
  assert.equal((await acp.page({limit: 1, beforeIndex: 25, beforeMessageId: 'acp-row-24', snapshot: acp.snapshot})).historyPage.startIndex, 24);
  bytesRead = 0;
  const acpPage = await (await openAcp()).page({limit: 4, beforeIndex: 124, beforeMessageId: 'acp-row-124', snapshot: acp.snapshot});
  const acpPageBytes = bytesRead;
  assert.equal(acpPage.historyPage.totalCount, 125);
  assert.deepEqual(acpPage.history, history.state.history.slice(120, 124));
  assert(acpPageBytes > 0 && acpPageBytes < (await stat(acpPath)).size / 100);
  await assert.rejects(openAcp('different-acp'), /identity/);
  await assert.rejects(async () => (await openAcp('original-acp', 'other-workspace')).page({snapshot: acp.snapshot}), StaleHistorySnapshotError);
  await history.write(state => { state.history[120] = {...state.history[120] as object, edited: true}; });
  await assert.rejects(acp.page({snapshot: acp.snapshot}), StaleHistorySnapshotError);
  await assert.rejects(async () => (await openAcp()).page({snapshot: acp.snapshot}), StaleHistorySnapshotError);
  bytesRead = 0;
  await assert.rejects(openSavedAcpHistory(acpPath, 'original-acp', {cacheKey: 'desktop-acp', snapshot: acp.snapshot}), StaleHistorySnapshotError);
  assert.equal(bytesRead, 0);

  const thread = 'native-codex-thread', hiveSession = 'hive-session';
  const metadata = {
    version: 1 as const, context: [], completed: ['delivery'],
    deliveries: [{itemId: 'item-1', turnId: 'turn', deliveryId: 'delivery',
      presentation: {version: 1 as const, origin: 'user' as const, displayText: 'presented'}}],
    inheritedSources: [{itemId: 'item-1', sourceSessionId: 'parent-hive-session'}],
  };
  writeCodexMetadata(dir, thread, metadata);
  const nativeEntries = Array.from({length: 160}, (_, i) => ({id: `item-${i}`, type: 'agentMessage', text: i < 140 ? large : `codex-${i}`}));
  nativeEntries[1] = {id: 'item-1', type: 'userMessage', text: large};
  const projection = new CodexProjection(dir, thread);
  projection.recover([{id: 'turn', status: 'completed', items: nativeEntries}], true);
  const openCodex = () => openSavedCodexProjection(dir, thread, {cacheKey: 'desktop-codex', sourceSessionId: hiveSession});
  const codex = await openCodex();
  assert.equal(codex.completeHistory, true);
  assert.deepEqual(codex.completedDeliveryIds, ['delivery']);
  const alias = await codex.page({limit: 1, aroundMessageId: 'delivery'});
  assert.deepEqual(alias.historyMessageIds, ['item-1']);
  assert.equal((alias.history[0] as {sourceSessionId: string}).sourceSessionId, 'parent-hive-session');
  assert.equal((alias.history[0] as {presentation: {displayText: string}}).presentation.displayText, 'presented');
  bytesRead = 0;
  const codexPage = await (await openCodex()).page({limit: 4, snapshot: codex.snapshot, beforeIndex: 159, beforeMessageId: 'item-159'});
  const codexPageBytes = bytesRead;
  const displayFile = `${codexMetadataFile(dir, thread)}.display.json`;
  assert(codexPageBytes > 0 && codexPageBytes < (await stat(displayFile)).size / 100);
  assert.deepEqual(codexPage.historyMessageIds, ['item-155', 'item-156', 'item-157', 'item-158']);
  assert.equal((codexPage.history[0] as {sourceSessionId: string}).sourceSessionId, hiveSession);
  writeCodexMetadata(dir, thread, {...metadata, inheritedSources: []});
  await assert.rejects(codex.page({snapshot: codex.snapshot}), StaleHistorySnapshotError);
  await assert.rejects(async () => (await openCodex()).page({snapshot: codex.snapshot}), StaleHistorySnapshotError);

  await writeFile(displayFile, JSON.stringify({version: 1, entries: [{turnId: 'turn', item: nativeEntries[0]}]}));
  assert.equal((await openCodex()).completeHistory, false);
  await writeFile(displayFile, '{"version":1,"entries":[],}');
  await assert.rejects(openCodex, /Invalid saved history JSON/);
  await rm(codexMetadataFile(dir, thread));
  await assert.rejects(openCodex, /delivery metadata is missing/);
  console.log(`Saved transcript pages verified. Warm ACP page read ${acpPageBytes} bytes and Codex page read ${codexPageBytes} bytes.`);
} finally {
  handlePrototype.read = nativeRead;
  await rm(dir, {recursive: true, force: true});
}
