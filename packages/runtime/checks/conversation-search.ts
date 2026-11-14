import { strict as assert } from 'node:assert';
import { mkdtemp, writeFile, appendFile, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { searchConversation } from '../src/conversation-search.js';
import { codexMetadataFile, writeCodexMetadata } from '../src/agents/codex-metadata.js';
import { projectClaudeMessage } from '../src/agents/claude-history.js';
import type { ChatRecord } from '../src/chat-index.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-search-'));
try {
  const file = join(dir, 'session.jsonl');
  const options = { workspacePath: dir, codexDirectory: dir, claudeDirectory: dir };
  const record: ChatRecord = { id: 's', workspaceId: 'w', title: 'test', backend: 'pi', sessionFile: file, updatedAt: 1 };
  const header = { type: 'session', id: 'session', version: 3, timestamp: '2026-01-01T00:00:00Z', cwd: dir };
  const entry = (id: string, parentId: string | null, text: string) => ({ type: 'message', id, parentId, timestamp: '2026-01-01T00:00:00Z', message: { role: 'user', content: [{type: 'text', text}], timestamp: 1 } });
  const lines = [header, entry('one', null, 'Hello 世界 café'), entry('abandoned', 'one', 'abandoned term'), entry('current', 'one', 'Current answer')];
  await writeFile(file, lines.map(v => JSON.stringify(v)).join('\n') + '\n');
  const search = (q: string) => searchConversation(record, q, options);
  let result = await search('世界 CAFE\u0301');
  assert.equal(result.complete, true);
  assert.equal(result.matches[0]?.messageId, 'one');
  assert.equal((await search('abandoned')).matches.length, 0);
  assert.equal((await search('current')).matches[0]?.messageId, 'current');
  assert.equal((await searchConversation({ ...record, backend: 'pi-rpc' }, 'current', options)).matches[0]?.messageId, 'current');
  const switched = await searchConversation(record, 'current', { ...options, liveHistory: { history: [{role: 'user', content: [{type: 'text', text: 'live branch'}]}], historyMessageIds: ['live-branch'] } });
  assert.equal(switched.matches.length, 0, 'live Pi branch replaces the old disk branch');
  await appendFile(file, JSON.stringify(entry('new', 'current', 'Fresh content')) + '\n');
  assert.equal((await search('fresh')).matches[0]?.messageId, 'new', 'appends are read on next search');
  const replaced = [...lines, entry('new', 'current', 'Other content')];
  await writeFile(file, replaced.map(v => JSON.stringify(v)).join('\n') + '\n');
  assert.equal((await search('other')).matches[0]?.messageId, 'new', 'same-size rewrites invalidate cached text');
  assert.equal((await search('fresh')).matches.length, 0);
  await appendFile(file, '{broken\n');
  result = await search('missing');
  assert.equal(result.complete, false);
  assert.match(result.warning!, /Malformed/);
  await writeFile(file, [header, entry('orphan', 'missing', 'target')].map(v => JSON.stringify(v)).join('\n'));
  assert.match((await search('target')).warning!, /parent is missing/);
  const many = Array.from({length: 3000}, (_, i) => entry(`e${i}`, i ? `e${i - 1}` : null, i === 2999 ? `${'𐀀'.repeat(500)} marker ${'x'.repeat(500)}` : 'ordinary'));
  await writeFile(file, [header, ...many].map(v => JSON.stringify(v)).join('\n'));
  result = await search('marker');
  assert.equal(result.complete, true);
  assert.equal(result.matches[0]?.messageId, 'e2999');
  assert.ok(Array.from(result.matches[0]!.snippet).length <= 222);
  assert.ok(result.matches[0]!.snippet.includes('marker'));

  const codex: ChatRecord = { ...record, backend: 'codex', sessionFile: null, codexThreadId: 'thread' };
  await mkdir(dir, { recursive: true });
  await writeFile(`${codexMetadataFile(dir, 'thread')}.display.json`, JSON.stringify({version: 1, entries: [{turnId: 't', item: {id: 'item', type: 'agentMessage', text: 'Journal content'}}], completedTurns: []}));
  result = await searchConversation(codex, 'journal', options);
  assert.equal(result.matches[0]?.messageId, 'item');
  assert.equal(result.complete, false);
  assert.match(result.warning!, /may omit/);
  result = await searchConversation(codex, 'live', { ...options, liveHistory: { history: [{role: 'assistant', content: [{type: 'text', text: 'live response'}]}], historyMessageIds: ['live-id'] } });
  assert.equal(result.matches[0]?.messageId, 'live-id');
  assert.equal(result.complete, false);
  const presentation = {version: 1 as const, origin: 'user' as const, displayText: 'cedar searchable'};
  const nativeUser = {id:'native-user', type:'userMessage', content:[{type:'text', text:'cedar searchable'}]};
  writeCodexMetadata(dir, 'thread', {version:1, context:[], completed:[], deliveries:[{turnId:'t', itemId:'native-user', deliveryId:'delivery-codex', presentation}]});
  await writeFile(`${codexMetadataFile(dir, 'thread')}.display.json`, JSON.stringify({version:1, entries:[{turnId:'t', item:nativeUser}], completedTurns:['t'], completeHistory:true, searchEntries:[{turnId:'t', item:nativeUser}]}));
  for (let i=0; i<2; i++) {
    result = await searchConversation(codex, 'cedar searchable', options);
    assert.equal(result.matches[0]?.messageId, 'delivery-codex', 'cold and cached Codex delivery use desktop ID');
  }
  const displayedUser = {role:'custom', customType:'hive-delivery', presentation:{...presentation, deliveryId:'delivery-codex'}, content:[{type:'text',text:'cedar searchable'}]};
  result = await searchConversation(codex, 'cedar', {...options, liveHistory:{history:[displayedUser], historyMessageIds:['native-user']}});
  assert.deepEqual(result.matches.map(match => match.messageId), ['delivery-codex']);
  // An incomplete Codex journal merges live messages by the desktop ID, not native ID.
  await writeFile(`${codexMetadataFile(dir, 'thread')}.display.json`, JSON.stringify({version:1, entries:[{turnId:'t', item:nativeUser}], completedTurns:[]}));
  result = await searchConversation(codex, 'cedar', {...options, liveHistory:{history:[displayedUser], historyMessageIds:['native-user']}});
  assert.deepEqual(result.matches.map(match => match.messageId), ['delivery-codex']);
  const marker = {type:'custom', id:'marker', parentId:null, timestamp:'2026-01-01T00:00:00Z', customType:'hive-user-input', data:{...presentation, deliveryId:'delivery-pi', rawText:'cedar searchable'}};
  await writeFile(file, [header, marker, entry('native-pi', 'marker', 'cedar searchable')].map(v => JSON.stringify(v)).join('\n'));
  for (const backend of ['pi','pi-rpc'] as const) {
    assert.deepEqual((await searchConversation({...record,backend}, 'cedar', options)).matches.map(m => m.messageId), ['delivery-pi']);
    const liveMessage = {role:'user', content:[{type:'text',text:'cedar searchable'}], presentation:{...presentation,deliveryId:'delivery-pi'}};
    assert.deepEqual((await searchConversation({...record,backend}, 'cedar', {...options,liveHistory:{history:[liveMessage],historyMessageIds:['native-pi']}})).matches.map(m=>m.messageId), ['delivery-pi']);
  }
  const claudeMessage = projectClaudeMessage({type:'user',uuid:'native-claude',session_id:'session',parent_tool_use_id:null,message:{role:'user',content:'cedar searchable'}}, {version:1, context:[],completed:[],deliveries:[{uuid:'native-claude',deliveryId:'delivery-claude',presentation}]})[0]!.message;
  const claude = {...record, backend:'claude' as const, sessionFile:null};
  result = await searchConversation(claude, 'cedar', {...options,liveHistory:{history:[claudeMessage],historyMessageIds:['native-claude']}});
  assert.deepEqual(result.matches.map(m=>m.messageId), ['delivery-claude']);
  result = await searchConversation(claude, 'tool text', {...options,liveHistory:{history:[{role:'toolResult',toolCallId:'tool-id',content:'tool text'}],historyMessageIds:['other-native-id']}});
  assert.deepEqual(result.matches.map(m=>m.messageId), ['tool-id']);
  await rm(`${codexMetadataFile(dir, 'thread')}.display.json`);
  assert.equal((await searchConversation(codex, 'anything', options)).complete, false);
  console.log('conversation search checks passed');
} finally { await rm(dir, { recursive: true, force: true }); }
