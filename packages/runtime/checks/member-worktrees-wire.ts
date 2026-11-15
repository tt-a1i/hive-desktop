/** Production JSONL APIs with real Hive SQLite/Git. Creation only: no native backend or model requests. */
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {once} from 'node:events';
import {mkdir,mkdtemp,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import {DatabaseSync} from 'node:sqlite';
type Frame={result?:unknown;error?:{code:string;message:string}};
type Session={id:string;backend:string;workspaceId:string;originAgentId?:string;agentId?:string;identityState:string;execution:{kind:string;path:string;branch:string}};
const directory=await mkdtemp(join(tmpdir(),'hive-member-worktrees-wire-')),repo=join(directory,'repo'),data=join(directory,'state');
await mkdir(repo);await mkdir(join(directory,'pi'));await writeFile(join(directory,'gitconfig'),'');
const git=(...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd:repo,stdio:'pipe'});
git('init','--template=');await writeFile(join(repo,'base.txt'),'base');git('add','.');git('commit','-qm','base');
const guard='const original=globalThis.fetch;globalThis.fetch=(input,init)=>{const url=new URL(typeof input==="string"?input:input instanceof URL?input:input.url);if(!["127.0.0.1","localhost","::1"].includes(url.hostname))return Promise.reject(new Error("Fixture disallows external fetch"));return original(input,init);};';
function start(){
 const child=spawn(process.execPath,['--import',`data:text/javascript,${encodeURIComponent(guard)}`,process.env.HIVE_DESKTOP_TEST_RUNTIME ?? 'dist/main.js'],{cwd:process.cwd(),env:{...process.env,HIVE_DESKTOP_DATA_DIR:data,PI_CODING_AGENT_DIR:join(directory,'pi'),PI_OFFLINE:'1',GIT_CONFIG_GLOBAL:join(directory,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1'},stdio:['pipe','pipe','pipe']});
 const exited=once(child,'exit');let id=0,stderr='';child.stderr.on('data',chunk=>{stderr+=String(chunk);});
 const pending=new Map<number,{resolve(frame:Frame):void;reject(error:Error):void}>();const lines=createInterface({input:child.stdout});
 lines.on('line',line=>{const frame=JSON.parse(line) as Frame&{id:number};const request=pending.get(frame.id);if(request){pending.delete(frame.id);request.resolve(frame);}});
 child.on('exit',()=>{for(const request of pending.values())request.reject(new Error('Runtime exited: '+stderr));pending.clear();});
 const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<Frame>((resolve,reject)=>{const requestId=++id,timer=setTimeout(()=>{pending.delete(requestId);reject(new Error('RPC timeout: '+method));},15000);pending.set(requestId,{resolve:frame=>{clearTimeout(timer);resolve(frame);},reject:error=>{clearTimeout(timer);reject(error);}});child.stdin.write(JSON.stringify({id:requestId,method,params})+'\n');});
 return {rpc,close:async()=>{if(child.exitCode===null){await rpc('shutdown');await exited;}lines.close();assert.equal(child.exitCode,0,stderr);}};
}
let wire=start(),db:DatabaseSync|undefined;
try{
 assert.ok((await wire.rpc('initialize')).result);
 const ws=(await wire.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:repo,name:'Wire creation fixture',autostart_orchestrator:false}})).result as {id:string};
 const sessions:Session[]=[];let workerId='';
 for(const backend of ['pi','codex','pi-rpc','claude']){
  const member=(await wire.rpc('team.member.create',{workspaceId:ws.id,name:`worker-${backend}`,role:'coder',backend})).result as {id:string};
  if(backend==='pi')workerId=member.id;
  const created=await wire.rpc('member.worktree.create',{workspaceId:ws.id,memberId:member.id});assert.equal(created.error,undefined);
  const session=created.result as Session;sessions.push(session);assert.equal(session.backend,backend);assert.equal(session.originAgentId,member.id);assert.equal(session.agentId,undefined);assert.equal(session.identityState,'known');assert.equal('sessionFile'in session,false);assert.equal('piSessionId'in session,false);
  const history=(await wire.rpc('member.worktrees',{workspaceId:ws.id,memberId:member.id})).result as {sessions:Session[]};assert.deepEqual(history.sessions.map(item=>item.id),[session.id]);
 }
 assert.equal((await wire.rpc('member.worktree.create',{workspaceId:ws.id,memberId:workerId,backend:'codex'})).error?.message.includes('[MEMBER_BACKEND_MISMATCH]'),true);
 assert.equal((await wire.rpc('member.worktree.create',{workspaceId:ws.id,memberId:`${ws.id}:orchestrator`})).error?.message.includes('[MEMBER_INVALID]'),true);
 db=new DatabaseSync(join(data,'hive/runtime.sqlite'));
 for(const table of ['messages','dispatches','dispatch_messages','agent_runs'])assert.equal((db.prepare('SELECT COUNT(*) AS n FROM '+table).get() as {n:number}).n,0,'create/list never starts native agents');
 const index=join(data,'chats/index.json');await rename(index,index+'.held');await mkdir(index);
 try{assert.ok((await wire.rpc('member.worktree.create',{workspaceId:ws.id,memberId:workerId})).error?.message.includes('工作树已保留'));}finally{await rm(index,{recursive:true});await rename(index+'.held',index);}
 const pending=(await wire.rpc('chat.worktrees.pending',{workspaceId:ws.id})).result as {pending:{sessionId:string;backend:string;originAgentId:string}[]};assert.equal(pending.pending.length,1);const orphan=pending.pending[0]!;assert.equal(orphan.originAgentId,workerId);
 db.close();db=undefined;await wire.close();wire=start();await wire.rpc('initialize');
 assert.ok((await wire.rpc('chat.worktrees.recover',{workspaceId:ws.id,sessionId:orphan.sessionId,backend:'codex'})).error?.message.includes('SESSION_IDENTITY_CONFLICT'));
 const recovered=(await wire.rpc('chat.worktrees.recover',{workspaceId:ws.id,sessionId:orphan.sessionId,memberId:workerId})).result as Session;assert.equal(recovered.originAgentId,workerId);assert.equal(recovered.backend,'pi');assert.equal(recovered.agentId,undefined);
 const retry=await wire.rpc('chat.worktrees.recover',{workspaceId:ws.id,sessionId:orphan.sessionId,memberId:workerId});assert.deepEqual(retry.result,recovered);
 assert.ok((await wire.rpc('chat.worktrees.recover',{workspaceId:ws.id,sessionId:orphan.sessionId,backend:'codex'})).error?.message.includes('SESSION_IDENTITY_CONFLICT'));
 assert.ok((await wire.rpc('chat.worktrees.recover',{workspaceId:ws.id,sessionId:orphan.sessionId,memberId:`${ws.id}:orchestrator`})).error?.message.includes('SESSION_IDENTITY_CONFLICT'));
 const intent=JSON.parse(await readFile(join(data,'worktrees',recovered.id,'intent.json'),'utf8'));assert.equal(intent.creation.originAgentId,workerId);assert.equal(intent.version,3);assert.equal(intent.creation.version,2);assert.deepEqual(intent.creation.setup,{enabled:false});
 await wire.close();await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,productionJsonl:true,realHiveSqliteGit:true,backends:sessions.map(item=>item.backend),noRuns:true,coldRecovery:true},null,2)+'\n');console.log(JSON.stringify({pass:true,directory}));
}finally{db?.close();await wire.close();}
