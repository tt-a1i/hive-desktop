import { strict as assert } from 'node:assert';
import { mkdtemp, mkdir, readFile, rename, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { CodexProjection } from '../src/agents/codex-projection.js';
import { codexMetadataFile, writeCodexMetadata } from '../src/agents/codex-metadata.js';
import { searchConversation } from '../src/conversation-search.js';
import { desktopHistoryMessage } from '../src/desktop-history.js';
import type { ChatRecord } from '../src/chat-index.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-search-codex-'));
try {
  const text = 'History body keeps one display copy and complete search text';
  const native = {type: 'userMessage', id: 'native-user-id', hiveInterrupted: true, content: text};
  const message = {
    role: 'user', content: [{type: 'text', text}],
    rawBackendItem: native, handoffItemId: native.id, sourceSessionId: 'source-thread',
    details: {patch: 'native patch'}, toolInput: {command: 'git status'},
  };
  assert.deepEqual(desktopHistoryMessage(message, 'desktop-user-id'), {
    ...message, rawBackendItem: {type: native.type, id: native.id, hiveInterrupted: true},
  });
  assert.equal(message.rawBackendItem.content, text);
  const legacy = {role:'notice', rawBackendItem: native};
  assert.deepEqual(desktopHistoryMessage(legacy, 'legacy'), legacy, 'JSON-rendered legacy notices retain their source fields');
  const record: ChatRecord = { id: 's', workspaceId: 'w', title: '', backend: 'codex', codexThreadId: 'native', sessionFile: null, updatedAt: 1 };
  const options = {codexDirectory: dir, claudeDirectory: dir};
  const projection = new CodexProjection(dir, 'native');
  const turn = (id: string, text: string) => ({id, status: 'completed', items: [{id: `${id}-item`, type: 'agentMessage', text}]});
  projection.recover([turn('first', 'hello'), turn('second', 'discard this')], true);
  let result = await searchConversation(record, 'hello', options);
  assert.equal(result.complete, true);
  assert.equal(result.matches[0]?.messageId, 'first-item');
  projection.recover([turn('first', 'changed')], true);
  result = await searchConversation(record, 'discard', {...options, liveHistory: {history: [{role: 'assistant', content: 'discard this'}], historyMessageIds: ['second-item']}});
  assert.equal(result.matches.length, 0, 'an obsolete live display entry cannot revive a removed native turn');
  assert.equal(result.complete, true);
  assert.equal((await searchConversation(record, 'changed', options)).matches[0]?.messageId, 'first-item');
  projection.put({id: 'new', type: 'agentMessage', text: 'stream'}, 'third');
  assert.equal((await searchConversation(record, 'stream', options)).complete, false);
  projection.recover([turn('first', 'changed')], true);
  projection.invalidateHistory();
  assert.equal((await searchConversation(record, 'changed', options)).complete, false);
  projection.recover([turn('first', 'legacy')]);
  assert.equal((await searchConversation(record, 'legacy', options)).complete, false);
  projection.recover([{...turn('inflight', 'partial'), status: 'inProgress'}], true);
  assert.equal((await searchConversation(record, 'partial', options)).complete, false);
  projection.recover([turn('duplicate', 'a'), turn('duplicate', 'b')], true);
  assert.equal((await searchConversation(record, 'a', options)).complete, false);
  const saved = JSON.parse(await readFile(`${codexMetadataFile(dir, 'native')}.display.json`, 'utf8'));
  assert.notEqual(saved.completeHistory, true);
  // A settled UI cache may have missed a native item even after completion.
  const coherent = new CodexProjection(dir, 'coherent');
  coherent.put({id:'u', type:'userMessage', content:[{type:'text', text:'outdated user'}]}, 'turn');
  coherent.put({id:'gone', type:'agentMessage', text:'obsolete branch content'}, 'turn');
  coherent.put({id:'tool', type:'commandExecution', status:'inProgress', hiveInterrupted:true, command:'old', aggregatedOutput:'cached output', extraToolDetail:'retained'}, 'turn');
  coherent.complete('turn');
  coherent.recover([{id:'turn', status:'completed', items:[
    {id:'u', type:'userMessage', content:[{type:'text', text:'updated user'}]},
    {id:'tool', type:'commandExecution', status:'completed', command:'new'},
    {id:'a', type:'agentMessage', text:'needle'},
  ]}], true);
  const visible = coherent.current();
  assert.deepEqual(visible.map(entry => entry.item.id), ['u', 'tool', 'a']);
  assert.deepEqual(visible[0]!.item.content, [{type:'text', text:'updated user'}]);
  assert.equal(visible[1]!.item.command, 'new');
  assert.equal(visible[1]!.item.aggregatedOutput, 'cached output');
  assert.equal(visible[1]!.item.extraToolDetail, 'retained');
  assert.equal(visible[1]!.item.hiveInterrupted, undefined, 'native completed state clears stale interruption');
  const coherentRecord = {...record, codexThreadId:'coherent'};
  const found = await searchConversation(coherentRecord, 'needle', options);
  assert.equal(found.complete, true);
  assert.deepEqual(found.matches.map(match => match.messageId), ['a']);
  assert.ok(found.matches.every(match => visible.some(entry => entry.item.id === match.messageId)), 'all search hits exist in the visible projection');
  assert.equal((await searchConversation(coherentRecord, 'obsolete', options)).matches.length, 0);
  assert.equal((await searchConversation(coherentRecord, 'outdated', options)).matches.length, 0);

  const fork = new CodexProjection(dir, 'fork');
  const user = (id: string) => ({id, type: 'userMessage', content: [{type: 'text', text: `Native raw ${id}`}]});
  fork.recover([
    {id:'inherited-turn', status:'completed', items:[user('inherited-user')]},
    {id:'local-turn', status:'completed', items:[user('exact-user'), user('legacy-user'),
      {id:'assistant', type:'agentMessage', text:'Visible assistant provenance needle'}]},
  ], true);
  const presentation = (displayText: string) => ({version: 1 as const, origin: 'user' as const, displayText});
  const delivery = (turnId: string, itemId: string | undefined, deliveryId: string, displayText: string) =>
    ({turnId, itemId, deliveryId, presentation: presentation(displayText)});
  writeCodexMetadata(dir, 'fork', {version:1, context:[], completed:[],
    inheritedDeliveries:[
      delivery('inherited-turn', 'inherited-user', 'first-inherited', 'Visible inherited provenance needle'),
      delivery('inherited-turn', 'inherited-user', 'duplicate-inherited', 'Suppressed inherited duplicate'),
    ],
    deliveries:[
      delivery('inherited-turn', 'inherited-user', 'duplicate-local', 'Suppressed local duplicate'),
      delivery('inherited-turn', 'legacy-user', 'wrong-turn', 'Suppressed wrong turn'),
      delivery('local-turn', undefined, 'first-legacy', 'Visible legacy provenance needle'),
      delivery('local-turn', undefined, 'duplicate-legacy', 'Suppressed legacy duplicate'),
      delivery('local-turn', 'exact-user', 'first-exact', 'Visible exact provenance needle'),
      delivery('local-turn', 'exact-user', 'duplicate-exact', 'Suppressed exact duplicate'),
    ],
  });
  const forkRecord = {...record, codexThreadId:'fork'};
  const forkResult = await searchConversation(forkRecord, 'provenance needle', options);
  assert.equal(forkResult.complete, true);
  assert.deepEqual(forkResult.matches.map(match => match.messageId), ['first-inherited', 'first-exact', 'first-legacy', 'assistant']);
  assert.deepEqual((await searchConversation(forkRecord, 'suppressed', options)).matches, []);
  // A failed first write must not create a false requirement; retry succeeds.
  const failedPath = codexMetadataFile(dir, 'first-write-failure');
  const oneDelivery = {version:1 as const, context:[], completed:[], deliveries:[{turnId:'turn',
    itemId:'item', deliveryId:'delivery', presentation:{version:1 as const, origin:'user' as const, displayText:'message'}}]};
  await mkdir(failedPath);
  assert.throws(() => writeCodexMetadata(dir, 'first-write-failure', oneDelivery));
  await assert.rejects(stat(failedPath + '.required'), {code:'ENOENT'});
  await rm(failedPath, {recursive:true});
  writeCodexMetadata(dir, 'first-write-failure', oneDelivery);
  assert.ok((await stat(failedPath + '.required')).isFile());

  // Persisted Hive presentation is needed to distinguish its user-visible
  // message from native input carrying a private attachment copy path.
  const guarded = new CodexProjection(dir, 'guarded');
  guarded.recover([{id:'turn', status:'completed', items:[{id:'native-id', type:'userMessage',
    content:[{type:'text', text:'user phrase\n\nprivate attachment /fixture/secret.txt'}]}]}], true);
  const attachment = {id:'11111111-1111-4111-8111-111111111111', workspaceId:'w', name:'file.txt',
    mimeType:'application/octet-stream', size:5, projectFile:{relativePath:'file.txt', sha256:'a'.repeat(64), sourceSessionId:'s'}};
  writeCodexMetadata(dir, 'guarded', {version:1, context:[], completed:[], deliveries:[{turnId:'turn',
    itemId:'native-id', deliveryId:'delivery-id', presentation:{version:1, origin:'user', displayText:'user phrase', attachments:[attachment]}}]});
  const guardedRecord = {...record, codexThreadId:'guarded'};
  const original = await searchConversation(guardedRecord, 'user phrase', options);
  assert.deepEqual(original.matches, [{messageId:'delivery-id', snippet:'user phrase'}]);
  assert.equal(original.complete, true);
  const metadataPath = codexMetadataFile(dir, 'guarded');
  await rename(metadataPath, metadataPath + '.held');
  const unavailable = await searchConversation(guardedRecord, 'private', {...options,
    liveHistory:{history:[{role:'user', content:'private attachment /fixture/secret.txt'}], historyMessageIds:['native-id']}});
  assert.deepEqual(unavailable.matches, []);
  assert.equal(unavailable.complete, false);
  assert.match(unavailable.warning!, /Restore the original metadata file/);
  await rename(metadataPath + '.held', metadataPath);
  assert.deepEqual(await searchConversation(guardedRecord, 'user phrase', options), original);
  // A native-only legacy thread has never had a Hive metadata requirement.
  const legacyNative = new CodexProjection(dir, 'native-only');
  legacyNative.recover([{id:'old-turn', status:'completed', items:[{id:'old-id', type:'userMessage',
    content:[{type:'text', text:'legacy native text'}]}]}], true);
  assert.deepEqual(await searchConversation({...record, codexThreadId:'native-only'}, 'legacy native', options),
    {matches:[{messageId:'old-id', snippet:'legacy native text'}], complete:true});
  console.log('Codex search snapshot completeness checks passed');
} finally { await rm(dir, {recursive:true, force:true}); }
