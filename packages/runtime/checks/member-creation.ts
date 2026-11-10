/** Production JSONL + true Hive HTTP/SQLite. No Agent or vendor request. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import {DatabaseSync} from 'node:sqlite';
const directory=await mkdtemp(join(tmpdir(),'hive-member-creation-'));
await mkdir(join(directory,'pi'));
const dataDir=join(directory,'state');
function start() {
 const child=spawn(process.execPath,['dist/main.js'],{cwd:process.cwd(),env:{...process.env,HIVE_DESKTOP_DATA_DIR:dataDir,PI_CODING_AGENT_DIR:join(directory,'pi'),PI_OFFLINE:'1'},stdio:['pipe','pipe','pipe']});
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
let wire=start();let db:DatabaseSync|undefined;
try {
 assert.ok((await wire.rpc('initialize')).result.hivePort);
 const workspace=(await wire.rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:directory,name:'Atomic members',autostart_orchestrator:false}})).result;
 const workspaceId=workspace.id;assert.equal(typeof workspaceId,'string');
 const http=(method:string,path:string,body?:unknown)=>wire.rpc('hive.request',{method,path,...(body!==undefined&&{body})});
 const members=async()=>(await wire.rpc('team.members',{workspaceId})).result as any[];
 const create=(name:string,backend:unknown,extra:Record<string,unknown>={})=>wire.rpc('team.member.create',{workspaceId,name,role:'reviewer',backend,...extra});
 assert.deepEqual(await members(),[]);
 for(const input of [{backend:'shell'},{backend:'hive:codex'},{role:'orchestrator'},{description:42},{name:'   '}]) {
  const rejected=await create('Invalid member','codex',input);assert.equal(rejected.error?.code,'DESKTOP_ERROR');assert.deepEqual(await members(),[]);
 }
 await http('PUT',`/api/settings/app-state/workspace.${workspaceId}.ui_language`,{value:'en'});
 const reference=(await http('POST',`/api/workspaces/${workspaceId}/workers`,{name:'Native defaults',role:'reviewer',autostart:false})).result;
 const defaults=(await members()).find(member=>member.id===reference.id).description;
 const ids:string[]=[];
 for(const backend of ['pi','codex','pi-rpc','claude']) {
  const reply=await create('Desktop '+backend,backend);assert.equal(reply.error,undefined);
  const member=(await members()).find(member=>member.id===reply.result.id);assert.ok(member);
  assert.equal(member.configured_command,'hive:'+backend);assert.equal(member.role,'reviewer');assert.equal(member.status,'stopped');assert.equal(member.pending_task_count,0);assert.equal(member.description,defaults);ids.push(member.id);
 }
 db=new DatabaseSync(join(dataDir,'hive/runtime.sqlite'));
 db.exec("CREATE TRIGGER owned_reject_member_config BEFORE INSERT ON agent_launch_configs WHEN NEW.command = 'hive:codex' BEGIN SELECT RAISE(ABORT, 'OWNED_CONFIG_WRITE_FAILURE'); END");
 const before=await members();
 const failed=await create('Retry original name','codex',{description:'完整职责\n保留第二行'});assert.equal(failed.error?.code,'DESKTOP_ERROR');
 assert.deepEqual(await members(),before);
 assert.equal((db.prepare('SELECT COUNT(*) AS n FROM workers WHERE name = ?').get('Retry original name') as {n:number}).n,0);
 db.exec('DROP TRIGGER owned_reject_member_config');
 const retried=await create('Retry original name','codex',{description:'完整职责\n保留第二行'});assert.equal(retried.error,undefined);
 const after=await members();assert.equal(after.length,before.length+1);
 const saved=after.find(member=>member.id===retried.result.id);assert.equal(saved.description,'完整职责\n保留第二行');assert.equal(saved.configured_command,'hive:codex');assert.equal(saved.status,'stopped');
 const duplicate=await create('Retry original name','claude');assert.equal(duplicate.error?.code,'DESKTOP_ERROR');assert.deepEqual(await members(),after);
 const missing=await wire.rpc('team.member.create',{workspaceId:'missing-workspace',name:'Missing',role:'coder',backend:'pi'});assert.equal(missing.error?.code,'DESKTOP_ERROR');assert.deepEqual(await members(),after);
 const launchRows=db.prepare('SELECT agent_id, command, args_json AS args FROM agent_launch_configs WHERE workspace_id = ? AND agent_id <> ? ORDER BY agent_id').all(workspaceId,workspaceId+':orchestrator') as {agent_id:string;command:string;args:string}[];
 assert.equal(launchRows.length,5);assert.deepEqual(new Set(launchRows.map(row=>row.agent_id)),new Set([...ids,saved.id]));assert.ok(launchRows.every(row=>JSON.parse(row.args).length===0));
 for(const table of ['messages','dispatches','dispatch_messages','agent_runs','workflow_runs'])assert.equal((db.prepare('SELECT COUNT(*) AS n FROM '+table).get() as {n:number}).n,0);
 db.close();db=undefined;await wire.close();wire=start();await wire.rpc('initialize');
 assert.deepEqual(await members(),after);
 await wire.close();
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,productionJsonl:true,realHttpSqlite:true,failedConfigRolledBack:true,originalNameRetrySucceeded:true,backends:4,memberCount:after.length,launchCount:5,coldIdentityUnchanged:true,noInference:true},null,2)+'\n');
 console.log(JSON.stringify({pass:true,directory,noInference:true}));
} finally {db?.close();await wire.close();}
