import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  boundOversizedToolOutput,
  readJsonlTailWindow,
  readToolOutputByteWindow,
  sliceHistoryWindow,
} from '../src/history-cursor.js';
import { readPiTranscriptTailWindow } from '../src/agents/external-pi-history.js';

const total = 1500;
const history = Array.from({ length: total }, (_, i) => ({ role: i % 2 === 0 ? 'user' : 'assistant', content: `msg-${i}` }));
const ids = Array.from({ length: total }, (_, i) => `m-${i}`);

// 1. Default initial open returns latest 120 messages with accurate cursor metadata.
const initial = sliceHistoryWindow(history, ids, { limit: 120 });
assert.equal(initial.history.length, 120);
assert.equal(initial.historyMessageIds[0], 'm-1380');
assert.equal(initial.historyMessageIds[119], 'm-1499');
assert.deepEqual(initial.historyPage, {
  totalCount: 1500,
  startIndex: 1380,
  hasOlder: true,
  nextBeforeIndex: 1380,
  nextBeforeMessageId: 'm-1380',
  hasNewer: false,
});

// 2. Old search and reading anchors load a bounded page with both cursors.
const anchored = sliceHistoryWindow(history, ids, { limit: 120, aroundMessageId: 'm-420' });
assert.deepEqual(anchored.historyMessageIds, ids.slice(400, 520));
assert.deepEqual(anchored.historyPage, {
  totalCount: 1500,
  startIndex: 400,
  hasOlder: true,
  nextBeforeIndex: 400,
  nextBeforeMessageId: 'm-400',
  hasNewer: true,
  nextAfterIndex: 519,
  nextAfterMessageId: 'm-519',
});
assert.deepEqual(sliceHistoryWindow(history, ids, {limit: 1, aroundMessageId: 'm-420'}).historyMessageIds, ['m-420']);
assert.deepEqual(sliceHistoryWindow(history, ids, {limit: 3, aroundMessageId: 'm-0'}).historyMessageIds, ids.slice(0, 3));
assert.deepEqual(sliceHistoryWindow(history, ids, {limit: 120, aroundMessageId: 'm-1480'}), initial);
assert.deepEqual(sliceHistoryWindow(history, ids, {limit: 120, aroundMessageId: 'missing'}), initial);
const deliveries = history.map((item, index) => index === 420
  ? {...item, presentation: {deliveryId: 'delivery-target'}} : item);
assert.deepEqual(sliceHistoryWindow(deliveries, ids, {limit: 1, aroundMessageId: 'delivery-target'}).historyMessageIds, ['m-420']);

const focusedWalk = [...anchored.historyMessageIds];
let newer = anchored.historyPage;
while (newer.nextAfterIndex !== undefined) {
  const next = sliceHistoryWindow(history, ids, {
    limit: 120,
    afterIndex: newer.nextAfterIndex,
    afterMessageId: newer.nextAfterMessageId,
  });
  assert.ok(next.history.length <= 120);
  focusedWalk.push(...next.historyMessageIds);
  newer = next.historyPage;
}
assert.equal(newer.hasNewer, false);
assert.deepEqual(focusedWalk, ids.slice(400));

// 3. Backward pagination walks to index 0 with zero gaps or duplicates.
const collected: string[] = [...initial.historyMessageIds];
let cursor = initial.historyPage;
while (cursor.hasOlder && cursor.nextBeforeIndex !== undefined) {
  const prev = sliceHistoryWindow(history, ids, {
    limit: 120,
    beforeIndex: cursor.nextBeforeIndex,
    ...(cursor.nextBeforeMessageId ? { beforeMessageId: cursor.nextBeforeMessageId } : {}),
  });
  collected.unshift(...prev.historyMessageIds);
  cursor = prev.historyPage;
}
assert.equal(cursor.hasOlder, false);
assert.equal(cursor.startIndex, 0);
assert.deepEqual(collected, ids);

// 4. Shifted beforeMessageId re-anchors safely without duplicating messages.
const shiftedHistory = [{ role: 'user', content: 'prepended' }, ...history];
const shiftedIds = ['m-new-0', ...ids];
const reanchored = sliceHistoryWindow(shiftedHistory, shiftedIds, {
  limit: 10,
  beforeIndex: 1380,
  beforeMessageId: 'm-1380',
});
assert.equal(reanchored.historyMessageIds.at(-1), 'm-1379');
assert.ok(!reanchored.historyMessageIds.includes('m-1380'));
const shiftedForward = sliceHistoryWindow(shiftedHistory, shiftedIds, {
  limit: 10,
  afterIndex: anchored.historyPage.nextAfterIndex,
  afterMessageId: anchored.historyPage.nextAfterMessageId,
});
assert.deepEqual(shiftedForward.historyMessageIds, ids.slice(520, 530));

// 5. Oversized toolResult output is bounded on paginated reads while user/assistant messages stay untouched.
const hugeTool = {
  role: 'toolResult',
  content: [{ type: 'text', text: 'A'.repeat(40_000) }],
};
const boundedTool = boundOversizedToolOutput(hugeTool, 1024);
assert.ok(boundedTool.content[0]!.text.length < 1200);
assert.ok(boundedTool.content[0]!.text.includes('已截断 38976 字符'));
const normalAssistant = { role: 'assistant', content: [{ type: 'text', text: 'B'.repeat(40_000) }] };
assert.equal(boundOversizedToolOutput(normalAssistant, 1024), normalAssistant);

// 6. UTF-8 safe byte-window streaming reconstructs multi-byte Chinese/emoji tool output byte-for-byte.
const multiByteOutput = '中文日志输出🚀'.repeat(600);
let offset = 0;
let reconstructed = '';
let chunks = 0;
while (true) {
  const win = readToolOutputByteWindow(multiByteOutput, offset, 2048);
  reconstructed += win.chunk;
  chunks++;
  if (!win.hasMore) {
    assert.equal(win.nextByteOffset, Buffer.byteLength(multiByteOutput, 'utf8'));
    break;
  }
  assert.ok(win.nextByteOffset > offset);
  offset = win.nextByteOffset;
}
assert.ok(chunks >= 4);
assert.equal(reconstructed, multiByteOutput);

// 7. True reverse byte-chunked JSONL reader walks a multi-chunk file backwards to byte 0.
const tmp = mkdtempSync(join(tmpdir(), 'hive-jsonl-tail-'));
try {
  const piFile = join(tmp, 'session.jsonl');
  const lines: string[] = [
    JSON.stringify({ type: 'session', id: 'pi-tail-test', timestamp: new Date().toISOString() }),
  ];
  let parentId: string | null = null;
  for (let i = 0; i < 240; i++) {
    const id = `e-${i}`;
    lines.push(JSON.stringify({
      type: 'message',
      id,
      parentId,
      timestamp: new Date().toISOString(),
      message: { role: i % 2 === 0 ? 'user' : 'assistant', content: [{ type: 'text', text: `line-${i}-${'X'.repeat(500)}` }] },
    }));
    parentId = id;
  }
  writeFileSync(piFile, `${lines.join('\n')}\n`, 'utf8');

  const tail1 = await readJsonlTailWindow(piFile, { maxLines: 50, chunkBytes: 8192 });
  assert.equal(tail1.lines.length, 50);
  assert.equal(tail1.hasEarlierBytes, true);
  assert.equal((JSON.parse(tail1.lines[49]!) as { id: string }).id, 'e-239');
  assert.equal((JSON.parse(tail1.lines[0]!) as { id: string }).id, 'e-190');

  const tail2 = await readJsonlTailWindow(piFile, { maxLines: 50, beforeByteOffset: tail1.startByteOffset, chunkBytes: 8192 });
  assert.equal(tail2.lines.length, 50);
  assert.equal((JSON.parse(tail2.lines[49]!) as { id: string }).id, 'e-189');

  const piTail = await readPiTranscriptTailWindow(piFile, { maxEntries: 30, expectedId: 'pi-tail-test' });
  assert.ok(piTail.entries.length >= 30);
  assert.equal(piTail.entries.at(-1)?.id, 'e-239');
  assert.equal(piTail.hasEarlierBytes, true);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

console.log('HISTORY_CURSOR_OK: bounded tail and focused opens, full walks in both directions, shifted-ID reanchoring, UTF-8 byte windows, and reverse JSONL reads verified.');
