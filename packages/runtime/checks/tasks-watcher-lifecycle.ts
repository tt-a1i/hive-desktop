/** Real Chokidar/filesystem lifecycle of the fixed Hive dependency, without model calls. */
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {mkdtemp,mkdir,writeFile,rename,access,rm} from 'node:fs/promises';
import {dirname,join} from 'node:path';
import {tmpdir} from 'node:os';
const require=createRequire(import.meta.url);
const hive=dirname(require.resolve('@tt-a1i/hive/package.json'));
const {createTasksFileWatcher}=await import(pathToFileURL(join(hive,'dist/src/server/tasks-file-watcher.js')).href);
const directory=await mkdtemp(join(tmpdir(),'hive-tasks-watcher-lifecycle-'));
async function deadline<T>(promise:Promise<T>,label:string,ms=5000){let timer:ReturnType<typeof setTimeout>;try{return await Promise.race([promise,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error(label)),ms);})]);}finally{clearTimeout(timer!);}}
const earlyStartup=async()=>{for(let i=0;i<4;i++)await Promise.resolve();};
try{
 for(let i=0;i<10;i++){
  const watcher=createTasksFileWatcher({onTasksUpdated(){throw new Error('Closed watcher emitted a callback');}});
  const starting=watcher.start('close',directory);void starting.catch(()=>{});
  if(i%2)await earlyStartup();await watcher.close();
  await deadline(starting,'Closing before ready must settle start, not wait for native ready timeout',2000);
  const closedProject=join(directory,'must-not-create-'+i);await watcher.start('after-close',closedProject);
  await assert.rejects(access(join(closedProject,'.hive')),error=>(error as NodeJS.ErrnoException).code==='ENOENT');
 }
 let expected='';let receive!:(content:string)=>void;
 const watcher=createTasksFileWatcher({onTasksUpdated(id:string,content:string){assert.equal(id,'restart');if(content===expected)receive(content);}});
 try{
  const starting=watcher.start('restart',directory);void starting.catch(()=>{});await earlyStartup();await watcher.stop('restart');
  await deadline(starting,'Stop before ready must release the replaced start',2000);
  await deadline(watcher.start('restart',directory),'Restart must reach real ready');
  for(const atomic of [false,true]){
   expected=atomic?'Atomic save after restart':'Direct save after restart';
   const delivered=new Promise<string>(resolve=>receive=resolve),tasks=join(directory,'.hive/tasks.md');
   if(atomic){const temp=tasks+'.tmp';await writeFile(temp,expected);await rename(temp,tasks);}else await writeFile(tasks,expected);
   assert.equal(await deadline(delivered,'Real file change must reach task listener'),expected);
  }
  const file=join(directory,'not-a-directory');await writeFile(file,'owned fixture');
  await assert.rejects(watcher.start('bad',file),error=>['ENOTDIR','EEXIST'].includes((error as NodeJS.ErrnoException).code??''));
 }finally{await watcher.close();}
 console.log('TASKS_WATCHER_LIFECYCLE_OK: ten close-before-ready cycles, terminal close, stop-before-ready/restart, actual direct/atomic file events and filesystem error. Real Chokidar; no models.');
}finally{await rm(directory,{recursive:true,force:true});}
