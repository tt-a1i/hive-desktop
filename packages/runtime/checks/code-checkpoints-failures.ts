/** Actual Git delegated through a fixture executable; inject real source/disk failures, never synthetic success. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,readFile,writeFile,rename,rm,chmod,readdir,symlink} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {CodeCheckpoints} from '../src/code-checkpoints.js';
if(process.platform==='win32')throw Error('POSIX executable/rename fault fixture only');
const root=await mkdtemp(join(tmpdir(),'hive-checkpoint-faults-')),repo=join(root,'repo'),data=join(root,'data');await mkdir(repo);
const realGit=String(execFileSync('which',['git'],{encoding:'utf8'})).trim();
const git=(...args:string[])=>execFileSync(realGit,['-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:repo,stdio:'pipe'});
const context={dataDir:data,workspaceId:'fault-workspace',sessionId:'fault-session',executionPath:repo},hash=(value:Buffer|string)=>createHash('sha256').update(value).digest('hex');
const scope=join(data,'code-checkpoints',hash(JSON.stringify([context.workspaceId,context.sessionId]))),control=join(root,'control.json'),script=join(root,'git-peer.cjs'),wrapper=join(root,'git-peer');
const quote=(value:string)=>"'"+value.replaceAll("'","'\\''")+"'";
try{
 git('init','--template=');await writeFile(join(repo,'file'),'A');git('add','.');git('commit','-m','fixture');
 await writeFile(script,`const fs=require('node:fs'),cp=require('node:child_process');
const args=process.argv.slice(2),result=cp.spawnSync(${JSON.stringify(realGit)},args,{cwd:process.cwd(),env:process.env,input:fs.readFileSync(0)});
if(args.includes('--batch')&&fs.existsSync(${JSON.stringify(control)})){
 const c=JSON.parse(fs.readFileSync(${JSON.stringify(control)},'utf8'));
 if(c.mode==='source'){c.count=(c.count||0)+1;fs.writeFileSync(c.path,'external-during-'+c.count);fs.writeFileSync(${JSON.stringify(control)},JSON.stringify(c));}
 if(c.mode==='storage'){fs.renameSync(c.path,c.path+'-missing');fs.unlinkSync(${JSON.stringify(control)});}
 if(c.mode==='storage-link'){fs.renameSync(c.path,c.path+'-missing');fs.symlinkSync(c.source,c.path,'dir');fs.unlinkSync(${JSON.stringify(control)});}
}
if(result.stdout)process.stdout.write(result.stdout);if(result.stderr)process.stderr.write(result.stderr);process.exitCode=result.status===null?1:result.status;
`);
 await writeFile(wrapper,`#!/bin/sh\nexec ${quote(process.execPath)} ${quote(script)} "$@"\n`);await chmod(wrapper,0o755);
 const module=new CodeCheckpoints(context),preview=await module.preview(),indexBefore=await readFile(join(repo,'.git','index')),headBefore=git('rev-parse','HEAD');
 const racing=new CodeCheckpoints(context,{gitExecutable:wrapper});await writeFile(control,JSON.stringify({mode:'source',path:join(repo,'file')}));const raceId=randomUUID();
 await assert.rejects(racing.capture({expectedVersion:preview.version,requestId:raceId}),error=>(error as {code:string}).code==='CONCURRENT_CHANGE');await rm(control);assert.equal((await module.get(raceId))!.state,'failed');assert.match(await readFile(join(repo,'file'),'utf8'),/^external-during-/);assert.deepEqual(await readFile(join(repo,'.git','index')),indexBefore);assert.deepEqual(git('rev-parse','HEAD'),headBefore);
 // Lose the requests directory after durable claim and an actual Git raw-object read.
 const current=await module.preview(),failedId=randomUUID();await writeFile(control,JSON.stringify({mode:'storage',path:join(scope,'requests')}));
 await assert.rejects(racing.capture({expectedVersion:current.version,requestId:failedId}),error=>(error as {code:string}).code==='WRITE_FAILED');
 await rename(join(scope,'requests-missing'),join(scope,'requests'));const incomplete=await module.get(failedId);assert.equal(incomplete!.state,'capturing');
 const beforeSnapshots=await readdir(join(scope,'snapshots'));await rename(repo,repo+'-missing');
 const cold=new CodeCheckpoints(context);assert.equal((await cold.capture({expectedVersion:current.version,requestId:failedId})).checkpointId,incomplete!.checkpointId);assert.equal((await cold.get(failedId))!.state,'capturing');
 await assert.rejects(cold.export(incomplete!.checkpointId),error=>(error as {code:string}).code==='INCOMPLETE');assert.deepEqual(await readdir(join(scope,'snapshots')),beforeSnapshots);await rename(repo+'-missing',repo);
 assert.deepEqual(await readFile(join(repo,'.git','index')),indexBefore);
 const treeBefore=(await readdir(repo)).sort(),linkedId=randomUUID();
 await writeFile(control,JSON.stringify({mode:'storage-link',path:join(scope,'snapshots'),source:repo}));
 await assert.rejects(racing.capture({expectedVersion:(await module.preview()).version,requestId:linkedId}),error=>(error as {code:string}).code==='STORAGE_UNSAFE');
 await rm(join(scope,'snapshots'));await rename(join(scope,'snapshots-missing'),join(scope,'snapshots'));
 assert.deepEqual((await readdir(repo)).sort(),treeBefore,'linked private storage must not create any source directory');assert.equal((await module.get(linkedId))!.state,'failed');
 // A finished private checkpoint fails closed for malformed manifests/blobs, not just missing source.
 const ready=await module.capture({expectedVersion:(await module.preview()).version,requestId:randomUUID()});const snapshot=join(scope,'snapshots',ready.checkpointId),manifestFile=join(snapshot,'manifest.json'),manifest=await readFile(manifestFile);
 await writeFile(manifestFile,'{partial');await assert.rejects(module.get(ready.requestId),error=>(error as {code:string}).code==='CORRUPT');await assert.rejects(module.capture({expectedVersion:ready.expectedVersion,requestId:ready.requestId}),error=>(error as {code:string}).code==='CORRUPT');await writeFile(manifestFile,manifest);
 const exported=await module.export(ready.checkpointId),blob=join(snapshot,'blobs',exported.disk[0]!.entry.sha256),original=await readFile(blob);await writeFile(blob,'tampered');await assert.rejects(module.get(ready.requestId),error=>(error as {code:string}).code==='CORRUPT');await writeFile(blob,original);
 const outside=join(root,'outside-blobs');await mkdir(outside);for(const name of await readdir(join(snapshot,'blobs')))await writeFile(join(outside,name),await readFile(join(snapshot,'blobs',name)));
 await rename(join(snapshot,'blobs'),join(snapshot,'blobs-original'));await symlink(outside,join(snapshot,'blobs'));await assert.rejects(module.export(ready.checkpointId),error=>(error as {code:string}).code==='CORRUPT');await rm(join(snapshot,'blobs'));await rename(join(snapshot,'blobs-original'),join(snapshot,'blobs'));
 const partial=randomUUID();await writeFile(join(scope,'requests',partial+'.json'),'{');await assert.rejects(module.capture({expectedVersion:ready.expectedVersion,requestId:partial}),error=>(error as {code:string}).code==='CORRUPT');assert.equal(await readFile(join(scope,'requests',partial+'.json'),'utf8'),'{');
 assert.equal((await module.get(ready.requestId))!.state,'ready');
 console.log('CODE_CHECKPOINT_FAILURES_OK: actual Git source race refused, durable claim before storage failure, cold incomplete/no recapture, partial publication not exportable, corruption and private-parent symlink fail closed; source index/HEAD remain exact.');
}finally{await rm(root,{recursive:true,force:true});}
