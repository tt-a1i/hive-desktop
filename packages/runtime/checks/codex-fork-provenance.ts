import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createForkCodexMetadata, readCodexMetadata, writeCodexMetadata } from '../src/agents/codex-metadata.js';
import { CodexProjection } from '../src/agents/codex-projection.js';
import { CodexEvents } from '../src/agents/codex-events.js';
import type { CodexTurn } from '../src/agents/codex-history.js';

const directory = mkdtempSync(join(tmpdir(), 'codex-fork-provenance-'));
try {
  const turns: CodexTurn[] = [
    {id:'t1', status:'completed', items:[{id:'u1',type:'userMessage',content:[{type:'text',text:'hello'}]}, {id:'tool1',type:'commandExecution',command:'pwd',status:'completed'}]},
    {id:'t2', status:'completed', items:[{id:'u2',type:'userMessage',content:[]}]},
  ];
  const presentation = {version: 1 as const, origin: 'user' as const, displayText:'hello'};
  writeCodexMetadata(directory, 'a', {version:1,context:[{text:'secret',customType:'context'}],completed:['executable'],deliveries:[
    {turnId:'t1',itemId:'u1',deliveryId:'delivery1',presentation}, {turnId:'t2',itemId:'u2',deliveryId:'delivery2',presentation},
  ]});
  createForkCodexMetadata(directory, 'a', 'b', 'session-a', turns.slice(0, 1));
  const b = readCodexMetadata(directory, 'b');
  assert.deepEqual(b.context, []); assert.deepEqual(b.completed, []); assert.deepEqual(b.deliveries, []);
  assert.deepEqual(b.inheritedDeliveries?.map(d => d.deliveryId), ['delivery1']);
  assert.deepEqual(b.inheritedSources, [{itemId:'u1',sourceSessionId:'session-a'}, {itemId:'tool1',sourceSessionId:'session-a'}]);
  const entries = new CodexProjection(directory, 'b').current();
  assert.deepEqual(entries.map(entry => entry.item.id), ['u1','tool1']);
  const events = new CodexEvents('b', () => {}, (turn, item) => b.inheritedDeliveries?.find(d => d.turnId === turn && d.itemId === item), item => b.inheritedSources?.find(s => s.itemId === item)?.sourceSessionId);
  events.restore(entries);
  const snapshot = events.snapshot();
  assert.equal(snapshot.history[0]?.itemId, 'u1'); assert.equal(snapshot.history[0]?.sourceSessionId, 'session-a');
  assert.equal(snapshot.history[0]?.role, 'custom'); assert.equal(snapshot.history[1]?.sourceSessionId, 'session-a');
  const bTurns = [...turns.slice(0,1), {id:'t3',status:'completed',items:[{id:'tool3',type:'commandExecution',status:'completed'}]}];
  createForkCodexMetadata(directory, 'b', 'c', 'session-b', bTurns);
  const c = readCodexMetadata(directory, 'c');
  assert.deepEqual(c.inheritedSources, [{itemId:'u1',sourceSessionId:'session-a'}, {itemId:'tool1',sourceSessionId:'session-a'}, {itemId:'tool3',sourceSessionId:'session-b'}]);
  assert.equal(c.inheritedDeliveries?.[0]?.deliveryId, 'delivery1');
  console.log('Codex fork provenance checks passed');
} finally { rmSync(directory, {recursive:true,force:true}); }
