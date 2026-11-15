/** Real Git HEAD/index/disk snapshots; this is an internal export module, not a shipped fork workflow. */
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,readFile,writeFile,rm,rename,symlink,readlink,chmod,stat,readdir} from 'node:fs/promises';
import {join,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {CodeCheckpoints,type ExportedCodeCheckpoint} from '../src/code-checkpoints.js';
if(process.platform==='win32')throw Error('This real symlink/mode fixture is POSIX native evidence only');
const directory=await mkdtemp(join(tmpdir(),'hive-code-checkpoints-')),repo=join(directory,'repo'),data=join(directory,'data');await mkdir(repo);
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgSign=false',...args],{cwd,stdio:['pipe','pipe','pipe']});
const hash=(bytes:Buffer)=>createHash('sha256').update(bytes).digest('hex');
const context={dataDir:data,workspaceId:'workspace-fixture',sessionId:'session-fixture',executionPath:repo};
const source=async()=>({index:hash(await readFile(join(repo,'.git','index'))),head:String(git(repo,'rev-parse','HEAD')),layer:await readFile(join(repo,'layer')),ignored:await readFile(join(repo,'.env')),flags:git(repo,'ls-files','-v','-z')});
try{
 git(repo,'init','--template=');
 for(const [name,text]of Object.entries({layer:'A',crlf:'HEAD-A\n',old:'old-A',deleted:'delete-A','disk-deleted':'disk-A',executable:'executable-A','.gitignore':'.env\nignored/\n'}))await writeFile(join(repo,name),text);
 await writeFile(join(repo,'binary'),Buffer.from([0,1,255]));await symlink('layer',join(repo,'link'));await chmod(join(repo,'executable'),0o755);
 git(repo,'add','.');git(repo,'commit','-m','HEAD A');const head=String(git(repo,'rev-parse','HEAD')).trim();
 git(repo,'config','core.autocrlf','true');await writeFile(join(repo,'crlf'),'index-B\r\n');git(repo,'add','crlf');await writeFile(join(repo,'crlf'),'disk-C\r\n');
 await writeFile(join(repo,'layer'),'B');git(repo,'add','layer');await writeFile(join(repo,'layer'),'C');
 git(repo,'mv','old','new');await writeFile(join(repo,'new'),'rename-B');git(repo,'add','new');await writeFile(join(repo,'new'),'rename-C');git(repo,'rm','deleted');await rm(join(repo,'disk-deleted'));
 await writeFile(join(repo,'binary'),Buffer.from([0,2,254,10]));await chmod(join(repo,'executable'),0o640);
 await rm(join(repo,'link'));await symlink('../outside-file',join(repo,'link'));await writeFile(join(directory,'outside-file'),'OUTSIDE_MUST_NOT_READ');
 await writeFile(join(repo,'untracked'),'U');await writeFile(join(repo,'ユニコード\nname'),'unicode');await writeFile(join(repo,'.env'),'IGNORED_TEST_SECRET_MUST_NOT_CAPTURE');await mkdir(join(repo,'ignored'));await writeFile(join(repo,'ignored','artifact'),'ignored artifact');
 const before=await source(),objectsBefore=await readdir(join(repo,'.git','objects'));
 const module=new CodeCheckpoints(context),preview=await module.preview();
 assert.equal(preview.canCapture,true,JSON.stringify(preview.unsupported));assert.equal((await module.preview()).version,preview.version);assert.equal(preview.indexFingerprint,before.index);
 assert.deepEqual(preview.exclusions,['.env','ignored/']);assert.equal(preview.coverage.ignored,'excluded-preserved-at-source');assert.equal(preview.coverage.index,'separate');
 const requestId=randomUUID();const [one,two]=await Promise.all([module.capture({expectedVersion:preview.version,requestId}),module.capture({expectedVersion:preview.version,requestId})]);
 assert.equal(one.checkpointId,two.checkpointId);const ready=await module.get(requestId);assert.equal(ready!.state,'ready');
 const exported=await module.export(ready!.checkpointId);
 const content=(kind:'head'|'index'|'disk',path:string)=>exported[kind].find(value=>value.entry.path===path)?.data;
 assert.equal(content('head','crlf')!.toString(),'HEAD-A\n');assert.equal(content('index','crlf')!.toString(),'index-B\n');assert.equal(content('disk','crlf')!.toString(),'disk-C\r\n');
 assert.equal(content('head','layer')!.toString(),'A');assert.equal(content('index','layer')!.toString(),'B');assert.equal(content('disk','layer')!.toString(),'C');
 assert.ok(content('head','old'));assert.equal(content('index','old'),undefined);assert.equal(content('index','new')!.toString(),'rename-B');assert.equal(content('disk','new')!.toString(),'rename-C');
 assert.ok(content('head','deleted'));assert.equal(content('index','deleted'),undefined);assert.equal(content('disk','deleted'),undefined);
 assert.ok(content('index','disk-deleted'));assert.equal(content('disk','disk-deleted'),undefined);assert.equal(content('disk','untracked')!.toString(),'U');assert.equal(content('head','untracked'),undefined);
 assert.deepEqual(content('disk','binary'),Buffer.from([0,2,254,10]));assert.equal(content('disk','link')!.toString(),'../outside-file');assert.equal(exported.disk.find(value=>value.entry.path==='executable')!.entry.permissions,0o640);
 assert.equal(exported.manifest.preview.workspace.headOid,head);assert.equal(hash(exported.rawIndex!),before.index);
 for(const value of [...exported.head,...exported.index,...exported.disk]){assert.notEqual(value.entry.path,'.env');assert.equal(value.data.includes(Buffer.from('IGNORED_TEST_SECRET')),false);assert.equal(value.data.includes(Buffer.from('OUTSIDE_MUST_NOT_READ')),false);}
 assert.deepEqual(await source(),before);assert.deepEqual(await readdir(join(repo,'.git','objects')),objectsBefore);
 // Controlled fixture consumer: real pre-created Git worktree; no product target manager/writer is claimed.
 const target=join(directory,'target');git(repo,'worktree','add','--detach',target,head);await writeFile(join(target,'.env'),'KEEP_TARGET_IGNORED');
 git(target,'read-tree','--empty');
 const indexInput=Buffer.from(exported.index.map(value=>`${value.entry.mode} ${value.entry.gitOid}\t${value.entry.path}\0`).join(''));
 execFileSync('git',['-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','update-index','-z','--index-info'],{cwd:target,input:indexInput});
 for(const path of new Set([...exported.manifest.head,...exported.manifest.index].map(entry=>entry.path)))await rm(join(target,path),{force:true});
 for(const {entry,data:bytes}of exported.disk){await mkdir(dirname(join(target,entry.path)),{recursive:true});if(entry.kind==='symlink')await symlink(bytes,join(target,entry.path));else{await writeFile(join(target,entry.path),bytes);await chmod(join(target,entry.path),entry.permissions??0o644);}}
 assert.equal(String(git(target,'rev-parse','HEAD')).trim(),head);assert.equal(git(target,'show',':layer').toString(),'B');assert.equal(await readFile(join(target,'layer'),'utf8'),'C');assert.equal(await readlink(join(target,'link')),'../outside-file');assert.equal((await stat(join(target,'executable'))).mode&0o777,0o640);
 assert.equal(await readFile(join(target,'.env'),'utf8'),'KEEP_TARGET_IGNORED');assert.deepEqual(git(target,'ls-files','--stage','-z'),git(repo,'ls-files','--stage','-z'));assert.deepEqual(await source(),before);
 // Changing ignored rules cannot make an old checkpoint delete, copy or re-read an ignored source file.
 await writeFile(join(repo,'.gitignore'),'ignored/\n');const ignoredBefore=await readFile(join(repo,'.env'));
 assert.deepEqual((await module.export(ready!.checkpointId)).manifest.preview.exclusions,['.env','ignored/']);assert.deepEqual(await readFile(join(repo,'.env')),ignoredBefore);
 await rename(repo,repo+'-missing');
 const cold=new CodeCheckpoints(context);const same=await cold.capture({expectedVersion:preview.version,requestId});assert.equal(same.state,'ready');assert.equal(same.checkpointId,ready!.checkpointId);assert.equal((await cold.export(same.checkpointId)).disk.find(value=>value.entry.path==='layer')!.data.toString(),'C');
 await assert.rejects(cold.capture({expectedVersion:'f'.repeat(64),requestId}),error=>(error as {code:string}).code==='REQUEST_CONFLICT');await rename(repo+'-missing',repo);
 const current=await module.preview();await writeFile(join(repo,'layer'),'external change');const staleId=randomUUID();await assert.rejects(module.capture({expectedVersion:current.version,requestId:staleId}),error=>(error as {code:string}).code==='STALE_SOURCE');assert.equal((await module.get(staleId))!.state,'failed');assert.equal((await module.capture({expectedVersion:current.version,requestId:staleId})).state,'failed');assert.equal(await readFile(join(repo,'layer'),'utf8'),'external change');
 console.log('CODE_CHECKPOINTS_OK: real HEAD A/index B/disk C, staged rename/delete, disk delete, binary/Unicode/symlink/modes, untracked+explicit ignored exclusions, unchanged source index/flags/HEAD/objects, exact controlled target reconstruction, cold source-missing idempotency and stale failure. Export module only; zero model/GUI.');
}finally{await rm(directory,{recursive:true,force:true});}
