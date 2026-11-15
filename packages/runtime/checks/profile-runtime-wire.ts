/** Production JSONL + true Hive HTTP/SQLite. No Agent or vendor request. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
const directory=await mkdtemp(join(tmpdir(),'hive-profile-wire-'));
await mkdir(join(directory,'pi'));
const dataDir=join(directory,'state');
function start() {
 const child=spawn(process.execPath,[process.env.HIVE_DESKTOP_TEST_RUNTIME ?? 'dist/main.js'],{cwd:process.cwd(),env:{...process.env,HIVE_DESKTOP_DATA_DIR:dataDir,PI_CODING_AGENT_DIR:join(directory,'pi'),PI_OFFLINE:'1'},stdio:['pipe','pipe','pipe']});
 const exited=once(child,'exit');let sequence=0,stderr='';
 child.stderr.on('data',chunk=>{stderr+=String(chunk);});
 const pending=new Map<number,{resolve(value:any):void;reject(error:Error):void}>();
 const lines=createInterface({input:child.stdout});
 lines.on('line',line=>{const frame=JSON.parse(line);const request=pending.get(frame.id);if(request){pending.delete(frame.id);request.resolve(frame);}});
 child.on('exit',()=>{for(const request of pending.values())request.reject(new Error('Runtime exited: '+stderr));pending.clear();});
 const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<any>((resolve,reject)=>{
  const id=++sequence,timer=setTimeout(()=>{pending.delete(id);reject(new Error('Member request timed out: '+method));},10000);
  pending.set(id,{resolve:value=>{clearTimeout(timer);resolve(value);},reject:error=>{clearTimeout(timer);reject(error);}});
  child.stdin.write(JSON.stringify({id,method,params})+'\n');
 });
 return {child,rpc,exited,close:async()=>{if(child.exitCode===null){await rpc('shutdown');await exited;}lines.close();assert.equal(child.exitCode,0);}};
}
let wire=start();
try {
 assert.ok((await wire.rpc('initialize')).result.hivePort);
 const workspace=(await wire.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:directory,name:'Profile wire',autostart_orchestrator:false}})).result;
 let list=(await wire.rpc('agent.profiles.list')).result;assert.equal(list.profiles.length,4);
 const invalid=await wire.rpc('agent.profiles.create',{driver:'codex',label:'No extra',launch:{command:'never-start-this',args:[]},expectedStoreVersion:list.storeVersion,extra:true});assert.ok(invalid.error);
 const created=await wire.rpc('agent.profiles.create',{driver:'codex',label:'Wire profile',launch:{command:'never-start-this',args:['--fixture','wire']},expectedStoreVersion:list.storeVersion});assert.equal(created.error,undefined);const ref=created.result.profile.ref;
 const stale=await wire.rpc('agent.profiles.update',{id:ref.id,label:'Stale',expectedStoreVersion:list.storeVersion});assert.ok(stale.error);
 const chat=await wire.rpc('chat.createRecord',{workspaceId:workspace.id,backend:'codex',profile:ref});assert.equal(chat.error,undefined);assert.deepEqual(chat.result.profile,ref);
 const invalidChat=await wire.rpc('chat.createRecord',{workspaceId:workspace.id,backend:'codex',profile:{...ref,revision:0}});assert.ok(invalidChat.error);
 const mixed=await wire.rpc('desktop.settings.codexAccount',{command:'never-start-this',profile:ref});assert.ok(mixed.error);
 const invalidCatalog=await wire.rpc('agent.models',{workspaceId:workspace.id,backend:'codex',profile:ref,extra:true});assert.ok(invalidCatalog.error);
 const member=await wire.rpc('team.member.create',{workspaceId:workspace.id,name:'Profile worker',role:'coder',backend:'codex',profile:ref,model:{provider:'fixture',id:'wire'}});assert.equal(member.error,undefined);
 const members=(await wire.rpc('team.members',{workspaceId:workspace.id})).result;const savedMember=members.find((m:any)=>m.id===member.result.id);assert.deepEqual(savedMember.desktop_profile,ref);assert.deepEqual(savedMember.desktop_model,{provider:'fixture',id:'wire'});
 list=(await wire.rpc('agent.profiles.list')).result;
 const disabled=await wire.rpc('agent.profiles.disable',{id:ref.id,expectedStoreVersion:list.storeVersion});assert.equal(disabled.error,undefined);
 const rejected=await wire.rpc('chat.createRecord',{workspaceId:workspace.id,backend:'codex',profile:ref});assert.ok(rejected.error);
 const offline=await wire.rpc('chat.open',{sessionId:chat.result.id});assert.equal(offline.error,undefined);assert.equal(offline.result.readOnly,true);
 const catalog=await wire.rpc('agent.models',{workspaceId:workspace.id,backend:'codex',profile:ref});assert.ok(catalog.error);
 const account=await wire.rpc('desktop.settings.codexAccount',{profile:ref});assert.ok(account.error);
 await wire.close();wire=start();await wire.rpc('initialize');
 const after=(await wire.rpc('chat.list',{workspaceId:workspace.id})).result;assert.deepEqual(after[0].profile,ref);
 list=(await wire.rpc('agent.profiles.list')).result;assert.equal(list.profiles.find((p:any)=>p.id===ref.id).enabled,false);
 console.log(JSON.stringify({pass:true,directory,productionJsonl:true,strictProfileSchemas:true,coldRefs:true,disabledNoProcess:true,noInference:true}));
}finally{await wire.close();}
