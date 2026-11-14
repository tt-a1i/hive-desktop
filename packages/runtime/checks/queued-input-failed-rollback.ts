// Real durable queues with deterministic native lifecycle and ledger fault fixtures.
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Chats } from '../src/chats.js';
import { DeliveryQueue } from '../src/delivery-queue.js';
const directory = mkdtempSync(join(tmpdir(), 'hive-failed-rollback-'));
try {
 for (const mode of ['edit', 'edit-busy', 'cancel-committed'] as const) {
  const id = mode, record = { id, workspaceId: 'workspace', agentId: 'workspace:orchestrator' };
  const executed: string[] = [], completed: string[] = [];
  let releaseHead!: () => void, enteredHead!: () => void;
  const headEntered = new Promise<void>(resolve => { enteredHead = resolve; });
  const headBlocked = new Promise<void>(resolve => { releaseHead = resolve; });
  let aborted = 0, disposed = 0, exited = 0, loads = 0;
  const rows = new Map<number, { type: 'user_input' | 'user_input_cancelled'; text: string }>([[1, {type:'user_input',text:'head'}], [2, {type:'user_input',text:'original'}]]);
  const chats: any = Object.create(Chats.prototype);
  Object.assign(chats, { closing: false, removing: new Set(), activating: new Map(), live: new Map(), opening: new Map(), mutatingQueues: new Set(), sequences: new Map(), emit: () => {},
   index: { get: () => record, peek: () => record },
   hive: { getUserInput: (_w: string,_a: string,s: number) => rows.get(s), mutateUserInput: (_w: string,_a: string,s: number,_e: string,replacement: string | null,persist: () => void) => {
    const before = rows.get(s)!;
    rows.set(s, {type:replacement === null ? 'user_input_cancelled' : 'user_input',text:replacement ?? before.text});
    try { persist(); if (mode !== 'cancel-committed') throw new Error('SQL commit failed'); }
    catch (error) { rows.set(s,before); throw error; }
   } },
  });
  const makeQueue = () => new DeliveryQueue({directory,sessionId:id,startPaused:true,completedIds:()=>completed,onError:()=>{}, execute: async item => {
   if (item.text === 'head') { enteredHead(); await headBlocked; }
   else executed.push(item.text);
   completed.push(item.id);
  }});
  const independent: string[] = [];
  const otherQueue = new DeliveryQueue({ directory, sessionId: id + '-independent', startPaused: true, completedIds: () => [], onError: error => { throw error; }, execute: async item => { independent.push(item.text); } });
  await otherQueue.enqueue('unrelated chat', true, { version: 1, origin: 'user', displayText: 'unrelated chat' });
  let queue = makeQueue();
  const live = {queue,handle:{abort:async()=>{aborted++;releaseHead();},dispose:async()=>{disposed++;}},aborted:false,onExit:()=>{exited++;}};
  chats.live.set(id,live);
  chats.load = async () => { loads++; const restored = makeQueue(); chats.reconcileUserInputs(record,restored); const value={queue:restored,handle:live.handle,aborted:false};chats.live.set(id,value);return value; };
  chats.activateOnce = async () => {};
  if (mode === 'edit-busy') { await queue.enqueue('head',true,{version:1,origin:'user',displayText:'head',messageSequence:1});queue.resume();await headEntered; }
  await queue.enqueue('original',true,{version:1,origin:'user',displayText:'original',messageSequence:2});
  const target=queue.pending.find(item=>item.text==='original')!;
  // First rename succeeds; all immediate recovery/final cleanup writes fail.
  queue.reconcilePendingUserInput=()=>{throw new Error('rollback spool write failed');};
  await assert.rejects(chats.mutateQueuedInput(id,target.id,'original',mode==='cancel-committed'?undefined:'UNCOMMITTED'), /failed/);
  await live.cleanup;
  otherQueue.resume();await otherQueue.idle();assert.deepEqual(independent,['unrelated chat']);
  assert.equal(aborted,1);assert.equal(disposed,1);assert.equal(exited,1);
  // Disk is healthy for the newly owned queue. Resume must reload/reconcile it,
  // never resume the previous queue that contains edited text or a tombstone.
  await chats.resume(id);queue=chats.live.get(id).queue;await queue.idle();
  assert.equal(loads,1);assert.ok(!executed.includes('UNCOMMITTED'));
  assert.deepEqual(executed,mode==='cancel-committed'?[]:['original']);
  assert.equal(rows.get(2)!.type,mode==='cancel-committed'?'user_input_cancelled':'user_input');
 }
 console.log('QUEUED_INPUT_FAILED_ROLLBACK_OK: failed reconciliation retires owned native handle; resume reloads authoritative original, including active-head interruption and committed-cancel cleanup faults.');
} finally {rmSync(directory,{recursive:true,force:true});}
