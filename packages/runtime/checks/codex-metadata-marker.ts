/** A synthetic Codex peer confirms a marker write failure does not replay a turn. */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {syncBuiltinESMExports} from 'node:module';
import {mkdtemp, readFile, rm, stat, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createCodexSession} from '../src/agents/codex.js';
import {codexMetadataFile, readCodexMetadata} from '../src/agents/codex-metadata.js';

const dir=await mkdtemp(join(tmpdir(),'hive-codex-marker-'));
await writeFile(join(dir,'package.json'),'{}');
await writeFile(join(dir,'app-server'),String.raw`
const {createInterface}=require('node:readline');const fs=require('node:fs');
const thread={id:'fixture-thread',path:null,historyMode:'legacy',turns:[]};
const send=value=>process.stdout.write(JSON.stringify(value)+'\n');
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
(async()=>{for await(const line of createInterface({input:process.stdin})){
 const q=JSON.parse(line);if(!q.method)continue;const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  fs.appendFileSync('calls','turn/start\n');
  const turn={id:'turn',status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});
  turn.status='completed';notify('turn/completed',{turn});
 }
}})().catch(error=>{console.error(error);process.exit(1)});
`);
const originalWrite=fs.writeFileSync, originalError=console.error;
let session:Awaited<ReturnType<typeof createCodexSession>>|undefined;
let injected=0;
try {
 session=await createCodexSession({cwd:dir,sessionDir:dir,command:process.execPath,model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(){}});
 const marker=codexMetadataFile(dir,'fixture-thread')+'.required';
 fs.writeFileSync=((path:unknown,...args:unknown[])=>{
   if(path===marker&&injected++===0){const error=new Error('injected marker failure') as NodeJS.ErrnoException;error.code='ENOSPC';throw error;}
   return (originalWrite as (...args:any[])=>void)(path,...args);
 }) as typeof fs.writeFileSync;
 syncBuiltinESMExports();console.error=()=>{};
 await session.prompt('one delivery',{deliveryId:'delivery-id',presentation:{version:1,origin:'user',displayText:'one delivery'}});
 assert.ok(injected>=1);
 assert.deepEqual(session.completedDeliveryIds(),['delivery-id']);
 assert.deepEqual(readCodexMetadata(dir,'fixture-thread').completed,['delivery-id']);
 assert.ok((await stat(marker)).isFile(),'later metadata write retries the guard');
 assert.equal(await readFile(join(dir,'calls'),'utf8'),'turn/start\n');
 console.log('CODEX_MARKER_FAILURE_NO_REPLAY_OK');
} finally {
 fs.writeFileSync=originalWrite;syncBuiltinESMExports();console.error=originalError;
 await session?.dispose();await rm(dir,{recursive:true,force:true});
}
