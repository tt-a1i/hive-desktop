import { strict as assert } from 'node:assert';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { CodexProjection } from '../src/agents/codex-projection.js';
import { codexMetadataFile } from '../src/agents/codex-metadata.js';
import { searchConversation } from '../src/conversation-search.js';
import type { ChatRecord } from '../src/chat-index.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-search-codex-'));
try {
  const record: ChatRecord = { id: 's', workspaceId: 'w', title: '', backend: 'codex', codexThreadId: 'native', sessionFile: null, updatedAt: 1 };
  const options = {workspacePath: dir, codexDirectory: dir, claudeDirectory: dir};
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
  console.log('Codex search snapshot completeness checks passed');
} finally { await rm(dir, {recursive:true, force:true}); }
