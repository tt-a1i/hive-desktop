/** Production runtime, Hive HTTP/SQLite, Git and a controlled JSONL peer; no model. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { writeCodexMetadata } from '../src/agents/codex-metadata.js';
import { ChatIndex } from '../src/chat-index.js';

const dir=await realpath(await mkdtemp(join(tmpdir(),'hive-history-runtime-'))), repo=join(dir,'repo'), data=join(dir,'state');
const peer=resolve('checks/fixtures/session-history-peer.cjs');
const envKeys=['GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM','PI_CODING_AGENT_DIR','PI_OFFLINE','HIVE_HISTORY_STATE','HIVE_HISTORY_LOG','HIVE_HISTORY_SOURCE','HIVE_HISTORY_DROP','HIVE_HISTORY_DROP_REVERT'] as const;
const old=Object.fromEntries(envKeys.map(k=>[k,process.env[k]]));
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:repo,encoding:'utf8'}).trim();
const requests=async()=> (await readFile(join(dir,'requests'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
try{
 await mkdir(repo);await mkdir(join(dir,'pi'));await writeFile(join(dir,'gitconfig'),'');
 Object.assign(process.env,{GIT_CONFIG_GLOBAL:join(dir,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1',PI_CODING_AGENT_DIR:join(dir,'pi'),PI_OFFLINE:'1',HIVE_HISTORY_STATE:join(dir,'peer.json'),HIVE_HISTORY_LOG:join(dir,'requests'),HIVE_HISTORY_SOURCE:repo,HIVE_HISTORY_DROP:join(dir,'drop'),HIVE_HISTORY_DROP_REVERT:join(dir,'drop-revert')});
 git('init','--template=');await writeFile(join(repo,'.gitignore'),'.secret\n');await writeFile(join(repo,'file.txt'),'A');git('add','.');git('commit','-m','base');
 await writeFile(join(repo,'file.txt'),'B');git('add','file.txt');await writeFile(join(repo,'file.txt'),'C');await writeFile(join(repo,'.secret'),'private');
 const open=()=>createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:peer}});
 runtime=await open();const project=await runtime.hive.request('POST','/api/workspaces',{path:repo,name:'fixture',autostart_orchestrator:false}) as {id:string};await runtime.close();runtime=undefined;
 new ChatIndex(join(data,'chats')).save({id:'session-source',workspaceId:project.id,title:'source',backend:'codex',sessionFile:null,codexThreadId:'source',originAgentId:`${project.id}:orchestrator`,identityState:'known',updatedAt:Date.now()});
 runtime=await open();
 const attachmentFile=join(dir,'attachment.txt');await writeFile(attachmentFile,'attachment');const attachment=(await runtime.chats.attachments.add(project.id,attachmentFile)).attachment;
 const missing={id:'missing-attachment',workspaceId:project.id,name:'missing.txt',mimeType:'application/octet-stream',size:1};
 writeCodexMetadata(join(data,'chats','codex'),'source',{version:1,context:[],completed:[],deliveries:[{turnId:'t2',itemId:'u2',deliveryId:'delivery-2',presentation:{version:1,origin:'user',displayText:'second original',attachments:[attachment,missing]}}]});
 const scripts=await runtime.projectScripts.list(project.id);await runtime.projectScripts.save(project.id,scripts.revision,{name:'Setup',command:'printf ran > setup-marker',runOnWorktreeCreate:true});
 const ledger=runtime.hive.executionLedgerRevision();
 async function apply(item:string,edit:boolean,requestId:string){const preview=await runtime!.chats.history!.preview({workspaceId:project.id,sessionId:'session-source',boundary:{nativeUserItemId:item},code:{kind:'current'},edit});const result=await runtime!.chats.history!.apply(project.id,'session-source',preview.previewToken as string,requestId) as any;return {preview,result};}
 const one=await apply('u1',false,'11111111-1111-4111-8111-111111111111');assert.equal(one.result.state,'ready',JSON.stringify(one.result));
 const child=one.result.session;assert.deepEqual(JSON.parse(await readFile(join(dir,'peer.json'),'utf8')).child1.turns.map((t:any)=>t.id),['t1']);assert.equal(await readFile(join(child.executionPath,'file.txt'),'utf8'),'C');assert.equal(execFileSync('git',['-C',child.executionPath,'show',':file.txt'],{encoding:'utf8'}),'B');
 assert.equal(execFileSync('git',['-C',child.executionPath,'show','HEAD:file.txt'],{encoding:'utf8'}),'A');await assert.rejects(readFile(join(child.executionPath,'.secret')));assert.equal(await readFile(join(repo,'.secret'),'utf8'),'private');
 assert.equal(runtime.hive.executionLedgerRevision(),ledger);
 const beforeOpen=(await requests()).length;await runtime.chats.open(child.id);const openRequests=(await requests()).slice(beforeOpen);assert.ok(!openRequests.some(x=>['thread/start','thread/resume','turn/start'].includes(x.method)));await assert.rejects(readFile(join(child.executionPath,'setup-marker')));
 const two=await apply('u2',true,'22222222-2222-4222-8222-222222222222');assert.equal(two.result.state,'ready',JSON.stringify(two.result));assert.deepEqual(JSON.parse(await readFile(join(dir,'peer.json'),'utf8')).child2.turns.map((t:any)=>t.id),['t1']);assert.deepEqual(two.result.draft,{text:'second original',attachmentIds:[attachment.id,missing.id],missingAttachments:[missing.id]});
 const first=await apply('u1',true,'33333333-3333-4333-8333-333333333333');assert.equal(first.result.state,'ready',JSON.stringify(first.result));assert.deepEqual(JSON.parse(await readFile(join(dir,'peer.json'),'utf8')).child3.turns,[]);assert.deepEqual(first.result.draft,{text:'first',attachmentIds:[],missingAttachments:[]});
 const logBefore=await requests();assert.equal(logBefore.filter(x=>x.method==='thread/fork').length,3);assert.equal(logBefore.filter(x=>x.method==='thread/revert').length,1);assert.ok(!logBefore.some(x=>['thread/start','thread/resume','turn/start'].includes(x.method)));
 await runtime.close();runtime=await open();const replay=await runtime.chats.history!.apply(project.id,'session-source',one.preview.previewToken as string,'11111111-1111-4111-8111-111111111111') as any;assert.equal(replay.session.id,child.id);assert.equal((await requests()).filter(x=>x.method==='thread/fork').length,3);
 const index=(runtime.chats as any).index;const originalSave=index.save.bind(index);let injected=false;index.save=(record:any)=>{if(record.lineage&&!injected){injected=true;throw Error('injected index write failure');}return originalSave(record);};
 const failed=await apply('u1',false,'55555555-5555-4555-8555-555555555555');assert.equal(failed.result.state,'uncertain',JSON.stringify(failed.result));assert.ok(injected);const countBeforeRecover=(await requests()).filter(x=>x.method==='thread/fork').length;
 await runtime.close();runtime=await open();const recovered=await runtime.chats.history!.recover(project.id,failed.result.operationId) as any;assert.equal(recovered.state,'ready',JSON.stringify(recovered));assert.equal((await requests()).filter(x=>x.method==='thread/fork').length,countBeforeRecover);
 await writeFile(join(dir,'drop-revert'),'1');const revertGap=await apply('u1',true,'66666666-6666-4666-8666-666666666666');assert.equal(revertGap.result.state,'uncertain',JSON.stringify(revertGap.result));const requestsBeforeRecover=await requests();await runtime.close();runtime=await open();const revertRecovered=await runtime.chats.history!.recover(project.id,revertGap.result.operationId) as any;assert.equal(revertRecovered.state,'ready',JSON.stringify(revertRecovered));const requestsAfterRecover=await requests();assert.equal(requestsAfterRecover.filter(x=>x.method==='thread/revert').length,requestsBeforeRecover.filter(x=>x.method==='thread/revert').length);assert.equal(requestsAfterRecover.filter(x=>x.method==='thread/fork').length,requestsBeforeRecover.filter(x=>x.method==='thread/fork').length);
 await writeFile(join(dir,'drop'),'1');const gap=await apply('u1',false,'44444444-4444-4444-8444-444444444444');assert.equal(gap.result.state,'uncertain',JSON.stringify(gap.result));const forks=(await requests()).filter(x=>x.method==='thread/fork').length;await runtime.close();runtime=await open();const gapReplay=await runtime.chats.history!.apply(project.id,'session-source',gap.preview.previewToken as string,'44444444-4444-4444-8444-444444444444') as any;assert.equal(gapReplay.state,'uncertain');assert.equal((await requests()).filter(x=>x.method==='thread/fork').length,forks);
 assert.equal(runtime.hive.executionLedgerRevision(),ledger);console.log('SESSION_HISTORY_OPERATIONS_OK');
}finally{if(runtime)await runtime.close();for(const [key,value] of Object.entries(old)){if(value===undefined)delete process.env[key];else process.env[key]=value;}await rm(dir,{recursive:true,force:true,maxRetries:5});}
