import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,writeFile,readFile,rm,symlink,readlink,chmod,stat} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {CodeCheckpoints} from '../src/code-checkpoints.js';
import {createForkWorktree,validateManagedWorktree,recoverManagedWorktree} from '../src/git-worktrees.js';
import {materializeCheckpoint} from '../src/checkpoint-materialize.js';
const dir=await mkdtemp(join(tmpdir(),'hive-materialize-')), repo=join(dir,'repo'), data=join(dir,'data');await mkdir(repo);
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','user.name=Test','-c','user.email=test@example.test','-c','core.hooksPath=/dev/null',...args],{cwd}).toString().trim();
try {
 git(repo,'init','--template=');await writeFile(join(repo,'file'),'A');await writeFile(join(repo,'.gitignore'),'.env\n');await symlink('file',join(repo,'link'));git(repo,'add','.');git(repo,'commit','-m','A');
 await writeFile(join(repo,'file'),'B');git(repo,'add','file');await writeFile(join(repo,'file'),'C');await writeFile(join(repo,'extra'),'extra');await chmod(join(repo,'extra'),0o751);await writeFile(join(repo,'.env'),'secret');
 const checkpoints=new CodeCheckpoints({dataDir:data,workspaceId:'w',sessionId:'s',executionPath:repo});const preview=await checkpoints.preview();const op=await checkpoints.capture({expectedVersion:preview.version,requestId:randomUUID()});const checkpoint=await checkpoints.export(op.checkpointId);
 const operationId=randomUUID(),record=await createForkWorktree({projectRoot:repo,dataRoot:data,chatId:'child',baseOid:preview.workspace.headOid!,operationId,creation:{version:1,workspaceId:'w',backend:'codex',identityState:'new',forkOperationId:operationId}});
 await assert.rejects(validateManagedWorktree(data,record),(error: any)=>error.code==='FORK_PENDING');await assert.rejects(recoverManagedWorktree(data,record),(error: any)=>error.code==='FORK_PENDING');
 await materializeCheckpoint({dataRoot:data,record,operationId,checkpoint});
 assert.equal(git(record.executionPath,'show',':file'),'B');assert.equal(await readFile(join(record.executionPath,'file'),'utf8'),'C');assert.equal(await readlink(join(record.executionPath,'link')),'file');assert.equal((await stat(join(record.executionPath,'extra'))).mode&0o777,0o751);await assert.rejects(readFile(join(record.executionPath,'.env')));assert.equal(await readFile(join(repo,'file'),'utf8'),'C');assert.equal(await readFile(join(repo,'.env'),'utf8'),'secret');
 console.log('CHECKPOINT_MATERIALIZE_OK');
} finally {await rm(dir,{recursive:true,force:true});}
