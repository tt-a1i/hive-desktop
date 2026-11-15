/** Production stdio dispatch and a controlled native peer; no model. */
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {once} from 'node:events';
import {mkdir,mkdtemp,readFile,realpath,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import {ChatIndex} from '../src/chat-index.js';
const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-history-wire-'))),repo=join(dir,'repo'),data=join(dir,'state');
await mkdir(repo);await mkdir(join(dir,'pi'));await writeFile(join(dir,'gitconfig'),'');
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid',...args],{cwd:repo,stdio:'pipe'});git('init','--template=');await writeFile(join(repo,'a'),'A');git('add','.');git('commit','-qm','base');
const peer=join(dirname(fileURLToPath(import.meta.url)),'fixtures/session-history-peer.cjs'),log=join(dir,'requests');
const runtime=process.env.HIVE_DESKTOP_TEST_RUNTIME ?? 'dist/main.js';
const guard='const original=globalThis.fetch;globalThis.fetch=(input,init)=>{const url=new URL(typeof input==="string"?input:input instanceof URL?input:input.url);if(!["127.0.0.1","localhost","::1"].includes(url.hostname))return Promise.reject(new Error("Fixture disallows external fetch"));return original(input,init);};';
function start(){const child=spawn(process.execPath,['--import',`data:text/javascript,${encodeURIComponent(guard)}`,runtime],{env:{...process.env,HIVE_DESKTOP_DATA_DIR:data,PI_CODING_AGENT_DIR:join(dir,'pi'),PI_OFFLINE:'1',GIT_CONFIG_GLOBAL:join(dir,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1',HIVE_CODEX_COMMAND:peer,HIVE_HISTORY_STATE:join(dir,'peer.json'),HIVE_HISTORY_LOG:log,HIVE_HISTORY_SOURCE:repo,HIVE_HISTORY_DROP:join(dir,'drop'),HIVE_HISTORY_DROP_REVERT:join(dir,'drop-revert'),HIVE_HISTORY_DROP_BEFORE_REVERT:join(dir,'drop-before')},stdio:['pipe','pipe','pipe']});const exited=once(child,'exit'),lines=createInterface({input:child.stdout});let id=0,stderr='';child.stderr.on('data',x=>stderr+=String(x));const pending=new Map<number,(x:any)=>void>();lines.on('line',line=>{const x=JSON.parse(line);pending.get(x.id)?.(x);pending.delete(x.id);});const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<any>((resolve,reject)=>{const n=++id,t=setTimeout(()=>reject(Error('timeout '+method+' '+stderr)),30000);pending.set(n,x=>{clearTimeout(t);resolve(x);});child.stdin.write(JSON.stringify({id:n,method,params})+'\n');});return {rpc,close:async()=>{if(child.exitCode===null){await rpc('shutdown');await exited;}lines.close();assert.equal(child.exitCode,0,stderr);}};}
let wire=start();
try{
 await wire.rpc('initialize');const ws=(await wire.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:repo,name:'wire',autostart_orchestrator:false}})).result;await wire.close();
 new ChatIndex(join(data,'chats')).save({id:'source-session',workspaceId:ws.id,title:'source',backend:'codex',sessionFile:null,codexThreadId:'source',originAgentId:`${ws.id}:orchestrator`,identityState:'known',updatedAt:Date.now()});
 wire=start();await wire.rpc('initialize');const base={workspaceId:ws.id,sessionId:'source-session'};assert.equal((await wire.rpc('history.capabilities',base)).result.fork,true);const cp=(await wire.rpc('checkpoint.preview',base)).result;assert.equal(cp.canCapture,true);
 const selection={...base,boundary:{nativeUserItemId:'u1'},code:{kind:'current'}};assert.ok((await wire.rpc('history.fork.preview',{...selection,cwd:repo})).error);assert.ok((await wire.rpc('history.fork.preview',{...selection,nativeThreadId:'source'})).error);
 const preview=(await wire.rpc('history.fork.preview',selection)).result;assert.ok(preview?.previewToken);const params={...base,previewToken:preview.previewToken,requestId:'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'};assert.ok((await wire.rpc('history.fork.apply',{...params,cwd:repo})).error);const applied=(await wire.rpc('history.fork.apply',params)).result;assert.equal(applied.state,'ready',JSON.stringify(applied));assert.ok((await wire.rpc('chat.open',{sessionId:applied.session.id})).result);assert.ok((await wire.rpc('history.operations.list',{workspaceId:ws.id})).result.operations.some((x:any)=>x.operationId===applied.operationId));
 await wire.close();wire=start();await wire.rpc('initialize');assert.equal((await wire.rpc('history.fork.apply',params)).result.session.id,applied.session.id);assert.equal((await wire.rpc('history.operations.get',{workspaceId:ws.id,operationId:applied.operationId})).result.state,'ready');const calls=(await readFile(log,'utf8')).trim().split('\n').map(x=>JSON.parse(x));assert.equal(calls.filter(x=>x.method==='thread/fork').length,1);assert.ok(!calls.some(x=>['thread/start','thread/resume','turn/start'].includes(x.method)));console.log('SESSION_HISTORY_WIRE_OK');
}finally{await wire.close();await rm(dir,{recursive:true,force:true});}
