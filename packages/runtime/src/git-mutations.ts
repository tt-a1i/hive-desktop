import { execFile, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { constants } from 'node:fs';
import { lstat, open, realpath, readlink, mkdtemp, writeFile, rm, rename, access, utimes } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { getGitWorkspaceVersion, GitWorkspaceError, type GitReadOptions, type GitWorkspaceVersion } from './git-workspace.js';

const OUTPUT_LIMIT=8*1024*1024,INDEX_LIMIT=64*1024*1024,FILE_LIMIT=32*1024*1024;
const NULL_FILE=process.platform==='win32'?'NUL':'/dev/null';
export class GitMutationError extends GitWorkspaceError {}
export interface GitSelectedVersion {
  path: string; repositoryPath: string; kind: 'missing'|'file'|'symlink'|'submodule'|'directory';
  sha256: string | null; bytes: number; executable: boolean;
}
export interface GitIndexPreview {
  operation: 'stage'|'unstage'; version: string; workspace: GitWorkspaceVersion;
  /** Exact approved paths; known staged renames explicitly include both sides. */
  paths: string[]; files: GitSelectedVersion[]; contentFilters: boolean;
}
export interface GitIndexRequest { paths: readonly string[]; expectedVersion: string }
export interface GitIndexResult { ok: true; operation: 'stage'|'unstage'; paths: string[]; workspace: GitWorkspaceVersion }
function error(code:string,message:string):GitMutationError{return new GitMutationError(code,message);}
function hash(data:Buffer|string):string{return createHash('sha256').update(data).digest('hex');}
function validatePaths(paths:readonly string[]):string[]{
  if(!Array.isArray(paths)||!paths.length||paths.length>200)throw error('INVALID_SELECTION','Select between 1 and 200 individual file paths');
  for(const path of paths)if(typeof path!=='string'||!path||Buffer.byteLength(path)>4096||path.includes('\0')||isAbsolute(path)||/^[A-Za-z]:/.test(path)||path.split(process.platform==='win32'?/[\\/]/:/\//).some(part=>!part||part==='.'||part==='..'||part.toLowerCase()==='.git'))throw error('INVALID_SELECTION','Select project-relative files outside Git metadata');
  return [...new Set(paths)].sort();
}
function inside(root:string,path:string):boolean{const r=relative(root,path);return r===''||(r!=='..'&&!r.startsWith(`..${sep}`)&&!isAbsolute(r));}
function environment(indexPath?:string):NodeJS.ProcessEnv{
  const env={...process.env};const config={GIT_CONFIG_GLOBAL:env.GIT_CONFIG_GLOBAL,GIT_CONFIG_SYSTEM:env.GIT_CONFIG_SYSTEM,GIT_CONFIG_NOSYSTEM:env.GIT_CONFIG_NOSYSTEM};
  for(const key of Object.keys(env))if(key.startsWith('GIT_'))delete env[key];
  return {...env,...config,LC_ALL:'C',GIT_TERMINAL_PROMPT:'0',GIT_NO_LAZY_FETCH:'1',GIT_ALLOW_PROTOCOL:'',GIT_PROTOCOL_FROM_USER:'0',GIT_OPTIONAL_LOCKS:'0',GIT_LITERAL_PATHSPECS:'1',GIT_PAGER:'cat',...(indexPath&&{GIT_INDEX_FILE:indexPath})};
}
async function git(workspace:GitWorkspaceVersion,args:string[],options:GitReadOptions,input?:Buffer,indexPath?:string,preserveHooks=false,accepted=[0]):Promise<Buffer>{
  const executable=options.gitExecutable??'git',timeout=options.timeoutMs??8000;
  if(!Number.isSafeInteger(timeout)||timeout<100||timeout>30000)throw error('INVALID_ARGUMENT','timeoutMs must be between 100 and 30000');
  return new Promise((resolveResult,reject)=>{
    let timedOut=false,limitExceeded=false,outputBytes=0,startFault:NodeJS.ErrnoException|undefined;
    let timer:ReturnType<typeof setTimeout>|undefined,escalation:ReturnType<typeof setTimeout>|undefined;
    const stdout:Buffer[]=[],stderr:Buffer[]=[];
    const child=spawn(executable,['--no-pager','-c','core.fsmonitor=false',...(preserveHooks?[]:['-c',`core.hooksPath=${NULL_FILE}`]),'-c','maintenance.auto=false','-c','gc.auto=0',...(indexPath?['-c','core.splitIndex=false','-c','splitIndex.sharedIndexExpire=never']:[]),...args],{cwd:workspace.repositoryRoot,env:environment(indexPath),detached:process.platform!=='win32',windowsHide:true,stdio:'pipe'});
    const stop=(signal:NodeJS.Signals)=>{
      if(!child.pid)return;
      if(process.platform==='win32'){execFile('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true},()=>{});return;}
      try{process.kill(-child.pid,signal);}catch{child.kill(signal);}
    };
    const terminate=()=>{stop('SIGTERM');clearTimeout(escalation);escalation=setTimeout(()=>stop('SIGKILL'),250);escalation.unref();};
    const output=(parts:Buffer[],data:Buffer)=>{const remaining=OUTPUT_LIMIT-outputBytes;outputBytes+=data.length;if(remaining>0)parts.push(data.subarray(0,remaining));if(outputBytes>OUTPUT_LIMIT&&!limitExceeded){limitExceeded=true;terminate();}};
    child.stdout.on('data',(data:Buffer)=>output(stdout,data));child.stderr.on('data',(data:Buffer)=>output(stderr,data));child.once('error',fault=>{startFault=fault;});
    child.once('close',code=>{
      clearTimeout(timer);clearTimeout(escalation);
      if(!startFault&&!timedOut&&!limitExceeded&&code!==null&&accepted.includes(code)){resolveResult(Buffer.concat(stdout));return;}
      const detail=Buffer.concat(stderr).toString('utf8').slice(0,1500).replace(/[\x00-\x08\x0b-\x1f\x7f]/g,'');
      const kind=timedOut?'GIT_TIMEOUT':limitExceeded?'OUTPUT_LIMIT':startFault?.code==='ENOENT'?'GIT_NOT_FOUND':'GIT_OPERATION_FAILED';
      reject(error(kind,`Git operation ${timedOut?'timed out':'failed'}${detail?`: ${detail}`:''}`));
    });
    timer=setTimeout(()=>{timedOut=true;terminate();},timeout);timer.unref();
    child.stdin.on('error',fault=>{if((fault as NodeJS.ErrnoException).code!=='EPIPE'){startFault=fault;terminate();}});child.stdin.end(input);
  });
}
async function indexBytes(path:string):Promise<Buffer|undefined>{
  let handle;try{handle=await open(path,constants.O_RDONLY|(constants.O_NOFOLLOW??0));}catch(fault){if((fault as NodeJS.ErrnoException).code==='ENOENT')return;throw fault;}
  try{const info=await handle.stat();if(!info.isFile()||info.size>INDEX_LIMIT)throw error('INDEX_LIMIT','Git index is not a regular file within the 64 MiB limit');const data=await handle.readFile();const after=await handle.stat();if(data.length!==info.size||data.length>INDEX_LIMIT||info.size!==after.size||info.mtimeMs!==after.mtimeMs||info.ctimeMs!==after.ctimeMs)throw error('CONCURRENT_CHANGE','Index changed while being read');return data;}finally{await handle.close();}
}
async function preparedIndex(path:string,bytes:Buffer,source:string):Promise<void>{
  const metadata=await lstat(source);await writeFile(path,bytes);
  // Copying an index must not advance its racy-stat baseline and hide a
  // same-size, same-second edit. An earlier timestamp forces Git's normal check.
  const seconds=Math.max(1,Math.floor(metadata.mtimeMs/1000)-1);await utimes(path,seconds,seconds);
}
function pathList(paths:readonly string[]):Buffer{return Buffer.from(paths.join('\0')+'\0');}
function text(data:Buffer):string{try{return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(data);}catch{throw error('UNSUPPORTED_PATH_ENCODING','Git paths must be valid UTF-8');}}
async function renamePairs(workspace:GitWorkspaceVersion,options:GitReadOptions):Promise<[string,string][]> {
  if(!workspace.headOid)return [];
  const fields=text(await git(workspace,['diff','--cached','--name-status','-z','--find-renames','--no-ext-diff','--no-textconv','--ignore-submodules=dirty',workspace.headOid,'--'],options)).split('\0');const pairs:[string,string][]=[];
  for(let i=0;i<fields.length-1;){const status=fields[i++]!,first=fields[i++]!;if(status.startsWith('R')||status.startsWith('C')){const second=fields[i++]!;if(status.startsWith('R'))pairs.push([first,second]);}}
  return pairs;
}
async function trackedModes(workspace:GitWorkspaceVersion,options:GitReadOptions):Promise<Map<string,string>>{
  const entries=text(await git(workspace,['ls-files','--stage','-z'],options)).split('\0').filter(Boolean);const modes=new Map<string,string>();
  for(const entry of entries){const tab=entry.indexOf('\t');if(tab<0)throw error('INVALID_GIT_OUTPUT','Invalid index entry');modes.set(entry.slice(tab+1),entry.slice(0,6));}return modes;
}
async function selectedVersion(workspace:GitWorkspaceVersion,path:string,modes:Map<string,string>,options:GitReadOptions,allowDirectory=false):Promise<GitSelectedVersion>{
  const repositoryPath=workspace.projectPrefix+path;const base={path,repositoryPath,sha256:null,bytes:0,executable:false};let current=workspace.projectRoot;const parts=path.split('/');
  try{
    for(let i=0;i<parts.length;i++){
      current=join(current,parts[i]!);const info=await lstat(current);const final=i===parts.length-1;
      if(!final){if(info.isSymbolicLink()||!info.isDirectory())throw error('UNSAFE_PATH','Selected file parent is not a project directory');continue;}
      if(info.isSymbolicLink()){const target=Buffer.from(await readlink(current));return{...base,kind:'symlink',sha256:hash(target),bytes:target.length};}
      if(info.isDirectory()){
        if(allowDirectory)return{...base,kind:'directory'};
        if(modes.get(repositoryPath)!=='160000')throw error('DIRECTORY_SELECTION','Select individual files; directories are not staged recursively');
        const root=text(await git(workspace,['-C',current,'rev-parse','--show-toplevel'],options)).replace(/\n$/,'');if(await realpath(root)!==await realpath(current))throw error('SUBMODULE_NOT_INITIALIZED','Initialize this submodule before staging its commit pointer');
        const oid=text(await git(workspace,['-C',current,'rev-parse','--verify','HEAD'],options)).trim();return{...base,kind:'submodule',sha256:hash(oid)};
      }
      if(!info.isFile()||info.size>FILE_LIMIT)throw error('FILE_LIMIT','Selected files must be regular files within 32 MiB');
      const canonical=await realpath(current);if(!inside(workspace.projectRoot,canonical))throw error('UNSAFE_PATH','Selected file is outside the project');
      const handle=await open(current,constants.O_RDONLY|(constants.O_NOFOLLOW??0));
      try{const before=await handle.stat();const digest=createHash('sha256');let bytes=0;const buffer=Buffer.alloc(64*1024);for(;;){const read=await handle.read(buffer,0,buffer.length,null);if(!read.bytesRead)break;bytes+=read.bytesRead;if(bytes>FILE_LIMIT)throw error('FILE_LIMIT','Selected file grew beyond 32 MiB');digest.update(buffer.subarray(0,read.bytesRead));}const after=await handle.stat();const target=await lstat(current);if(before.dev!==info.dev||before.ino!==info.ino||before.size!==after.size||before.mtimeMs!==after.mtimeMs||before.ctimeMs!==after.ctimeMs||target.ino!==before.ino||target.dev!==before.dev||await realpath(current)!==canonical)throw error('CONCURRENT_CHANGE','Selected file changed while being read');return{...base,kind:'file',sha256:digest.digest('hex'),bytes,executable:!!(info.mode&0o111)};}finally{await handle.close();}
    }
  }catch(fault){if((fault as NodeJS.ErrnoException).code==='ENOENT')return{...base,kind:'missing'};throw fault;}
  return{...base,kind:'missing'};
}
async function filtersUsed(workspace:GitWorkspaceVersion,paths:readonly string[],options:GitReadOptions):Promise<boolean>{
  const attributes=text(await git(workspace,['check-attr','--stdin','-z','filter'],options,pathList(paths))).split('\0');
  const data=text(await git(workspace,['config','--null','--list'],options)).split('\0');const effective=new Map<string,string>();
  for(const entry of data){const at=entry.indexOf('\n');if(at>=0)effective.set(entry.slice(0,at),entry.slice(at+1));}
  for(let i=0;i<attributes.length-1;i+=3){const driver=attributes[i+2]!;if(effective.get(`filter.${driver}.clean`)?.trim()||effective.get(`filter.${driver}.process`)?.trim())return true;}
  return false;
}

async function preview(operation:'stage'|'unstage',projectRoot:string,input:readonly string[],options:GitReadOptions):Promise<GitIndexPreview>{
  const selected=validatePaths(input),workspace=await getGitWorkspaceVersion(projectRoot,options);const repositoryPaths=new Set(selected.map(path=>workspace.projectPrefix+path));
  for(const [from,to] of await renamePairs(workspace,options))if(repositoryPaths.has(from)||repositoryPaths.has(to)){if(!from.startsWith(workspace.projectPrefix)||!to.startsWith(workspace.projectPrefix))throw error('CROSS_PROJECT_RENAME','Select the repository root to change both sides of this rename');repositoryPaths.add(from);repositoryPaths.add(to);}
  const paths=[...repositoryPaths].map(path=>path.slice(workspace.projectPrefix.length)).sort();validatePaths(paths);
  const modes=await trackedModes(workspace,options),files:GitSelectedVersion[]=[];
  for(const path of paths){
    const repositoryPath=workspace.projectPrefix+path;
    const inHead=workspace.headOid?text(await git(workspace,['ls-tree','-z',workspace.headOid,'--',repositoryPath],options)).split('\0').some(entry=>entry.slice(entry.indexOf('\t')+1)===repositoryPath&&/^(100644|100755|120000|160000) /.test(entry)):false;
    // Unstage selects the index/HEAD file identity. A directory now occupying
    // that filename is preserved, never traversed or mistaken for a folder selection.
    const file=await selectedVersion(workspace,path,modes,options,operation==='unstage'&&(modes.has(repositoryPath)||!!inHead));
    if(file.kind==='missing'&&!modes.has(repositoryPath)&&!inHead)throw error('PATH_NOT_FOUND','Selected path does not exist in the working tree, index, or HEAD');files.push(file);
  }
  const contentFilters=operation==='stage'&&await filtersUsed(workspace,[...repositoryPaths],options);
  const current=await getGitWorkspaceVersion(projectRoot,options);if(JSON.stringify(current)!==JSON.stringify(workspace))throw error('CONCURRENT_CHANGE','Repository changed while preparing the preview');
  const version=hash(JSON.stringify({operation,workspace,files,contentFilters}));return{operation,version,workspace,paths,files,contentFilters};
}
export function previewGitStage(projectRoot:string,paths:readonly string[],options:GitReadOptions={}):Promise<GitIndexPreview>{return preview('stage',projectRoot,paths,options);}
export function previewGitUnstage(projectRoot:string,paths:readonly string[],options:GitReadOptions={}):Promise<GitIndexPreview>{return preview('unstage',projectRoot,paths,options);}
async function mutate(operation:'stage'|'unstage',projectRoot:string,request:GitIndexRequest,options:GitReadOptions):Promise<GitIndexResult>{
  if(typeof request.expectedVersion!=='string'||!/^[a-f0-9]{64}$/.test(request.expectedVersion))throw error('INVALID_VERSION','Refresh the preview before changing the index');
  const workspace=await getGitWorkspaceVersion(projectRoot,options);let lock;const lockPath=workspace.indexPath+'.lock';
  try{lock=await open(lockPath,'wx',0o666);}catch(fault){if((fault as NodeJS.ErrnoException).code==='EEXIST')throw error('INDEX_BUSY','Another Git operation holds index.lock; wait for it to finish');throw fault;}
  let directory:string|undefined,installed=false;
  const owner=await lock.stat();
  const ownsLock=async()=>{try{const current=await lstat(lockPath);return current.dev===owner.dev&&current.ino===owner.ino;}catch(fault){if((fault as NodeJS.ErrnoException).code==='ENOENT')return false;throw fault;}};
  try{
    const approved=await preview(operation,projectRoot,request.paths,options);if(approved.version!==request.expectedVersion)throw error('STALE_PREVIEW','HEAD, index, configuration or selected files changed; refresh the preview');
    directory=await mkdtemp(join(dirname(workspace.indexPath),'hive-desktop-index-'));const prepared=join(directory,'index');const original=await indexBytes(workspace.indexPath);if(original)await preparedIndex(prepared,original,workspace.indexPath);
    const paths=approved.files.filter(file=>operation==='unstage'||file.kind!=='missing').map(file=>file.repositoryPath);
    const modes=await trackedModes(approved.workspace,options);if(operation==='stage')for(const file of approved.files)if(file.kind==='missing'&&modes.has(file.repositoryPath))paths.push(file.repositoryPath);
    if(operation==='stage'&&paths.length)await git(approved.workspace,['add','--all','--pathspec-from-file=-','--pathspec-file-nul'],options,pathList(paths),prepared);
    if(operation==='unstage'){
      const all=approved.files.map(file=>file.repositoryPath);
      if(approved.workspace.headOid)await git(approved.workspace,['reset','--quiet',approved.workspace.headOid,'--pathspec-from-file=-','--pathspec-file-nul'],options,pathList(all),prepared);
      else await git(approved.workspace,['update-index','--force-remove','-z','--stdin'],options,pathList(all),prepared);
    }
    const after=await preview(operation,projectRoot,request.paths,options);if(after.version!==approved.version)throw error('CONCURRENT_CHANGE','Repository or selected files changed during the Git operation; no index change was installed');
    const result=await indexBytes(prepared);if(!result)throw error('GIT_OPERATION_FAILED','Git did not produce a prepared index');
    if((original?hash(original):'missing')!==(approved.workspace.indexFingerprint))throw error('CONCURRENT_CHANGE','Index changed before preparation');
    await lock.writeFile(result);await lock.chmod((await lstat(prepared)).mode&0o777);await lock.sync();await lock.close();lock=undefined;
    // Atomic Git-style installation. Never restore a prior index over another writer.
    if(!await ownsLock())throw error('LOCK_LOST','The Git index lock was replaced; refresh before retrying');
    const stillCurrent=await indexBytes(workspace.indexPath);if((stillCurrent?hash(stillCurrent):'missing')!==approved.workspace.indexFingerprint)throw error('CONCURRENT_CHANGE','Another tool changed the actual index; its index was preserved');
    await rename(lockPath,workspace.indexPath);installed=true;
    try{return{ok:true,operation,paths:approved.paths,workspace:await getGitWorkspaceVersion(projectRoot,options)};}
    catch(fault){throw Object.assign(error('INDEX_UPDATED_REFRESH_FAILED','The index was updated, but its new state could not be read; refresh before another action'),{applied:true,operation,paths:approved.paths,cause:fault});}
  }finally{await lock?.close();if(!installed&&await ownsLock())await rm(lockPath,{force:true});if(directory)await rm(directory,{recursive:true,force:true});}
}
export function stageGitPaths(projectRoot:string,request:GitIndexRequest,options:GitReadOptions={}):Promise<GitIndexResult>{return mutate('stage',projectRoot,request,options);}
export function unstageGitPaths(projectRoot:string,request:GitIndexRequest,options:GitReadOptions={}):Promise<GitIndexResult>{return mutate('unstage',projectRoot,request,options);}

export interface GitStagedChange { repositoryPath:string; projectPath:string|null; status:string; beforeMode:string; afterMode:string; beforeOid:string; afterOid:string; originalRepositoryPath?:string }
export interface GitCommitPreview {
  version:string; workspace:GitWorkspaceVersion; changes:GitStagedChange[]; patch:string;
  outsideProjectPaths:string[]; signingRequired:boolean; hooks:string[];
  /** Native hooks are trusted project programs and may edit files/index/message. */
  hooksMayModifyIndex:true;
}
export interface GitCommitRequest { expectedVersion:string; message:string; includeOutsideProject?:boolean }
export interface GitCommitResult {
  ok:boolean; committed:true|'unknown'; applied:true|'unknown'; commitOid:string|null; observedHeadOid?:string|null; indexUpdated:boolean;
  workspace?:GitWorkspaceVersion; warnings:string[]; error?:{code:string;message:string};
}
async function noActiveOperation(workspace:GitWorkspaceVersion,options:GitReadOptions):Promise<void>{
  for(const name of ['MERGE_HEAD','CHERRY_PICK_HEAD','REVERT_HEAD','rebase-merge','rebase-apply','sequencer']){
    const path=resolve(workspace.repositoryRoot,text(await git(workspace,['rev-parse','--git-path',name],options)).replace(/\n$/,''));
    try{await lstat(path);throw error('REPOSITORY_OPERATION_ACTIVE','Finish the existing merge, rebase, cherry-pick or revert before making a new plain commit');}catch(fault){if((fault as NodeJS.ErrnoException).code!=='ENOENT')throw fault;}
  }
}
async function stagedChanges(workspace:GitWorkspaceVersion,options:GitReadOptions):Promise<GitStagedChange[]>{
  const args=['diff','--cached','--raw','-z','--no-abbrev','--find-renames','--no-ext-diff','--no-textconv','--ignore-submodules=dirty',...(workspace.headOid?[workspace.headOid]:[]),'--'];
  const tokens=text(await git(workspace,args,options)).split('\0'),result:GitStagedChange[]=[];
  for(let i=0;i<tokens.length-1;){const fields=tokens[i++]!.split(' '),first=tokens[i++]!;if(fields.length!==5||!fields[0]!.startsWith(':'))throw error('INVALID_GIT_OUTPUT','Invalid staged change record');const status=fields[4]!;const target=status.startsWith('R')||status.startsWith('C')?tokens[i++]!:first;
    if(!target)throw error('INVALID_GIT_OUTPUT','Missing staged path');result.push({repositoryPath:target,projectPath:target.startsWith(workspace.projectPrefix)?target.slice(workspace.projectPrefix.length):null,status,beforeMode:fields[0]!.slice(1),afterMode:fields[1]!,beforeOid:fields[2]!,afterOid:fields[3]!,...(target!==first&&{originalRepositoryPath:first})});}
  return result;
}
export async function previewGitCommit(projectRoot:string,options:GitReadOptions={},request:{includePatch?:boolean;expectedVersion?:string}={}):Promise<GitCommitPreview>{
  const workspace=await getGitWorkspaceVersion(projectRoot,options);await noActiveOperation(workspace,options);
  const index=text(await git(workspace,['ls-files','--stage','-z'],options)).split('\0').filter(Boolean);
  if(index.some(entry=>entry.slice(0,entry.indexOf('\t')).split(' ')[2]!=='0'))throw error('UNMERGED_INDEX','Resolve and stage all conflicts before committing');
  const changes=await stagedChanges(workspace,options);if(!changes.length)throw error('NOTHING_STAGED','There are no staged changes to commit');
  const outsideProjectPaths=[...new Set(changes.flatMap(change=>[change.repositoryPath,...(change.originalRepositoryPath?[change.originalRepositoryPath]:[])].filter(path=>!path.startsWith(workspace.projectPrefix))))].sort();
  const signingRequired=text(await git(workspace,['config','--bool','--get','commit.gpgSign'],options,undefined,undefined,true,[0,1])).trim()==='true';
  const hooks:string[]=[];for(const name of ['pre-commit','prepare-commit-msg','commit-msg','post-commit']){const path=resolve(workspace.repositoryRoot,text(await git(workspace,['rev-parse','--git-path',`hooks/${name}`],options,undefined,undefined,true)).replace(/\n$/,''));try{const hook=await lstat(path);if(hook.isFile()||hook.isSymbolicLink()){await access(path,constants.X_OK);hooks.push(name);}}catch(fault){if(!['ENOENT','EACCES'].includes((fault as NodeJS.ErrnoException).code??''))throw fault;}}
  const version=hash(JSON.stringify({workspace,changes,signingRequired,hooks}));
  if(request.expectedVersion!==undefined&&request.expectedVersion!==version)throw error('STALE_PREVIEW','HEAD, index or commit configuration changed; refresh the complete staged preview');
  const patch=request.includePatch===false?'':text(await git(workspace,['diff','--cached','--no-ext-diff','--no-textconv','--no-color','--ignore-submodules=dirty',...(workspace.headOid?[workspace.headOid]:[]),'--'],options));
  if(JSON.stringify(await getGitWorkspaceVersion(projectRoot,options))!==JSON.stringify(workspace))throw error('CONCURRENT_CHANGE','Repository changed while preparing the complete staged preview');
  return{version,workspace,changes,patch,outsideProjectPaths,signingRequired,hooks,hooksMayModifyIndex:true};
}
function nativeFailure(fault:unknown,hooks:readonly string[]):GitMutationError{
  const detail=fault instanceof Error?fault.message:String(fault);const kind=/cannot lock ref|but expected|reference already exists/i.test(detail)?'CONCURRENT_CHANGE':fault instanceof GitMutationError&&['GIT_TIMEOUT','OUTPUT_LIMIT'].includes(fault.code)?fault.code:/Author identity unknown|unable to auto-detect|no email was given|empty ident name/i.test(detail)?'IDENTITY_REQUIRED':/gpg|failed to sign|signing failed|ssh-keygen|public key/i.test(detail)?'SIGNING_FAILED':hooks.length?'HOOK_REJECTED':'COMMIT_FAILED';
  return Object.assign(error(kind,`${detail}. The native commit did not complete; working files changed by hooks are preserved. Refresh before retrying.`),{committed:false,applied:false,cause:fault});
}
/** Calls native Git commit with its hooks, identity, signing and ref CAS intact.
 * Preflight is optimistic: the normal native process-initialization window remains. */
export async function commitGitIndex(projectRoot:string,request:GitCommitRequest,options:GitReadOptions={}):Promise<GitCommitResult>{
  if(typeof request.expectedVersion!=='string'||!/^[a-f0-9]{64}$/.test(request.expectedVersion))throw error('INVALID_VERSION','Refresh the complete staged preview before committing');
  if(typeof request.message!=='string'||!request.message.trim()||request.message.includes('\0')||Buffer.byteLength(request.message)>64*1024)throw error('INVALID_MESSAGE','Supply a nonempty commit message within 64 KiB');
  const workspace=await getGitWorkspaceVersion(projectRoot,options);const lockPath=workspace.indexPath+'.lock';let lock;
  try{lock=await open(lockPath,'wx',0o666);}catch(fault){if((fault as NodeJS.ErrnoException).code==='EEXIST')throw error('INDEX_BUSY','Another Git operation holds index.lock; wait for it to finish');throw fault;}
  const owner=await lock.stat();const ownsLock=async()=>{try{const info=await lstat(lockPath);return info.dev===owner.dev&&info.ino===owner.ino;}catch(fault){if((fault as NodeJS.ErrnoException).code==='ENOENT')return false;throw fault;}};
  let directory:string|undefined,installed=false,commitOid:string|null=null,committed=false,outcome:GitCommitResult|undefined;
  const partial=(code:string,message:string,indexUpdated=false):GitCommitResult=>(outcome={ok:false,committed:true,applied:true,commitOid,indexUpdated,warnings:['The commit exists. Do not automatically retry or reset HEAD. Refresh and inspect the repository.'],error:{code,message}});
  try{
    const approved=await previewGitCommit(projectRoot,options,{includePatch:false,expectedVersion:request.expectedVersion});
    if(approved.outsideProjectPaths.length&&request.includeOutsideProject!==true)throw error('OUTSIDE_PROJECT_CONFIRMATION','This commit includes staged paths outside the selected project; explicitly review and confirm the whole repository');
    directory=await mkdtemp(join(dirname(workspace.indexPath),'hive-desktop-commit-'));const prepared=join(directory,'index');const original=await indexBytes(workspace.indexPath);if(!original)throw error('INDEX_MISSING','The staged index is missing');await preparedIndex(prepared,original,workspace.indexPath);
    // Native Git owns the prepared index's own lock. The real index remains
    // locked so other normal Git writers cannot replace the reviewed contents.
    try{await git(approved.workspace,['commit','--file=-','--no-edit'],options,Buffer.from(request.message),prepared,true);committed=true;}
    catch(fault){
      // Git explicitly distinguishes its own post-ref index failure from a
      // rejected hook/signature/ref CAS. Never undo an already-created commit.
      if(fault instanceof Error&&/repository has been updated, but unable to write/i.test(fault.message))committed=true;
      else {
        const failure=nativeFailure(fault,approved.hooks);if(failure.code==='CONCURRENT_CHANGE')throw failure;
        let observed:string|null=null,stateReadable=true;
        try{observed=text(await git(approved.workspace,['rev-parse','--verify','--quiet','HEAD'],options,undefined,undefined,false,[0,1])).trim()||null;}catch{stateReadable=false;}
        if(!stateReadable||observed!==approved.workspace.headOid){
          committed=true; // Cleanup must not turn an uncertain write into a retryable error.
          return outcome={ok:false,committed:'unknown',applied:'unknown',commitOid:null,observedHeadOid:observed,indexUpdated:false,warnings:['Git stopped after repository state changed. A commit may already exist; do not automatically retry or reset HEAD. Refresh and inspect the repository.'],error:{code:'COMMIT_STATE_UNCERTAIN',message:failure.message}};
        }
        throw failure;
      }
    }
    try{commitOid=text(await git(approved.workspace,['rev-parse','--verify','HEAD'],options)).trim();}
    catch{return partial('COMMIT_CREATED_STATE_UNAVAILABLE','Git created the commit, but HEAD could not be read. Refresh the repository before another action.');}
    try{
      const now=await indexBytes(workspace.indexPath);if(!await ownsLock()||(now?hash(now):'missing')!==approved.workspace.indexFingerprint)return partial('COMMIT_CREATED_INDEX_CONFLICT','Git created the commit, but another tool replaced the actual index or its lock. That tool’s index and lock were preserved.');
      const result=await indexBytes(prepared);if(!result)return partial('COMMIT_CREATED_INDEX_UNAVAILABLE','Git created the commit, but its prepared index is missing. Refresh and inspect staged changes.');
      await lock.writeFile(result);await lock.chmod((await lstat(prepared)).mode&0o777);await lock.sync();await lock.close();lock=undefined;
      if(!await ownsLock())return partial('COMMIT_CREATED_INDEX_CONFLICT','Git created the commit, but the index lock was replaced. The other lock was preserved.');
      const current=await indexBytes(workspace.indexPath);if((current?hash(current):'missing')!==approved.workspace.indexFingerprint)return partial('COMMIT_CREATED_INDEX_CONFLICT','Git created the commit, but another tool changed the actual index. Its changes were preserved.');
      await rename(lockPath,workspace.indexPath);installed=true;
    }catch(fault){return partial('COMMIT_CREATED_INDEX_INSTALL_FAILED',`Git created the commit, but its index could not be installed: ${fault instanceof Error?fault.message:String(fault)}`);}
    try{return outcome={ok:true,committed:true,applied:true,commitOid,indexUpdated:true,workspace:await getGitWorkspaceVersion(projectRoot,options),warnings:approved.hooks.length?['Native commit hooks may have changed files, the message, or the index.']:[]};}
    catch{return partial('COMMIT_CREATED_REFRESH_FAILED','The commit and index were updated, but the new state could not be read. Refresh before another action.',true);}
  }catch(fault){if(committed)return partial('COMMIT_CREATED_STATE_UNAVAILABLE',fault instanceof Error?fault.message:String(fault),installed);throw fault;}
  finally{
    const failures:unknown[]=[];try{await lock?.close();}catch(fault){failures.push(fault);}
    try{if(!installed&&await ownsLock())await rm(lockPath,{force:true});}catch(fault){failures.push(fault);}
    try{if(directory)await rm(directory,{recursive:true,force:true});}catch(fault){failures.push(fault);}
    if(failures.length){if(committed&&outcome){outcome.ok=false;outcome.warnings.push('Commit created, but temporary index/lock cleanup failed. Check repository permissions and disk space before another Git action.');outcome.error??={code:'COMMIT_CREATED_CLEANUP_FAILED',message:'The commit exists; cleanup needs attention. Do not retry the commit or reset HEAD.'};}else throw error('INDEX_CLEANUP_FAILED','Git did not complete and temporary index cleanup failed; check repository permissions and index.lock before retrying');}
  }
}
