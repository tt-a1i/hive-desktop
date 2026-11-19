/** Cold search and read-only history with a missing worktree and Hive metadata. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, rename, rm, stat, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {ChatIndex} from '../src/chat-index.js';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {CodexProjection} from '../src/agents/codex-projection.js';
import {codexMetadataFile, writeCodexMetadata} from '../src/agents/codex-metadata.js';

const root=await mkdtemp(join(tmpdir(),'hive-codex-metadata-recovery-'));
const project=join(root,'project'), data=join(root,'state'), agent=join(root,'agent');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const oldPi=process.env.PI_CODING_AGENT_DIR, oldOffline=process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR=agent; process.env.PI_OFFLINE='1';
try {
  await mkdir(project); await mkdir(agent);
  const git=(...args:string[])=>execFileSync('git',['-C',project,...args],{stdio:'pipe'});
  git('init','--template=');
  await writeFile(join(project,'file.txt'),'fixture');
  git('add','file.txt');
  git('-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false','commit','-m','fixture');
  runtime=await createDesktopRuntime(data,()=>{});
  const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Fixture',autostart_orchestrator:false}) as {id:string};
  const chat=await runtime.chats.createRecord(workspace.id,'codex',{isolation:'worktree'});
  const execution=(await runtime.chats.sessionExecution(chat.id)).path;
  await runtime.close(); runtime=undefined;
  const index=new ChatIndex(join(data,'chats'));
  index.save({...index.get(chat.id),codexThreadId:'fixture-thread',originAgentId:`${workspace.id}:orchestrator`,identityState:'known'});
  const dir=join(data,'chats','codex');
  const metadata=codexMetadataFile(dir,'fixture-thread');
  const attachment={id:'11111111-1111-4111-8111-111111111111',workspaceId:workspace.id,name:'file.txt',mimeType:'application/octet-stream',size:7,
    projectFile:{relativePath:'file.txt',sha256:'a'.repeat(64),sourceSessionId:chat.id}};
  writeCodexMetadata(dir,'fixture-thread',{version:1,context:[],completed:[],deliveries:[{turnId:'turn',itemId:'native-id',deliveryId:'delivery-id',
    presentation:{version:1,origin:'user',displayText:'visible words',attachments:[attachment]}}]});
  new CodexProjection(dir,'fixture-thread').recover([{id:'turn',status:'completed',items:[{id:'native-id',type:'userMessage',
    content:[{type:'text',text:'visible words\n\nprivate copy /fixture/file.txt'}]}]}],true);
  await rename(execution,execution+'.held');
  runtime=await createDesktopRuntime(data,()=>{});
  await runtime.chats.setArchived(chat.id,true);
  assert.ok((await stat(metadata+'.required')).isFile());
  const before=await runtime.chats.search('visible words',true,0,workspace.id);
  assert.equal(before.matches[0]?.bodyMatch?.messageId,'delivery-id');
  const opened=await runtime.chats.open(chat.id);
  assert.equal(opened.history[0]?.presentation?.deliveryId,'delivery-id');
  assert.equal(opened.history[0]?.presentation?.attachments?.[0]?.projectFile?.sourceSessionId,chat.id);
  await runtime.close(); runtime=undefined;
  await rename(metadata,metadata+'.held');
  runtime=await createDesktopRuntime(data,()=>{});
  const missing=await runtime.chats.search('private copy',true,0,workspace.id);
  assert.equal(missing.total,0);
  assert.equal(missing.incompleteHistoryCount,1);
  assert.match(missing.incompleteHistory[0]!.message,/Restore the original metadata file/);
  await rename(execution+'.held',execution);
  const blocked=await runtime.chats.open(chat.id);
  assert.equal(blocked.readOnly,true);
  assert.equal(blocked.identityError?.code,'CODEX_DELIVERY_METADATA_MISSING');
  assert.deepEqual(blocked.history,[]);
  assert.match(blocked.historyWarning!,/Restore the original metadata file/);
  await runtime.close(); runtime=undefined;
  await rename(metadata+'.held',metadata);
  await rename(execution,execution+'.held');
  runtime=await createDesktopRuntime(data,()=>{});
  const restored=await runtime.chats.search('visible words',true,0,workspace.id);
  assert.equal(restored.matches[0]?.bodyMatch?.messageId,'delivery-id');
  const recovered=await runtime.chats.open(chat.id);
  assert.equal(recovered.history[0]?.presentation?.deliveryId,restored.matches[0]?.bodyMatch?.messageId);
  await runtime.chats.setArchived(chat.id,false);
  assert.ok((await stat(metadata+'.required')).isFile());
  assert.equal((await runtime.chats.search('visible words',false,0,workspace.id)).matches[0]?.bodyMatch?.messageId,'delivery-id');
  const removed=await runtime.chats.removeWorkspace(workspace.id,async()=>{});
  assert.ok(removed.removedSessionIds.includes(chat.id));
  assert.ok((await stat(metadata)).isFile());
  assert.ok((await stat(metadata+'.required')).isFile());
  console.log('CODEX_METADATA_RECOVERY_OK');
} finally {
  await runtime?.close();
  if(oldPi===undefined)delete process.env.PI_CODING_AGENT_DIR;else process.env.PI_CODING_AGENT_DIR=oldPi;
  if(oldOffline===undefined)delete process.env.PI_OFFLINE;else process.env.PI_OFFLINE=oldOffline;
  await rm(root,{recursive:true,force:true,maxRetries:5});
}
