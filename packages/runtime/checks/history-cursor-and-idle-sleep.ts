import assert from 'node:assert/strict';
import { boundOversizedToolOutput, sliceHistoryWindow } from '../src/history-cursor.js';

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
});

// 2. Search jump / reading anchor restore includes target message + preceding context through tail.
const anchored = sliceHistoryWindow(history, ids, { limit: 120, aroundMessageId: 'm-420' });
assert.equal(anchored.historyMessageIds[0], 'm-400');
assert.ok(anchored.historyMessageIds.includes('m-420'));
assert.equal(anchored.historyMessageIds.at(-1), 'm-1499');
assert.equal(anchored.historyPage.hasOlder, true);
assert.equal(anchored.historyPage.nextBeforeIndex, 400);
assert.equal(anchored.historyPage.nextBeforeMessageId, 'm-400');

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

console.log('HISTORY_CURSOR_OK: bounded open, anchored search/reading restore, full backward walk, shifted-ID reanchor, and oversized tool truncation verified.');
