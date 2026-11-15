import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, mkdir, open, readdir, readFile, realpath, rename, symlink, unlink } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { checkpointDiskEntry, checkpointHash, checkpointPath, CodeCheckpointError } from './checkpoint-files.js';
import type { ExportedCodeCheckpoint } from './code-checkpoints.js';
import { validateForkWorktree, type ManagedWorktree } from './git-worktrees.js';

function fail(message: string): never { throw new CodeCheckpointError('MATERIALIZE_CONFLICT', message); }
async function runGit(cwd: string, index: string, args: string[], input?: Buffer): Promise<void> {
  const env = {...process.env}; for (const key of Object.keys(env)) if (key.toUpperCase().startsWith('GIT_')) delete env[key];
  Object.assign(env, {GIT_INDEX_FILE:index, GIT_CONFIG_NOSYSTEM:'1', GIT_CONFIG_GLOBAL:process.platform === 'win32' ? 'NUL' : '/dev/null', GIT_TERMINAL_PROMPT:'0', GIT_NO_LAZY_FETCH:'1', LC_ALL:'C'});
  await new Promise<void>((resolve, reject) => {
    const child=spawn('git',['-c','core.hooksPath='+ (process.platform === 'win32' ? 'NUL' : '/dev/null'),'-c','core.fsmonitor=false',...args],{cwd,env,stdio:['pipe','ignore','pipe']});
    let error=''; child.stderr.on('data',chunk=>{error+=chunk.toString().slice(0,4096);}); child.on('error',reject); child.on('close',code=>code===0?resolve():reject(new Error(`Git index reconstruction failed: ${error}`))); child.stdin.on('error',()=>{}); child.stdin.end(input);
  });
}
async function entries(root: string): Promise<string[]> {
  const result: string[]=[];
  async function walk(dir: string, prefix: string) { for (const name of await readdir(dir)) { if (!prefix && name === '.git') continue; const path=prefix ? `${prefix}/${name}` : name; const st=await lstat(join(dir,name)); if(st.isDirectory()&&!st.isSymbolicLink()) await walk(join(dir,name),path); else result.push(path); } }
  await walk(root,''); return result.sort();
}
/** Materialize only into a private, operation-owned, unpublished worktree. Never modifies the source. */
export async function materializeCheckpoint(input: {dataRoot:string; record:ManagedWorktree; operationId:string; checkpoint:ExportedCodeCheckpoint}): Promise<void> {
  const {dataRoot,record,operationId,checkpoint}=input;
  await validateForkWorktree(dataRoot,record,operationId);
  if(record.projectPrefix || checkpoint.manifest.preview.workspace.headOid!==record.baseOid || checkpoint.manifest.preview.workspace.commonDir!==record.repoCommonDir) fail('Checkpoint repository, scope, or HEAD does not match the fork');
  const root=record.managedWorktreeRoot, gitDir=record.gitDir!;
  if(await realpath(root)!==root || await realpath(gitDir)!==gitDir) fail('Fork directories changed');
  const journal=join(dirname(root),'materialize.json');
  // Exclusive claim makes an interrupted copy diagnosable, never silently overwritten.
  const claim=await open(journal,'wx',0o600); try {await claim.writeFile(JSON.stringify({version:1,operationId,checkpointId:checkpoint.manifest.checkpointId,state:'applying'}));await claim.sync();} finally {await claim.close();}
  if((await entries(root)).length) fail('Fork contains outside changes');
  const indexPath=join(gitDir,'index'), lockPath=indexPath+'.lock';
  const lock=await open(lockPath,'wx',0o600);
  try {
    try {await lstat(indexPath); fail('Fork index already exists');} catch(error) {if((error as NodeJS.ErrnoException).code!=='ENOENT') throw error;}
    const temp=join(gitDir,`hive-index-${operationId}`);
    const indexRecords:Buffer[]=[];
    for(const {entry,data} of checkpoint.index) {
      checkpointPath(entry.path); if(!entry.gitOid || checkpointHash(data)!==entry.sha256 || data.length!==entry.bytes) fail('Checkpoint index content is invalid');
      indexRecords.push(Buffer.from(`${entry.mode} ${entry.gitOid}\t${entry.path}\0`));
    }
    await runGit(root,temp,['read-tree','--empty']);
    await runGit(root,temp,['update-index','-z','--index-info'],Buffer.concat(indexRecords));
    const index=await readFile(temp); await unlink(temp); await lock.writeFile(index); await lock.sync();
    for(const {entry,data} of checkpoint.disk) {
      checkpointPath(entry.path); if(checkpointHash(data)!==entry.sha256 || data.length!==entry.bytes) fail('Checkpoint disk content is invalid');
      const parts=entry.path.split('/'); let parent=root;
      for(const part of parts.slice(0,-1)) { parent=join(parent,part); try {await mkdir(parent,{mode:0o700});} catch(error) {if((error as NodeJS.ErrnoException).code!=='EEXIST') throw error;} const st=await lstat(parent); if(!st.isDirectory()||st.isSymbolicLink()) fail('Fork path was replaced'); }
      const path=join(root,entry.path);
      if(entry.kind==='symlink') await symlink(data,path);
      else {const file=await open(path,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|(constants.O_NOFOLLOW??0),entry.permissions??(entry.mode==='100755'?0o755:0o644)); try {await file.writeFile(data);await file.chmod(entry.permissions??(entry.mode==='100755'?0o755:0o644));await file.sync();} finally {await file.close();}}
    }
    if(JSON.stringify(await entries(root))!==JSON.stringify(checkpoint.disk.map(x=>x.entry.path).sort())) fail('Fork changed during materialization');
    for (const {entry} of checkpoint.disk) {
      const observed=await checkpointDiskEntry(root,entry.path,entry.bytes);
      if (!observed || observed.entry.sha256!==entry.sha256 || observed.entry.kind!==entry.kind || observed.entry.permissions!==entry.permissions) fail('Fork file changed during materialization');
    }
    await validateForkWorktree(dataRoot,record,operationId);
    try {await lstat(indexPath); fail('Fork index changed during materialization');} catch(error) {if((error as NodeJS.ErrnoException).code!=='ENOENT') throw error;}
    await lock.close(); await rename(lockPath,indexPath);
    const done=await open(journal,'w',0o600); try {await done.writeFile(JSON.stringify({version:1,operationId,checkpointId:checkpoint.manifest.checkpointId,state:'ready'}));await done.sync();} finally {await done.close();}
  } catch(error) {await lock.close().catch(()=>{}); await unlink(lockPath).catch(()=>{}); throw error;}
}
