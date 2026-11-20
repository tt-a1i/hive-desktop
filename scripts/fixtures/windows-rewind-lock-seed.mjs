// Real packaged runtime/Git with the existing offline native-history peer.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const [root,resources,mode='seed']=process.argv.slice(2);
const data=join(root,'state'),project=join(root,'project'),info=join(root,'rewind-lock-seed.json');
const native=join(root,'native.json'),log=join(root,'requests.jsonl'),draft='Independent rewind draft 50A1';
Object.assign(process.env,{HIVE_DESKTOP_DATA_DIR:data,PI_CODING_AGENT_DIR:join(root,'offline-pi'),PI_OFFLINE:'1',
  GIT_CONFIG_GLOBAL:join(root,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1',HIVE_HISTORY_STATE:native,HIVE_HISTORY_LOG:log,
  HIVE_HISTORY_DROP:join(root,'never-drop'),HIVE_HISTORY_DROP_REVERT:join(root,'never-revert'),HIVE_HISTORY_DROP_BEFORE_REVERT:join(root,'never-before')});
const dist=join(resources,'runtime/dist');
const load=path=>import(pathToFileURL(join(dist,path)).href);
const {createDesktopRuntime}=await load('desktop-runtime.js');
const {ChatIndex}=await load('chat-index.js');
const {writeCodexMetadata,codexMetadataFile}=await load('agents/codex-metadata.js');
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(cwd,...args)=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd,encoding:'utf8',env:{...process.env,GIT_OPTIONAL_LOCKS:'0'}}).trim();
const calls=async()=> (await readFile(log,'utf8').catch(error=>{if(error.code==='ENOENT')return '';throw error;})).split('\n').filter(Boolean).map(JSON.parse);
async function lockIdentity(path) {const s=await stat(path,{bigint:true});return {sha256:digest(await readFile(path)),dev:String(s.dev),ino:String(s.ino),mode:String(s.mode),mtimeNs:String(s.mtimeNs),size:String(s.size)};}
async function snapshot(f) {return {head:git(f.execution,'rev-parse','HEAD'),rootHead:git(project,'rev-parse','HEAD'),index:digest(await readFile(join(f.gitDir,'index'))),tracked:digest(await readFile(join(f.execution,'tracked.txt'))),ignored:digest(await readFile(join(f.execution,'ignored.txt'))),untracked:digest(await readFile(join(f.execution,'untracked.txt'))),lock:await lockIdentity(f.indexLock)};}
const identity=r=>({id:r.id,workspaceId:r.workspaceId,backend:r.backend,profile:r.profile,codexThreadId:r.codexThreadId,originAgentId:r.originAgentId,identityState:r.identityState,execution:r.execution});
let runtime;
try {
 if(mode==='seed') {
  await mkdir(project,{recursive:true});await mkdir(data,{recursive:true});await writeFile(join(root,'gitconfig'),'');
  git(project,'init','--template=');await writeFile(join(project,'tracked.txt'),'checkpoint base\n');await writeFile(join(project,'.gitignore'),'ignored.txt\n');git(project,'add','.');git(project,'commit','-m','Private rewind fixture');
  runtime=await createDesktopRuntime(data,()=>{});
  const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Rewind lock QA',autostart_orchestrator:false});
  const {storeVersion}=await runtime.profiles.list();
  const {profile}=await runtime.profiles.create({driver:'codex',label:'Offline history peer',launch:{command:process.execPath,args:[fileURLToPath(new URL('./windows-rewind-lock-peer.cjs',import.meta.url))]}},storeVersion);
  let chat=await runtime.chats.createRecord(ws.id,'codex',{profile:profile.ref,isolation:'worktree'});
  chat=runtime.chats.rename(chat.id,'Rewind lock source');
  const execution=chat.execution.path;process.env.HIVE_HISTORY_SOURCE=execution;
  runtime.hive.saveNavigation(JSON.stringify({workspace:ws.id,sessions:{[ws.id]:chat.id}}));
  await runtime.close();runtime=undefined;
  const index=new ChatIndex(join(data,'chats'));index.save({...index.get(chat.id),codexThreadId:'source',originAgentId:`${ws.id}:orchestrator`,identityState:'known'});
  writeCodexMetadata(join(data,'chats/codex'),'source',{version:1,context:[],deliveries:[],completed:['fixture-delivery-retained']});
  runtime=await createDesktopRuntime(data,()=>{});
  assert.equal((await runtime.chats.history.capabilities(ws.id,chat.id)).rewindCode,true);
  const preview=await runtime.chats.history.checkpointPreview(ws.id,chat.id);
  const {checkpointId}=await runtime.chats.history.checkpointCapture(ws.id,chat.id,preview.version,randomUUID());
  await writeFile(join(execution,'tracked.txt'),'future staged\n');git(execution,'add','tracked.txt');await writeFile(join(execution,'tracked.txt'),'future disk\n');
  await writeFile(join(execution,'ignored.txt'),'ignored private bytes\n');await writeFile(join(execution,'untracked.txt'),'untracked private bytes\n');
  const opened=await runtime.chats.open(chat.id);assert.equal(opened.history.length,2);
  const baselineHistory=opened.history;
  const gitDir=git(execution,'rev-parse','--absolute-git-dir'),indexLock=join(gitDir,'index.lock');
  await writeFile(indexLock,'UNKNOWN_GIT_OWNER_50B2\n',{flag:'wx'});
  const f={workspaceId:ws.id,sessionId:chat.id,project,execution,gitDir,indexLock,checkpointId,draft,
    identity:identity(new ChatIndex(join(data,'chats')).get(chat.id)),baselineHistory,
    metadataSha256:digest(await readFile(codexMetadataFile(join(data,'chats/codex'),'source'))),
    environment:{HIVE_HISTORY_SOURCE:execution,HIVE_HISTORY_STATE:native,HIVE_HISTORY_LOG:log,
      HIVE_HISTORY_DROP:join(root,'never-drop'),HIVE_HISTORY_DROP_REVERT:join(root,'never-revert'),HIVE_HISTORY_DROP_BEFORE_REVERT:join(root,'never-before'),GIT_CONFIG_GLOBAL:join(root,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1'}};
  f.baseline=await snapshot(f);
  await mkdir(join(data,'desktop-drafts'),{recursive:true});await writeFile(join(data,'desktop-drafts',`session-${chat.id}.json`),JSON.stringify({key:`session:${chat.id}`,input:draft,failed:[]}));
  await writeFile(join(data,'desktop-language.json'),JSON.stringify('en'));
  await writeFile(info,JSON.stringify(f));
 } else {
  const f=JSON.parse(await readFile(info,'utf8'));Object.assign(process.env,f.environment);
  runtime=await createDesktopRuntime(data,()=>{});
  const opened=await runtime.chats.open(f.sessionId),observed=await snapshot(f),requests=await calls();
  const ops=(await runtime.chats.history.list(f.workspaceId)).operations;
  const draftNow=JSON.parse(await readFile(join(data,'desktop-drafts',`session-${f.sessionId}.json`),'utf8')).input;
  const record=identity(new ChatIndex(join(data,'chats')).get(f.sessionId));
  const mutationMethods=requests.filter(x=>['thread/fork','thread/revert','turn/start','turn/steer','account/read'].includes(x.method)).map(x=>x.method);
  const journals=await readdir(join(data,'history-operations')).catch(e=>{if(e.code==='ENOENT')return [];throw e;});
  const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
  const result={mode,sourceIdentity:record,draft:draftNow,history:opened.history,snapshot:observed,operations:ops,nativeMutations:mutationMethods,journals,
    baselineComparison:{sourceIdentity:same(record,f.identity),draft:draftNow===f.draft,history:same(opened.history,f.baselineHistory),
      headIndexDiskAndUnknownLock:same(observed,f.baseline),metadata:digest(await readFile(codexMetadataFile(join(data,'chats/codex'),'source')))===f.metadataSha256}};
  if(mode!=='inspect') {
   assert.deepEqual(record,f.identity);assert.equal(draftNow,f.draft);assert.deepEqual(JSON.parse(JSON.stringify(opened.history)),f.baselineHistory);assert.deepEqual(observed,f.baseline);
   assert.equal(digest(await readFile(codexMetadataFile(join(data,'chats/codex'),'source'))),f.metadataSha256);
   assert.deepEqual(mutationMethods,[]);assert.equal(ops.length,1);assert.equal(ops[0].state,'failed');assert.equal(ops[0].rewind,true);assert.match(ops[0].error,/Git index is locked by another operation/);
   assert.equal(ops[0].sourceSessionId,f.sessionId);assert.equal(ops[0].recoverable,false);
   const operation=JSON.parse(await readFile(join(data,'history-operations',journals.find(x=>x.endsWith('.json'))),'utf8'));
   assert.equal(operation.preview.input.boundary.nativeUserItemId,'u2');assert.equal(operation.preview.input.code.checkpointId,f.checkpointId);
   assert.equal(operation.childThreadId,undefined);assert.equal(operation.state,'failed');
   assert.equal(new ChatIndex(join(data,'chats')).list(f.workspaceId).length,1);
   assert.equal((await runtime.chats.history.capabilities(f.workspaceId,f.sessionId)).rewindCode,true);
   Object.assign(result,{originalIdentityPreserved:true,draftPreserved:true,fullHistoryPreserved:true,headIndexDiskAndUnknownLockPreserved:true,noNativeMutation:true,failedBeforeNativeBackup:true});
  }
  await writeFile(join(root,`rewind-lock-${mode}.json`),JSON.stringify(result,null,2));
 }
} finally {await runtime?.close();}
