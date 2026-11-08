import assert from 'node:assert/strict';
import { CodexEvents } from '../src/agents/codex-events.js';
import { normalizeCodexItem, type CodexDelivery } from '../src/agents/codex-history.js';
import type { PiEvent } from '../src/agents/types.js';

const raw = { type: 'userMessage', id: 'native-input', content: [{ type: 'text', text: '<hive-dispatch>Full raw task envelope</hive-dispatch>', text_elements: [] }] };
const original = structuredClone(raw);
for (const origin of ['user', 'dispatch', 'report', 'recovery', 'cancel', 'system', 'unknown'] as const) {
  const delivery: CodexDelivery = { turnId: 'turn', deliveryId: `delivery-${origin}`,
    presentation: { version: 1, origin, displayText: 'Concise visible text', dispatchId: 'dispatch', workerName: 'worker' } };
  const message = normalizeCodexItem(raw, delivery);
  assert.equal(message.role, 'custom');
  assert.equal(message.customType, 'hive-delivery');
  assert.deepEqual(message.presentation, { ...delivery.presentation, deliveryId: delivery.deliveryId });
  assert.deepEqual(message.details, message.presentation);
  assert.deepEqual(message.content, [{ type: 'text', text: raw.content[0]!.text }]);
  assert.deepEqual(message.rawBackendItem, original);
  const events: PiEvent[] = [];
  const projector = new CodexEvents('thread', event => events.push(event), turnId => turnId === 'turn' ? delivery : undefined);
  projector.item(raw, 'turn', true);
  const ended = events.find(event => event.type === 'message_end');
  assert.ok(ended?.type === 'message_end');
  assert.deepEqual(ended.message, message);
  assert.deepEqual(projector.snapshot().history[0], message, 'Live and restored history must use the same native provenance contract');
}
const legacy = normalizeCodexItem(raw);
assert.equal(legacy.role, 'user', 'User-pasted envelopes without metadata remain ordinary input');
assert.equal(legacy.customType, undefined);
assert.equal(legacy.presentation, undefined);
assert.deepEqual(raw, original, 'Presentation must not modify the backend payload');
console.log('Codex normalization passed: native hive-delivery contract for all origins, live/snapshot parity, raw preservation, ordinary legacy input. No model calls.');


const events: PiEvent[] = [];
const tools = new CodexEvents('thread', event => events.push(event), () => undefined);
const command = { type: 'commandExecution', id: 'shell', command: 'git status', cwd: '/project' };
tools.item({ ...command, status: 'inProgress' }, 'turn', false);
tools.item({ ...command, status: 'completed', aggregatedOutput: 'clean' }, 'turn', true);
const start = events.find(event => event.type === 'tool_start');
assert.deepEqual(start?.args, { command: 'git status', cwd: '/project' });
const finished = tools.snapshot().history[0] as ReturnType<typeof normalizeCodexItem>;
assert.deepEqual(finished.toolInput, start?.args);
assert.deepEqual(finished.content, [{ type: 'text', text: 'clean' }]);
assert.equal(normalizeCodexItem({ ...command, status: 'declined' }).toolStatus, 'declined');
const result = normalizeCodexItem({ type: 'dynamicToolCall', tool: 'team', id: 'send',
  arguments: { args: ['list'] }, contentItems: [{ type: 'inputText', text: 'worker ready' }] });
assert.deepEqual(result.toolInput, { args: ['list'] });
assert.deepEqual(result.content, [{ type: 'text', text: 'worker ready' }]);

// Partial output and interruption must survive a UI snapshot and real projection restart.
const partialEvents: PiEvent[] = [];
const partial = new CodexEvents('thread', event => partialEvents.push(event), () => undefined);
const nativePending = {...command,id:'pending',status:'inProgress'};
partial.item(nativePending,'cancelled-turn',false);
partial.delta('item/commandExecution/outputDelta',{itemId:'pending',delta:'first\n'});
partial.delta('item/commandExecution/outputDelta',{itemId:'pending',delta:'second 🐝\n'});
const updates=partialEvents.filter(event=>event.type==='tool_update');
assert.deepEqual(updates.map(e=>e.result),[
  {content:[{type:'text',text:'first\n'}]}, {content:[{type:'text',text:'first\nsecond 🐝\n'}]},
]);
assert.deepEqual((partial.snapshot().history[0] as Record<string,unknown>).content,[{type:'text',text:'first\nsecond 🐝\n'}]);
partial.item({...command,id:'already-done',status:'inProgress'},'cancelled-turn',false);
partial.item({...command,id:'already-done',status:'completed',aggregatedOutput:'done'},'cancelled-turn',true);
assert.equal(partial.interruptedItems('other-turn').length,0);
const interrupted=partial.interruptedItems('cancelled-turn');
assert.equal(interrupted.length,1);
assert.equal(nativePending.status,'inProgress');
assert.equal('hiveInterrupted' in nativePending,false);
partial.item(interrupted[0]!,'cancelled-turn',true);
const interruptedMessage=partial.snapshot().history[0] as Record<string,unknown>;
assert.equal(interruptedMessage.toolStatus,'interrupted');
assert.deepEqual(interruptedMessage.content,[{type:'text',text:'first\nsecond 🐝\n'}]);
assert.equal(partial.interruptedItems('cancelled-turn').length,0);
partial.delta('item/commandExecution/outputDelta',{itemId:'pending',delta:'late'});
assert.deepEqual((partial.snapshot().history[0] as Record<string,unknown>).content,interruptedMessage.content);
const {mkdtemp,rm}=await import('node:fs/promises');const {tmpdir}=await import('node:os');const {join}=await import('node:path');
const {CodexProjection}=await import('../src/agents/codex-projection.js');
const state=await mkdtemp(join(tmpdir(),'hive-codex-partial-'));
try {
 const projection=new CodexProjection(state,'thread');projection.put(interrupted[0]!,'cancelled-turn');projection.complete('cancelled-turn');
 const recovered=new CodexProjection(state,'thread');recovered.recover([{id:'cancelled-turn',status:'interrupted',items:[nativePending]}],true);
 const restored=new CodexEvents('thread',()=>{},()=>undefined);restored.restore(recovered.current());
 assert.deepEqual(restored.snapshot().history[0],interruptedMessage);
 const imported=new CodexProjection(state,'imported');
 imported.recover([{id:'native-turn',status:'interrupted',items:[{...nativePending,aggregatedOutput:'native partial'}]}],true);
 const importedItem=normalizeCodexItem(imported.current()[0]!.item);
 assert.equal(importedItem.toolStatus,'interrupted');assert.deepEqual(importedItem.content,[{type:'text',text:'native partial'}]);
 assert.equal(nativePending.status,'inProgress');
 const background=new CodexProjection(state,'background');background.put(nativePending,'background-turn');background.complete('background-turn');
 const nativeCompleted={...nativePending,status:'completed',aggregatedOutput:'native final'};
 background.recover([{id:'background-turn',status:'completed',items:[nativeCompleted]}],true);
 assert.deepEqual(background.current()[0]!.item,nativeCompleted);
}finally{await rm(state,{recursive:true,force:true});}
console.log('Codex partial output and interrupted-tool projection/restart checks passed. No model calls.');
