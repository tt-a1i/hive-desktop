import assert from 'node:assert/strict';
import {codexNativeVersion,supportsCodexDeferredContinuation} from '../src/agents/codex-native-version.js';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { openCodexHistoryControl, CodexHistoryUncertainError } from '../src/agents/codex-history-control.js';

// Shape captured from the owned official 0.169.0 initialize response; no CLI is run here.
const actualUserAgent='hive_desktop/0.169.0 (Mac OS 26.7.3; arm64) dumb (hive_desktop; 0.1.0)';
assert.equal(codexNativeVersion({userAgent:actualUserAgent}),'0.169.0');
assert.equal(supportsCodexDeferredContinuation({userAgent:actualUserAgent}),true);
for(const userAgent of ['hive_desktop/0.168.0 (hive_desktop; 0.169.0)','hive_desktop/0.169.1','hive_desktop/0.169.00','hive_desktop/0.169.0-beta.1','hive_desktop/0.169.0junk','other-client/0.169.0','codex_cli_rs/0.169.0','prefix hive_desktop/0.169.0'])assert.equal(supportsCodexDeferredContinuation({userAgent}),false,userAgent);
for(const response of [null,{}, {userAgent:null},{userAgent:169}])assert.equal(supportsCodexDeferredContinuation(response),false);

const dir = await mkdtemp(join(tmpdir(), 'hive-history-control-'));
const peer = join(dir, 'peer.cjs'), log = join(dir, 'requests');
await writeFile(peer, `#!/usr/bin/env node
const fs=require('fs'),rl=require('readline');const log=${JSON.stringify(log)};
let child=[],mode=process.env.HISTORY_MODE||'';
const laterStatus=['interrupted','failed','inProgress','unknown'].includes(mode)?(mode==='unknown'?'futureStatus':mode):mode==='interrupted-active-item'?'interrupted':'completed';
const turns=[{id:'t1',status:'completed',itemsView:'full',items:[{id:'i1',type:'userMessage',content:[]}]},{id:'t2',status:laterStatus,itemsView:'full',items:[{id:'i2',type:'agentMessage',text:'ok',...(mode==='interrupted-active-item'?{status:'inProgress'}:{})}]}];
function thread(id){return {id,path:'/native/'+id,cwd:id==='source'?'/source':${JSON.stringify(dir)},historyMode:'paginated',forkedFromId:id==='child'?'source':null};}
rl.createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);fs.appendFileSync(log,JSON.stringify(x)+'\\n');if(!x.id)return;let result={};
if(x.method==='initialize')result={userAgent:'hive_desktop/0.169.0 (Mac OS 26.7.3; arm64) dumb (hive_desktop; 0.1.0)'};
else if(x.method==='thread/read')result={thread:thread(x.params.threadId)};
else if(x.method==='thread/turns/list'){let a=x.params.threadId==='source'?turns:child;let n=x.params.cursor?Number(x.params.cursor):0;result={data:a.slice(n,n+1),nextCursor:n+1<a.length?String(n+1):null};if(mode==='duplicate'&&n===1)result.data=[a[0]];}
else if(x.method==='thread/fork'){if(mode==='gap')return process.exit(1);child=x.params.lastTurnId?turns.slice(0,turns.findIndex(t=>t.id===x.params.lastTurnId)+1):turns.slice();result={thread:thread('child')};}
else if(x.method==='thread/revert'){child=[];result={thread:thread('child')};}
else throw Error(x.method);console.log(JSON.stringify({id:x.id,result}));});`, { mode: 0o700 });
try {
  const control = await openCodexHistoryControl({cwd: dir, command: peer});
  const order: string[] = [];
  assert.equal((await control.read('source')).turns.length, 2);
  const branch = await control.fork({threadId:'source',cwd:dir,lastTurnId:'t1',onNativeRequested:()=>{order.push('requested');},onNativeKnown:id=>{order.push(id);}});
  assert.deepEqual(branch.turns.map(t=>t.id), ['t1']);
  assert.deepEqual(order, ['requested','child']);
  const empty = await control.fork({threadId:'source',cwd:dir,beforeFirstTurnId:'t1',onNativeRequested:()=>{},onNativeKnown:()=>{}});
  assert.equal(empty.turns.length, 0);
  await control.close();
  const entries = (await readFile(log,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
  const fork = entries.find(x=>x.method==='thread/fork');
  assert.equal(fork.params.deferGoalContinuation, true);
  assert.equal(fork.params.lastTurnId, 't1');
  assert.ok(entries.find(x=>x.method==='thread/revert' && x.params.threadId==='child' && x.params.beforeTurnId==='t1'));
  assert.ok(!entries.some(x=>['thread/start','thread/resume','turn/start'].includes(x.method)));
  for (const terminalStatus of ['interrupted','failed']) {
    process.env.HISTORY_MODE=terminalStatus;
    const terminal=await openCodexHistoryControl({cwd:dir,command:peer});
    try {
      assert.deepEqual((await terminal.read('source')).turns.map(t=>t.status),['completed',terminalStatus]);
      const earlier=await terminal.fork({threadId:'source',cwd:dir,lastTurnId:'t1',onNativeRequested:()=>{},onNativeKnown:()=>{}});
      assert.deepEqual(earlier.turns.map(t=>t.id),['t1'],'A later terminal turn must not block the earlier completed boundary');
      const throughTerminal=await terminal.fork({threadId:'source',cwd:dir,lastTurnId:'t2',onNativeRequested:()=>{},onNativeKnown:()=>{}});
      assert.deepEqual(throughTerminal.turns.map(t=>t.status),['completed',terminalStatus]);
    } finally {await terminal.close();}
  }
  for (const mode of ['inProgress','unknown','interrupted-active-item']) {
    process.env.HISTORY_MODE=mode;let nativeRequested=false;
    const unsafe=await openCodexHistoryControl({cwd:dir,command:peer});
    try {
      await assert.rejects(unsafe.fork({threadId:'source',cwd:dir,lastTurnId:'t1',onNativeRequested:()=>{nativeRequested=true;},onNativeKnown:()=>{}}),/Incomplete or duplicate/);
      assert.equal(nativeRequested,false,'Reject nonterminal/unknown history and active items before any native mutation');
    } finally {await unsafe.close();}
  }
  process.env.HISTORY_MODE='duplicate';
  const duplicate = await openCodexHistoryControl({cwd:dir,command:peer});
  await assert.rejects(duplicate.read('source'), /duplicate/); await duplicate.close();
  process.env.HISTORY_MODE='gap';
  const gap = await openCodexHistoryControl({cwd:dir,command:peer});
  await assert.rejects(gap.fork({threadId:'source',cwd:dir,lastTurnId:'t1',onNativeRequested:()=>{},onNativeKnown:()=>{}}), CodexHistoryUncertainError);
  await gap.close();
  console.log('Codex history control JSONL checks passed');
} finally { delete process.env.HISTORY_MODE; await rm(dir,{recursive:true,force:true}); }
