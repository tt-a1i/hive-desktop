/** Real JSONL process, synthetic two-page native history. No vendor/model request. */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createCodexSession } from '../src/agents/codex.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-codex-pages-'));
await writeFile(join(directory, 'app-server'), `
const {createInterface}=require('node:readline');
(async()=>{
let mode='full';
const turns=[1,2].map(n=>({id:'turn-'+n,status:'completed',itemsView:'full',items:[{id:'native-'+n,type:'agentMessage',text:'message '+n}]}));
for await(const line of createInterface({input:process.stdin})) {
 const q=JSON.parse(line);if(!q.method||q.id===undefined)continue;
 let result;
 if(q.method==='initialize')result={};
 else if(q.method==='model/list')result={data:[],nextCursor:null};
 else if(q.method==='thread/resume'){mode=q.params.threadId;result={thread:{id:mode,path:null,historyMode:'paginated',turns:[]},model:'gpt-6.1-terra',modelProvider:'openai'};}
 else if(q.method==='thread/turns/list'){
  if(q.params.itemsView!=='full'||q.params.sortDirection!=='asc')throw Error('Wrong history request');
  const n=q.params.cursor===null?0:q.params.cursor==='second'?1:-1;if(n<0)throw Error('Wrong cursor');
  result={data:[{...turns[n],itemsView:mode==='partial'?'summary':'full'}],nextCursor:n===0?'second':null};
 }else throw Error('Unexpected request: '+q.method);
 if(q.id!==undefined)process.stdout.write(JSON.stringify({id:q.id,result})+'\\n');
}
})().catch(error=>{console.error(error);process.exit(1)});
`, {mode:0o600});
const options = {cwd:directory,sessionDir:directory,command:process.execPath,threadId:'full',onEvent(){}};
let snapshot: unknown;
for(let n=0;n<2;n++) {
 const session=await createCodexSession(options);
 try {
  const current=session.snapshot();
  assert.equal(current.history.length,2);
  assert.deepEqual(current.history.map((entry:any)=>entry.content),[[{type:'text',text:'message 1'}],[{type:'text',text:'message 2'}]]);
  assert.deepEqual(current.history.map((entry:any)=>entry.rawBackendItem.id),['native-1','native-2']);
  if(n===0)snapshot=current;else assert.deepEqual(current,snapshot);
 }finally{await session.dispose();}
}
await assert.rejects(createCodexSession({...options,threadId:'partial'}));
console.log('Codex paginated history: two full pages in native order, native item IDs, restart parity; partial summary refused. Synthetic peer.');
