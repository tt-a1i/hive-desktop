/** Actual Hive Worker dispatcher; controlled host-call gate, no model or HTTP. */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runWorkflowScriptWorker } from '@tt-a1i/hive/dist/src/server/workflow-script-worker.js';
const dir=await mkdtemp(join(tmpdir(),'hive-workflow-worker-gate-'));
let enter!:()=>void,release!:()=>void,exited!:()=>void,ownershipReleased!:()=>void;
const entered=new Promise<void>(r=>enter=r),gate=new Promise<void>(r=>release=r),workerExit=new Promise<void>(r=>exited=r),released=new Promise<void>(r=>ownershipReleased=r);
let owned=false;let callbacks=0;let hostDone=false;
class CheckTimeoutError extends Error {}
const limit=async<T>(p:Promise<T>)=>{let timer:ReturnType<typeof setTimeout>|undefined;try{return await Promise.race([p,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new CheckTimeoutError('Worker check timed out')),5000)})]);}finally{clearTimeout(timer);}};
const workerSource=`const {parentPort}=require('node:worker_threads');parentPort.postMessage({type:'hostCall',id:'owned',name:'agent',args:['fixture-only',{}]});parentPort.postMessage({type:'done',ok:false,error:'fixture logical failure'});`;
const task=runWorkflowScriptWorker({args:undefined,compiledFunctionSource:'',scriptPath:'owned-fixture',workerSource,
 onWorkerCreated(worker){owned=true;worker.once('exit',()=>exited());},onWorkerSettled(){owned=false;callbacks++;ownershipReleased();},
 handlers:{assertRunActive(){},agent:async()=>{enter();await gate;hostDone=true;return 'finished';},phase(){},log(){},workflow:async()=>{},catchPerItem(){},cancelDagLayerAgents(){}}});
void task.catch(()=>{});
try{
 await limit(entered);await assert.rejects(limit(task),(error:unknown)=>error instanceof Error && !(error instanceof CheckTimeoutError));await limit(workerExit);
 // The actual worker has exited and logical rejection is delivered; its accepted host call is still held.
 assert.equal(hostDone,false);assert.equal(owned,true);assert.equal(callbacks,0);
 release();await limit(released);assert.equal(hostDone,true);assert.equal(owned,false);assert.equal(callbacks,1);
 await writeFile(join(dir,'receipt.json'),JSON.stringify({pass:true,noModel:true,realWorker:true,syntheticWorkerSource:true,logicalRejectBeforeHostCallCompletion:true,workerExitedWhileHostCallPending:true,ownershipCallbackHeldUntilBoth:true,scope:'module-level actual Worker dispatcher; lease/HTTP integration verified separately'},null,2));console.log('Evidence:',dir);
}catch(error){await writeFile(join(dir,'failure.json'),JSON.stringify({error:String(error),owned,callbacks,hostDone},null,2));console.error('Evidence:',dir);throw error;}finally{release();}
