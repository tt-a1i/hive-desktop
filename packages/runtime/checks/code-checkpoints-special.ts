/** Actual FIFO/Unix socket inventory: never read special-node data or follow symlinks. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {createServer,type Server} from 'node:net';
import {mkdtemp,mkdir,writeFile,readFile,rm,lstat,symlink,chmod} from 'node:fs/promises';
import {join} from 'node:path';
import {CodeCheckpoints} from '../src/code-checkpoints.js';
if(process.platform==='win32')throw Error('FIFO/Unix socket fixture is POSIX native evidence only');
// Darwin Unix socket addresses need a short path; all content is an exclusive fixture.
const root=await mkdtemp('/tmp/hcp-special-'),repo=join(root,'repo'),data=join(root,'data');await mkdir(repo);
const git=(...args:string[])=>execFileSync('git',['-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd:repo,stdio:'pipe'});
const servers:Server[]=[];
const socket=async(path:string)=>{const server=createServer();servers.push(server);await new Promise<void>((resolve,reject)=>{server.once('error',reject);server.listen(path,()=>resolve());});return server;};
const context={dataDir:data,workspaceId:'special-fixture',sessionId:'special-session',executionPath:repo};const module=new CodeCheckpoints(context);
const refused=async()=>{const preview=await module.preview();assert.equal(preview.canCapture,false);assert.ok(preview.unsupported.some(value=>value.code==='UNSUPPORTED_SPECIAL_FILE'),JSON.stringify(preview.unsupported));const requestId=randomUUID();await assert.rejects(module.capture({expectedVersion:preview.version,requestId}),error=>(error as {code:string}).code==='UNSUPPORTED');assert.equal((await module.get(requestId))!.state,'failed');};
try{
 git('init','--template=');await writeFile(join(repo,'tracked'),'A');await writeFile(join(repo,'.gitignore'),'ignored.pipe\nignored.sock\nignored-dir/\nmixed/\n');await mkdir(join(repo,'mixed'));await writeFile(join(repo,'mixed','kept'),'tracked');git('add','.');git('add','-f','mixed/kept');git('commit','-m','fixture');const index=await readFile(join(repo,'.git','index')),head=git('rev-parse','HEAD');
 const before=await module.preview();assert.equal(before.canCapture,true);
 execFileSync('mkfifo',[join(repo,'pipe')]);assert.equal((await lstat(join(repo,'pipe'))).isFIFO(),true);await refused();assert.equal((await lstat(join(repo,'pipe'))).isFIFO(),true);await rm(join(repo,'pipe'));
 const active=await socket(join(repo,'socket'));assert.equal((await lstat(join(repo,'socket'))).isSocket(),true);await refused();await new Promise<void>((resolve,reject)=>active.close(error=>error?reject(error):resolve()));
 // Ignored special nodes and ignored subtrees remain untouched and are explicit exclusions.
 execFileSync('mkfifo',[join(repo,'mixed','ignored.pipe')]);
 execFileSync('mkfifo',[join(repo,'ignored.pipe')]);const ignoredSocket=await socket(join(repo,'ignored.sock'));await mkdir(join(repo,'ignored-dir'));execFileSync('mkfifo',[join(repo,'ignored-dir','nested-pipe')]);
 const ignored=await module.preview();assert.equal(ignored.canCapture,true,JSON.stringify(ignored.unsupported));assert.ok(ignored.exclusions.includes('ignored.pipe'));assert.ok(ignored.exclusions.includes('ignored.sock'));assert.ok(ignored.exclusions.includes('ignored-dir/'));assert.ok(ignored.exclusions.includes('mixed/ignored.pipe'));assert.equal(ignored.exclusions.includes('mixed/'),false);
 const ready=await module.capture({expectedVersion:ignored.version,requestId:randomUUID()});const exported=await module.export(ready.checkpointId);assert.equal(exported.disk.find(value=>value.entry.path==='mixed/kept')!.data.toString(),'tracked');assert.equal(exported.disk.some(value=>value.entry.path.startsWith('ignored')),false);assert.equal((await lstat(join(repo,'ignored.pipe'))).isFIFO(),true);assert.equal((await lstat(join(repo,'ignored.sock'))).isSocket(),true);
 // A symlink to a FIFO outside the project is captured as target bytes, never followed.
 const outside=join(root,'outside-pipe');execFileSync('mkfifo',[outside]);await symlink('../outside-pipe',join(repo,'link'));const linked=await module.preview();assert.equal(linked.canCapture,true,JSON.stringify(linked.unsupported));const linkedReady=await module.capture({expectedVersion:linked.version,requestId:randomUUID()});assert.equal((await module.export(linkedReady.checkpointId)).disk.find(value=>value.entry.path==='link')!.data.toString(),'../outside-pipe');
 // New empty directories are declared non-content but still change inventory version.
 const token=(await module.preview()).version;await mkdir(join(repo,'new-empty'));const withDirectory=await module.preview();assert.notEqual(withDirectory.version,token);await assert.rejects(module.capture({expectedVersion:token,requestId:randomUUID()}),error=>(error as {code:string}).code==='STALE_SOURCE');
 // An actual new FIFO introduced after the first inventory cannot pass the later verification.
 const control=join(root,'race-once'),peer=join(root,'git-peer.cjs'),wrapper=join(root,'git-peer'),racingPipe=join(repo,'racing.pipe');
 const realGit=String(execFileSync('which',['git'],{encoding:'utf8'})).trim(),quote=(value:string)=>"'"+value.replaceAll("'","'\\''")+"'";
 await writeFile(peer,`const fs=require('node:fs'),cp=require('node:child_process');const args=process.argv.slice(2),r=cp.spawnSync(${JSON.stringify(realGit)},args,{input:fs.readFileSync(0),cwd:process.cwd(),env:process.env});if(args.includes('--batch')&&fs.existsSync(${JSON.stringify(control)})){fs.unlinkSync(${JSON.stringify(control)});cp.execFileSync('mkfifo',[${JSON.stringify(racingPipe)}]);}if(r.stdout)process.stdout.write(r.stdout);if(r.stderr)process.stderr.write(r.stderr);process.exitCode=r.status===null?1:r.status;`);
 await writeFile(wrapper,`#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`);await chmod(wrapper,0o755);
 const raceToken=(await module.preview()).version;await writeFile(control,'1');const racing=new CodeCheckpoints(context,{gitExecutable:wrapper}),raceId=randomUUID();
 await assert.rejects(racing.capture({expectedVersion:raceToken,requestId:raceId}),error=>(error as {code:string}).code==='CONCURRENT_CHANGE');assert.equal((await module.get(raceId))!.state,'failed');assert.ok((await lstat(racingPipe)).isFIFO());await rm(racingPipe);
 // Negation must re-include a special node instead of silently treating it as ignored.
 const originalRules=await readFile(join(repo,'.gitignore'));await writeFile(join(repo,'.gitignore'),Buffer.concat([originalRules,Buffer.from('*.pipe\n!negated.pipe\n')]));execFileSync('mkfifo',[join(repo,'negated.pipe')]);await refused();await rm(join(repo,'negated.pipe'));await writeFile(join(repo,'.gitignore'),originalRules);
 // Git itself would block reading a FIFO .gitignore: detect it before ignore/status commands.
 const rules=await readFile(join(repo,'.gitignore'));await rm(join(repo,'.gitignore'));execFileSync('mkfifo',[join(repo,'.gitignore')]);await refused();await rm(join(repo,'.gitignore'));await writeFile(join(repo,'.gitignore'),rules);
 assert.deepEqual(await readFile(join(repo,'.git','index')),index);assert.deepEqual(git('rev-parse','HEAD'),head);assert.equal(await readFile(join(repo,'tracked'),'utf8'),'A');assert.ok(ignoredSocket.listening);
 // A previously published v1 checkpoint has no proof of complete real-FS inventory.
 const legacyRequest=randomUUID(),legacy=await module.capture({expectedVersion:(await module.preview()).version,requestId:legacyRequest});
 const scope=join(data,'code-checkpoints',createHash('sha256').update(JSON.stringify([context.workspaceId,context.sessionId])).digest('hex')),manifestFile=join(scope,'snapshots',legacy.checkpointId,'manifest.json'),requestFile=join(scope,'requests',legacyRequest+'.json');
 const legacyManifest=Buffer.from(JSON.stringify({...JSON.parse(await readFile(manifestFile,'utf8')),version:1}));await writeFile(manifestFile,legacyManifest);await writeFile(requestFile,JSON.stringify({...legacy,manifestSha256:createHash('sha256').update(legacyManifest).digest('hex')}));
 const noGit=new CodeCheckpoints(context,{gitExecutable:join(root,'does-not-exist')});await assert.rejects(noGit.get(legacyRequest),error=>(error as {code:string}).code==='CORRUPT');await assert.rejects(noGit.capture({expectedVersion:legacy.expectedVersion,requestId:legacyRequest}),error=>(error as {code:string}).code==='CORRUPT');
 console.log('CODE_CHECKPOINT_SPECIAL_OK: actual nonignored FIFO/socket refused, ignored special nodes pruned/preserved/excluded, no symlink-follow, special rule files rejected before Git reads, full inventory affects source version; source index/HEAD/code unchanged.');
}finally{for(const server of servers)if(server.listening)await new Promise<void>(resolve=>server.close(()=>resolve()));await rm(root,{recursive:true,force:true});}
