import { strict as assert } from 'node:assert';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { searchConversation } from '../src/conversation-search.js';
import { CodexProjection } from '../src/agents/codex-projection.js';
import { codexMetadataFile } from '../src/agents/codex-metadata.js';
import { projectClaudeMessage } from '../src/agents/claude-history.js';
import type { ChatRecord } from '../src/chat-index.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-search-tools-'));
try {
  const opts = { codexDirectory:dir, claudeDirectory:dir };
  const base: ChatRecord = {id:'s',workspaceId:'w',backend:'codex',codexThreadId:'codex',title:'',sessionFile:null,updatedAt:1};
  const projection = new CodexProjection(dir, 'codex');
  projection.put({id:'cmd',type:'commandExecution',status:'completed',command:'rg 旧文 docs/路径.md',aggregatedOutput:'shown output'},'t');
  projection.complete('t');
  projection.recover([{id:'t',status:'completed',items:[{id:'cmd',type:'commandExecution',status:'completed'}]}],true);
  for (let i=0;i<2;i++) {
    const match = await searchConversation(base,'旧文 路径.md',opts);
    assert.equal(match.complete,true);
    assert.deepEqual(match.matches.map(x=>x.messageId),['cmd']);
    assert.deepEqual((await searchConversation(base,'commandExecution',opts)).matches.map(x=>x.messageId),['cmd']);
    assert.deepEqual((await searchConversation(base,'shown output',opts)).matches.map(x=>x.messageId),['cmd']);
  }
  const raw = {id:'unknown-tool',type:'newTool', authMetadata:'must-not-index-secret'};
  projection.recover([{id:'t',status:'completed',items:[raw]}],true);
  assert.equal((await searchConversation(base,'must-not-index-secret',opts)).matches.length,0);
  assert.deepEqual((await searchConversation(base,'newTool',opts)).matches.map(x=>x.messageId),['unknown-tool']);

  // Old snapshots do not include merged cached inputs and cannot claim completeness.
  await writeFile(`${codexMetadataFile(dir,'codex')}.display.json`,JSON.stringify({version:1,entries:[{turnId:'t',item:raw}],completedTurns:['t'],completeHistory:true,searchEntries:[{turnId:'t',item:raw}]}));
  assert.equal((await searchConversation(base,'newTool',opts)).complete,false);

  const pi = {...base,backend:'pi' as const,codexThreadId:undefined};
  const assistant = {role:'assistant',content:[{type:'toolCall',id:'pi-call',name:'read_file',arguments:{path:'文档/重要.md', nested:{needle:'独角兽'}}}]};
  const result = {role:'toolResult',toolCallId:'pi-call',toolName:'read_file',content:[{type:'text',text:'read complete'}],details:{patch:'-before\n+特殊补丁'},source:{backend:'pi'}};
  const live = {history:[assistant,result],historyMessageIds:['assistant','native-result']};
  for (const query of ['文档/重要.md','独角兽','特殊补丁','read complete','read_file']) {
    assert.deepEqual((await searchConversation(pi,query,{...opts,liveHistory:live})).matches.map(x=>x.messageId),['pi-call'],query);
  }
  const piFile = join(dir, 'pi.jsonl');
  await writeFile(piFile, [
    {type:'session',id:'session',version:3,timestamp:'2026-01-01T00:00:00Z',cwd:dir},
    {type:'message',id:'a',parentId:null,timestamp:'2026-01-01T00:00:00Z',message:assistant},
    {type:'message',id:'r',parentId:'a',timestamp:'2026-01-01T00:00:00Z',message:result},
  ].map(row=>JSON.stringify(row)).join('\n'));
  for (let i=0;i<2;i++) for (const query of ['文档/重要.md','特殊补丁','read complete']) {
    const found = await searchConversation({...pi,sessionFile:piFile},query,opts);
    assert.equal(found.complete,true);
    assert.deepEqual(found.matches.map(x=>x.messageId),['pi-call'], `cold/cache ${query}`);
  }
  const activeCall = {role:'assistant',content:[{type:'toolCall',id:'active-call',name:'bash',arguments:{command:'sleep 30; echo active-command-marker'}}]};
  const activeSearch = (history: unknown[], ids:string[], liveBusy:boolean) => searchConversation(pi, 'active-command-marker', {...opts, liveBusy, liveHistory:{history,historyMessageIds:ids}});
  let active = await activeSearch([activeCall], ['assistant-active'], true);
  assert.deepEqual(active.matches.map(x=>x.messageId), ['active-call']);
  assert.equal(active.complete,false);
  assert.match(active.warning!,/unfinished tool output/);
  active = await activeSearch([activeCall], ['assistant-active'], false);
  assert.equal(active.complete,false, 'unresolved call remains incomplete after busy ends');
  const completedTool = {role:'toolResult',toolCallId:'active-call',toolName:'bash',content:'done'};
  active = await activeSearch([activeCall,completedTool], ['assistant-active','native-result'], false);
  assert.deepEqual(active.matches.map(x=>x.messageId), ['active-call'], 'completed call has exactly one result');
  assert.equal(active.complete,true);
  active = await activeSearch([activeCall,completedTool], ['assistant-active','native-result'], true);
  assert.equal(active.complete,false, 'busy providers can have unrecorded partial output');
  await writeFile(piFile, [
    {type:'session',id:'session',version:3,timestamp:'2026-01-01T00:00:00Z',cwd:dir},
    {type:'message',id:'a',parentId:null,timestamp:'2026-01-01T00:00:00Z',message:activeCall},
  ].map(row=>JSON.stringify(row)).join('\n'));
  for (let i=0;i<2;i++) {
    active = await searchConversation({...pi,sessionFile:piFile},'active-command-marker',opts);
    assert.deepEqual(active.matches.map(x=>x.messageId),['active-call']);
    assert.equal(active.complete,false, 'cold and cached unresolved call is incomplete');
  }
  await writeFile(piFile, [
    {type:'session',id:'session',version:3,timestamp:'2026-01-01T00:00:00Z',cwd:dir},
    {type:'message',id:'a',parentId:null,timestamp:'2026-01-01T00:00:00Z',message:activeCall},
    {type:'message',id:'r',parentId:'a',timestamp:'2026-01-01T00:00:00Z',message:completedTool},
  ].map(row=>JSON.stringify(row)).join('\n'));
  active = await searchConversation({...pi,sessionFile:piFile},'active-command-marker',opts);
  assert.deepEqual(active.matches.map(x=>x.messageId),['active-call']);
  assert.equal(active.complete,true, 'completion and disk update restore completeness');
  const claudeBase = {...base,backend:'claude' as const,codexThreadId:undefined};
  const busyClaude = await searchConversation(claudeBase,'active-command-marker',{...opts,liveBusy:true,liveHistory:{history:[activeCall],historyMessageIds:['assistant-active']}});
  assert.deepEqual(busyClaude.matches.map(x=>x.messageId),['active-call']);
  assert.equal(busyClaude.complete,false);
  const meta = {version:1 as const,context:[],deliveries:[],completed:[]};
  const claudeCall = projectClaudeMessage({type:'assistant',uuid:'assistant',session_id:'s',parent_tool_use_id:null,message:{id:'a',role:'assistant',content:[{type:'tool_use',id:'claude-call',name:'Shell',input:{command:'cat 资料/读我.md'}}]}},meta)[0]!.message;
  const claudeResult = projectClaudeMessage({type:'user',uuid:'user',session_id:'s',parent_tool_use_id:null,message:{role:'user',content:[{type:'tool_result',tool_use_id:'claude-call',content:'ok'}]}},meta)[0]!.message;
  const claudeLive={history:[claudeCall,claudeResult],historyMessageIds:['assistant','claude-call']};
  for (const query of ['Shell','资料/读我.md','ok']) assert.deepEqual((await searchConversation(claudeBase,query,{...opts,liveHistory:claudeLive})).matches.map(x=>x.messageId),['claude-call']);
  const huge = {role:'toolResult',toolCallId:'large',toolName:'Tool',toolInput:{command:'x'.repeat(33_000)+'tail-marker'},content:'done'};
  let limited = await searchConversation(pi,'done',{...opts,liveHistory:{history:[huge],historyMessageIds:['large']}});
  assert.equal(limited.complete,false);
  assert.match(limited.warning!,/search limit/);
  limited = await searchConversation(pi,'tail-marker',{...opts,liveHistory:{history:[huge],historyMessageIds:['large']}});
  assert.equal(limited.complete,false);
  assert.equal(limited.matches.length,0);
  const firstOnly = await searchConversation(pi, 'done', {
    ...opts, firstMatchOnly: true, liveHistory: {
      history: [{role:'assistant',content:'first match done'}, huge], historyMessageIds: ['first', 'large'],
    },
  });
  assert.deepEqual(firstOnly.matches.map(match => match.messageId), ['first']);
  assert.match(firstOnly.warning!, /search limit/, 'the result list still warns about later incomplete history');
  const deep:any={}; let node=deep; for(let i=0;i<12;i++) node=node.child={}; node.hidden='too deep';
  assert.equal((await searchConversation(pi,'done',{...opts,liveHistory:{history:[{...huge,toolInput:deep}],historyMessageIds:['large']}})).complete,false);
  console.log('conversation search tool checks passed');
} finally { await rm(dir,{recursive:true,force:true}); }
