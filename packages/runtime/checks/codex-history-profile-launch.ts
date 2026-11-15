/** Controlled profile launch through history control; no vendor CLI, account or model. */
import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {openCodexHistoryControl} from '../src/agents/codex-history-control.js';
const directory=await mkdtemp(join(tmpdir(),'hive-history-profile-'));
const before=process.env.HIVE_HISTORY_PROFILE_MARKER, homeBefore=process.env.CODEX_HOME;
try{
 const peer=join(directory,'profile-peer.cjs');
 await writeFile(peer,`const fs=require('fs'),rl=require('readline');
if(process.argv[2]!=='--profile-history-fixture'||process.argv[3]!=='app-server'||process.env.HIVE_HISTORY_PROFILE_MARKER!=='isolated-profile'||process.env.CODEX_HOME!==${JSON.stringify(join(directory,'profile-home'))})throw Error('Profile launch not forwarded');
let child;const turn={id:'turn',status:'completed',itemsView:'full',items:[{id:'user',type:'userMessage',content:[]}]};
rl.createInterface({input:process.stdin}).on('line',line=>{const q=JSON.parse(line);if(!q.id)return;let result;
if(q.method==='initialize')result={userAgent:'hive_desktop/0.169.0 (Mac OS 26.7.3; arm64) dumb (hive_desktop; 0.1.0)'};
else if(q.method==='thread/read')result={thread:{id:q.params.threadId,cwd:${JSON.stringify(directory)},path:'/native/'+q.params.threadId,historyMode:'paginated',...(q.params.threadId==='child'?{forkedFromId:'source'}:{})}};
else if(q.method==='thread/turns/list')result={data:[turn],nextCursor:null};
else if(q.method==='thread/fork'){if(q.params.deferGoalContinuation!==true)throw Error('Missing deferred continuation');result={thread:{id:'child'}};}
else throw Error('Unexpected execution '+q.method);
fs.appendFileSync(${JSON.stringify(join(directory,'methods'))},q.method+'\\n');console.log(JSON.stringify({id:q.id,result}));});`);
 const control=await openCodexHistoryControl({cwd:directory,command:process.execPath,args:[peer,'--profile-history-fixture'],env:{HIVE_HISTORY_PROFILE_MARKER:'isolated-profile',CODEX_HOME:join(directory,'profile-home')}});
 try{assert.equal((await control.read('source')).turns.length,1);const fork=await control.fork({threadId:'source',cwd:directory,lastTurnId:'turn',onNativeRequested:()=>{},onNativeKnown:()=>{}});assert.equal(fork.thread.id,'child');}finally{await control.close();}
 const methods=(await readFile(join(directory,'methods'),'utf8')).trim().split('\n');assert.equal(methods.filter(method=>method==='thread/fork').length,1);assert.ok(!methods.some(method=>['thread/start','thread/resume','turn/start'].includes(method)));
 assert.equal(process.env.HIVE_HISTORY_PROFILE_MARKER,before);assert.equal(process.env.CODEX_HOME,homeBefore);
 console.log('CODEX_HISTORY_PROFILE_LAUNCH_OK: argv/env honored without changing host environment or starting a turn');
}finally{await rm(directory,{recursive:true,force:true,maxRetries:5});}
