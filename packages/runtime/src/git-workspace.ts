import { execFile } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, open, realpath, readlink, mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, isAbsolute, join, relative, resolve, sep } from 'node:path';

const NULL_FILE = process.platform === 'win32' ? 'NUL' : '/dev/null';
const MAX_OUTPUT = 8 * 1024 * 1024;
const MAX_INDEX = 64 * 1024 * 1024;
const MAX_FILE = 4 * 1024 * 1024;
export class GitWorkspaceError extends Error {
  constructor(public readonly code: string, message: string) { super(message); this.name = 'GitWorkspaceError'; }
}
export interface GitReadOptions { gitExecutable?: string; timeoutMs?: number }
export interface GitStatusOptions extends GitReadOptions { offset?: number; limit?: number }
export interface GitWorkspaceIdentity {
  projectRoot: string; repositoryRoot: string; projectPrefix: string;
  gitDir: string; commonDir: string; linkedWorktree: boolean;
  branch: string | null; headOid: string | null; detached: boolean; unborn: boolean;
}
export interface GitChange {
  /** Both paths preserve Git's spelling, including Unicode and newline characters. */
  path: string; repositoryPath: string; originalPath?: string; originalRepositoryPath?: string;
  /** A rename leaving this project is selectable by its source path. */
  pathRole?: 'target' | 'rename-source';
  kind: 'ordinary' | 'rename' | 'unmerged' | 'untracked';
  indexStatus: string; worktreeStatus: string; submodule?: string;
  headMode?: string; indexMode?: string; worktreeMode?: string;
  headOid?: string; indexOid?: string; renameScore?: string;
  conflictStages?: { stage: number; mode: string; oid: string }[];
}
export interface GitWorkspaceStatus extends GitWorkspaceIdentity {
  entries: GitChange[]; total: number; offset: number; limit: number; hasMore: boolean;
  complete: boolean; warnings: string[];
  /** Repository-wide index changes from the same status scan, including unresolved conflicts. */
  repositoryStagedTotal: number;
  /** Submodule commit pointers are included; nested file changes and ahead/behind are not scanned. */
  scope: 'project'; submoduleDetails: 'not-scanned'; globalConfiguration: 'read-only';
}
export interface GitFileSnapshot {
  kind: 'missing' | 'text' | 'binary' | 'symlink' | 'submodule';
  bytes: number; mode?: string; oid?: string; sha256?: string; text?: string;
}
export interface GitDiffOptions extends GitReadOptions {
  comparison?: 'staged' | 'unstaged' | 'head-worktree'; maxFileBytes?: number; maxDiffBytes?: number;
}
export interface GitFileDiff extends GitWorkspaceIdentity {
  path: string; repositoryPath: string; comparison: 'staged' | 'unstaged' | 'head-worktree';
  /** Project-relative names for each comparison side, including staged rename origins. */
  beforePath: string; afterPath: string;
  before: GitFileSnapshot; after: GitFileSnapshot;
  /** Content-only patch with neutral before/after names; modes and kinds are above. */
  patch?: string;
  binary: boolean; complete: boolean; limitations: string[];
  conflictStages?: { stage: number; snapshot: GitFileSnapshot }[];
  /** Blob bytes and disk bytes, without clean/smudge, textconv, or external diff. */
  content: 'raw';
}
type CommandResult = { stdout: Buffer; stderr: string; code: number };
type Context = GitWorkspaceIdentity & { git: string; timeout: number; env: NodeJS.ProcessEnv; config: Buffer; indexFingerprint: string; configFingerprint: string };
const safeConfig = ['-c','core.fsmonitor=false','-c',`core.hooksPath=${NULL_FILE}`,'-c','core.untrackedCache=false','-c','diff.external=','-c','submodule.recurse=false','-c','status.submoduleSummary=false','-c','protocol.allow=never','-c','maintenance.auto=false','-c','gc.auto=0'];
function integer(value: number | undefined, fallback: number, min: number, max: number, field: string): number {
  const selected = value ?? fallback;
  if (!Number.isSafeInteger(selected) || selected < min || selected > max) throw new GitWorkspaceError('INVALID_ARGUMENT', `${field} must be an integer from ${min} to ${max}`);
  return selected;
}
function environment(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  const configuration = { GIT_CONFIG_GLOBAL: env.GIT_CONFIG_GLOBAL, GIT_CONFIG_SYSTEM: env.GIT_CONFIG_SYSTEM, GIT_CONFIG_NOSYSTEM: env.GIT_CONFIG_NOSYSTEM };
  // A desktop process may itself have been launched from another Git command.
  for (const key of Object.keys(env)) if (key.startsWith('GIT_')) delete env[key];
  return { ...env, ...configuration, LC_ALL:'C',
    GIT_OPTIONAL_LOCKS:'0', GIT_TERMINAL_PROMPT:'0', GIT_NO_LAZY_FETCH:'1', GIT_ALLOW_PROTOCOL:'', GIT_PROTOCOL_FROM_USER:'0', GIT_LITERAL_PATHSPECS:'1', GIT_PAGER:'cat', PAGER:'cat' };
}
async function command(git: string, cwd: string, env: NodeJS.ProcessEnv, timeout: number, args: string[], maxBuffer = MAX_OUTPUT, accepted = [0], input?: Buffer): Promise<CommandResult> {
  return new Promise((resolveResult, reject) => {
    const child = execFile(git, ['--no-pager', ...safeConfig, ...args], {cwd,env,timeout,maxBuffer,encoding:'buffer',windowsHide:true}, (error,stdout,stderr) => {
      const detail = stderr.toString('utf8').slice(0, 1500).replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
      const code = error && typeof error.code === 'number' ? error.code : 0;
      if (error && (typeof error.code !== 'number' || !accepted.includes(error.code))) {
        const fault = error as NodeJS.ErrnoException & { killed?: boolean };
        const kind = fault.code === 'ENOENT' ? 'GIT_NOT_FOUND' : fault.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER' ? 'OUTPUT_LIMIT' : fault.killed ? 'GIT_TIMEOUT' : /not a git repository/.test(detail) ? 'NOT_REPOSITORY' : 'GIT_COMMAND_FAILED';
        reject(new GitWorkspaceError(kind, kind === 'GIT_NOT_FOUND' ? 'Git executable was not found' : `Git read failed${detail ? `: ${detail}` : ''}`));
      } else resolveResult({stdout,stderr:detail,code});
    });
    child.stdin?.on('error', error => { if ((error as NodeJS.ErrnoException).code !== 'EPIPE') reject(new GitWorkspaceError('GIT_COMMAND_FAILED','Could not supply paths to Git')); });
    child.stdin?.end(input);
  });
}
function stripLine(value: Buffer): string { return value.toString('utf8').replace(/\n$/, ''); }
function inside(root: string, path: string): boolean { const r = relative(root,path); return r === '' || (r !== '..' && !r.startsWith(`..${sep}`) && !isAbsolute(r)); }
function projectPath(repositoryPath: string, prefix: string): string | undefined {
  return prefix ? repositoryPath.startsWith(prefix) ? repositoryPath.slice(prefix.length) : undefined : repositoryPath;
}
function validatePath(path: string): void {
  if (!path || path.includes('\0') || isAbsolute(path) || /^[A-Za-z]:/.test(path)
    || path.split(process.platform === 'win32' ? /[\\/]/ : /\//).some(part => !part || part === '.' || part === '..' || part.toLowerCase() === '.git')) {
    throw new GitWorkspaceError('INVALID_PATH', 'Use a project-relative file path outside Git metadata');
  }
}
async function boundedFile(path: string, max: number): Promise<Buffer> {
  const handle = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const before = await handle.stat();
    if (!before.isFile()) throw new GitWorkspaceError('UNSUPPORTED_FILE', 'Expected a regular file');
    if (before.size > max) throw new GitWorkspaceError('FILE_LIMIT', `File exceeds the ${max} byte limit`);
    const data = Buffer.alloc(Math.min(max + 1,before.size + 1)); let length = 0;
    while (length < data.length) { const read = await handle.read(data,length,data.length-length,null); if (!read.bytesRead) break; length += read.bytesRead; }
    const after = await handle.stat();
    if (length > max) throw new GitWorkspaceError('FILE_LIMIT', `File exceeds the ${max} byte limit`);
    if (before.size !== after.size || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs || length !== before.size) throw new GitWorkspaceError('CONCURRENT_CHANGE','File changed while being read; refresh and retry');
    return data.subarray(0,length);
  } finally { await handle.close(); }
}
async function fingerprint(path: string): Promise<string> {
  try { return createHash('sha256').update(await boundedFile(path,MAX_INDEX)).digest('hex'); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return 'missing'; throw error; }
}
async function context(projectRoot: string, options: GitReadOptions): Promise<Context> {
  const project = await realpath(projectRoot);
  if (!(await lstat(project)).isDirectory()) throw new GitWorkspaceError('INVALID_PROJECT','Project root must be a directory');
  const git = options.gitExecutable ?? 'git';
  if (typeof git !== 'string' || !git || git.includes('\0')) throw new GitWorkspaceError('INVALID_ARGUMENT','Invalid Git executable');
  const timeout = integer(options.timeoutMs,8000,100,30000,'timeoutMs'), env = environment();
  const run = (args: string[], accepted = [0]) => command(git,project,env,timeout,args,MAX_OUTPUT,accepted);
  if (stripLine((await run(['rev-parse','--is-inside-work-tree'])).stdout) !== 'true') throw new GitWorkspaceError('NOT_WORKTREE','The project is not in a Git working tree');
  const repositoryRoot = await realpath(stripLine((await run(['rev-parse','--show-toplevel'])).stdout));
  if (!inside(repositoryRoot,project)) throw new GitWorkspaceError('INVALID_PROJECT','Project is outside its Git working tree');
  const gitDir = await realpath(resolve(project,stripLine((await run(['rev-parse','--git-dir'])).stdout)));
  const commonDir = await realpath(resolve(project,stripLine((await run(['rev-parse','--git-common-dir'])).stdout)));
  const branchResult = await run(['symbolic-ref','--quiet','HEAD'],[0,1]);
  const branch = branchResult.code === 0 ? stripLine(branchResult.stdout).replace(/^refs\/heads\//,'') : null;
  const head = await run(['rev-parse','--verify','--quiet','HEAD'],[0,1]);
  const headOid = head.code === 0 ? stripLine(head.stdout) : null;
  if (headOid !== null && !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(headOid)) throw new GitWorkspaceError('INVALID_REPOSITORY','Git returned an invalid HEAD object');
  if (!headOid && !branch) throw new GitWorkspaceError('INVALID_REPOSITORY','Detached HEAD is missing');
  const config = (await run(['config','--null','--list','--show-origin'])).stdout;
  const indexPath = resolve(project,stripLine((await run(['rev-parse','--git-path','index'])).stdout));
  const prefix = relative(repositoryRoot,project).split(sep).join('/');
  return {projectRoot:project,repositoryRoot,projectPrefix:prefix ? prefix + '/' : '',gitDir,commonDir,linkedWorktree:gitDir!==commonDir,branch,headOid,detached:branch===null,unborn:headOid===null,
    git,timeout,env,config,indexFingerprint:await fingerprint(indexPath),configFingerprint:createHash('sha256').update(config).digest('hex')};
}
function identity(c: Context): GitWorkspaceIdentity { const {projectRoot,repositoryRoot,projectPrefix,gitDir,commonDir,linkedWorktree,branch,headOid,detached,unborn}=c; return {projectRoot,repositoryRoot,projectPrefix,gitDir,commonDir,linkedWorktree,branch,headOid,detached,unborn}; }
async function unchanged(c: Context): Promise<void> {
  const next = await context(c.projectRoot,{gitExecutable:c.git,timeoutMs:c.timeout});
  if (JSON.stringify(identity(c))!==JSON.stringify(identity(next)) || c.indexFingerprint!==next.indexFingerprint || c.configFingerprint!==next.configFingerprint) throw new GitWorkspaceError('CONCURRENT_CHANGE','Repository HEAD, index, configuration or worktree changed; refresh and retry');
}
async function requireSafeStatus(c: Context): Promise<void> {
  // Only clean/process filters can execute while status hashes tracked files.
  // Installed but unused drivers (for example a global LFS setup) are harmless.
  const configured = await command(c.git,c.repositoryRoot,c.env,c.timeout,['config','--null','--get-regexp','^filter\\..*\\.(clean|process)$'],256*1024,[0,1]);
  const effective = new Map<string,string>();
  for (const entry of configured.stdout.toString('utf8').split('\0').filter(Boolean)) {
    const at=entry.indexOf('\n'),key=entry.slice(0,at);
    if(at<0||!/^filter\..+\.(clean|process)$/.test(key))throw new GitWorkspaceError('UNSUPPORTED_FILTER','Git filter configuration cannot be safely classified');
    effective.set(key,entry.slice(at+1)); // Git uses the last configured value.
  }
  const drivers = new Set([...effective].filter(([,value])=>value.trim()).map(([key])=>key.slice('filter.'.length,key.lastIndexOf('.'))));
  if(!drivers.size)return;
  // Status scans the entire index before results are restricted to a project,
  // so classify the whole index too. NUL-separated stdin preserves all paths.
  const tracked = await command(c.git,c.repositoryRoot,c.env,c.timeout,['ls-files','--cached','-z']);
  if(!tracked.stdout.length)return;
  const attributes = await command(c.git,c.repositoryRoot,c.env,c.timeout,['check-attr','--stdin','-z','filter'],MAX_OUTPUT,[0],tracked.stdout);
  const values=pathText(attributes.stdout).split('\0');
  if(values.pop()!==''||values.length%3!==0)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Incomplete Git filter attribute result');
  for(let i=0;i<values.length;i+=3){
    if(values[i+1]!=='filter')throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Unexpected Git filter attribute');
    if(drivers.has(values[i+2]!))throw new GitWorkspaceError('UNSUPPORTED_FILTER','Git status is unavailable because a tracked file uses a configured external content filter. Raw file diffs remain available.');
  }
}

function fields(record: string, count: number): [string[],string] {
  const result: string[]=[];let at=0;
  for(let n=0;n<count;n++){const next=record.indexOf(' ',at);if(next<0)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Incomplete Git status record');result.push(record.slice(at,next));at=next+1;}
  return [result,record.slice(at)];
}
function pathText(data: Buffer): string {
  try { return new TextDecoder('utf-8', {fatal:true,ignoreBOM:true}).decode(data); }
  catch { throw new GitWorkspaceError('UNSUPPORTED_PATH_ENCODING','Git filenames are not valid UTF-8; these paths cannot be displayed without ambiguity'); }
}
function parseStatus(data: Buffer, prefix: string): {entries: GitChange[]; repositoryStagedTotal: number} {
  const records=pathText(data).split('\0'); const entries: GitChange[]=[];let repositoryStagedTotal=0;
  if(records.pop()!=='')throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Git status did not end with a NUL separator');
  for(let i=0;i<records.length;i++){
    const record=records[i]!;const kind=record[0];
    if(kind==='#')continue;
    if(kind==='?') {const repositoryPath=record.slice(2),path=projectPath(repositoryPath,prefix);if(path!==undefined)entries.push({kind:'untracked',path,repositoryPath,indexStatus:'?',worktreeStatus:'?'});continue;}
    if(kind==='!')continue;
    if(kind!=='1'&&kind!=='2'&&kind!=='u')throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Unsupported Git status record');
    const [f,repositoryPath]=fields(record,kind==='1'?8:kind==='2'?9:10);const path=projectPath(repositoryPath,prefix);
    const originalRepositoryPath=kind==='2'?records[++i]:undefined;
    if(kind==='2'&&originalRepositoryPath===undefined)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Missing Git rename source');
    const originalPath=originalRepositoryPath===undefined?undefined:projectPath(originalRepositoryPath,prefix);
    if(!['',' ','.','?'].includes(f[1]![0]!))repositoryStagedTotal++;
    if(path===undefined&&originalPath===undefined)continue;
    const entry: GitChange={kind:kind==='1'?'ordinary':kind==='2'?'rename':'unmerged',path:path??originalPath!,repositoryPath,...(kind==='2'&&{pathRole:path===undefined?'rename-source' as const:'target' as const}),indexStatus:f[1]![0]!,worktreeStatus:f[1]![1]!,submodule:f[2]!,...(kind!=='u'&&{headMode:f[3]!,indexMode:f[4]!}),worktreeMode:f[kind==='u'?6:5]!};
    if(kind==='u')entry.conflictStages=[1,2,3].map(stage=>({stage,mode:f[stage+2]!,oid:f[stage+6]!}));
    else {entry.headOid=f[6]!;entry.indexOid=f[7]!;}
    if(originalRepositoryPath!==undefined){entry.originalRepositoryPath=originalRepositoryPath;if(originalPath!==undefined)entry.originalPath=originalPath;entry.renameScore=f[8]!;}
    entries.push(entry);
  }
  return {entries,repositoryStagedTotal};
}
/** Isolate executable configuration while retaining the actual read-only index.
 * Attributes can change at any time. With no filter definitions in this private
 * config, even a newly applied/newly configured driver cannot execute here. */
async function safeStatus(c: Context): Promise<CommandResult> {
  const directory=await mkdtemp(join(tmpdir(),'hive-git-status-'));
  try {
    await mkdir(join(directory,'objects'));await mkdir(join(directory,'refs'));await mkdir(join(directory,'info'));
    const gitPath=async(name:string)=>resolve(c.repositoryRoot,stripLine((await command(c.git,c.repositoryRoot,c.env,c.timeout,['rev-parse','--git-path',name],4096)).stdout));
    const index=await gitPath('index'),objects=await gitPath('objects');
    const env={...c.env,GIT_DIR:directory,GIT_WORK_TREE:c.repositoryRoot,GIT_INDEX_FILE:index,GIT_OBJECT_DIRECTORY:objects,GIT_CONFIG_GLOBAL:NULL_FILE,GIT_CONFIG_SYSTEM:NULL_FILE,GIT_CONFIG_NOSYSTEM:'1'};
    await writeFile(join(directory,'HEAD'),c.headOid ? c.headOid+'\n':'ref: refs/heads/desktop-unborn\n');
    const configPath=join(directory,'config');await writeFile(configPath,'');
    // These scalar/path settings affect normal index/worktree comparisons and
    // ignore rules. Never copy includes, remotes, credentials, or helper commands.
    const pattern='^(core\\.(repositoryformatversion|filemode|ignorecase|precomposeunicode|symlinks|ignorestat|trustctime|checkstat|autocrlf|eol|safecrlf|excludesfile|attributesfile|sparsecheckout|sparsecheckoutcone|longpaths)|extensions\\.objectformat|index\\.sparse|diff\\.renamelimit|status\\.renamelimit)$';
    const configured=await command(c.git,c.repositoryRoot,c.env,c.timeout,['config','--null','--get-regexp',pattern],256*1024,[0,1]);
    const effective=new Map<string,string>();
    for(const entry of configured.stdout.toString('utf8').split('\0').filter(Boolean)){const at=entry.indexOf('\n');if(at<0)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Invalid status configuration');effective.set(entry.slice(0,at),entry.slice(at+1));}
    for(const [key,value] of effective)await command(c.git,c.repositoryRoot,env,c.timeout,['config','--file',configPath,key,value],4096);
    for(const name of ['info/exclude','info/attributes','info/sparse-checkout']){
      try {await writeFile(join(directory,...name.split('/')),await boundedFile(await gitPath(name),1024*1024));}
      catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;}
    }
    const shared=stripLine((await command(c.git,c.repositoryRoot,c.env,c.timeout,['rev-parse','--shared-index-path'],4096)).stdout);
    if(shared){const name=basename(shared);if(!/^sharedindex\.[a-f0-9]+$/.test(name))throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Invalid shared index path');await writeFile(join(directory,name),await boundedFile(resolve(c.repositoryRoot,shared),MAX_INDEX));}
    return await command(c.git,c.repositoryRoot,env,c.timeout,['-c','status.renames=true','status','--porcelain=v2','-z','--untracked-files=all','--ignore-submodules=dirty','--no-ahead-behind','--','.']);
  }finally{await rm(directory,{recursive:true,force:true});}
}
/** Optimistic version for explicit local index operations; no write is performed. */
export interface GitWorkspaceVersion extends GitWorkspaceIdentity {
  indexPath: string; indexFingerprint: string; configFingerprint: string;
}
export async function getGitWorkspaceVersion(projectRoot: string, options: GitReadOptions = {}): Promise<GitWorkspaceVersion> {
  const c=await context(projectRoot,options);
  const indexPath=resolve(c.repositoryRoot,stripLine((await command(c.git,c.repositoryRoot,c.env,c.timeout,['rev-parse','--git-path','index'],4096)).stdout));
  return {...identity(c),indexPath,indexFingerprint:c.indexFingerprint,configFingerprint:c.configFingerprint};
}
export async function getGitWorkspaceStatus(projectRoot: string, options: GitStatusOptions = {}): Promise<GitWorkspaceStatus> {
  const offset=integer(options.offset,0,0,1_000_000,'offset'),limit=integer(options.limit,100,1,500,'limit');
  const c=await context(projectRoot,options);await requireSafeStatus(c);
  const output=await safeStatus(c);
  // Reclassify the real attributes after the isolated scan. A newly active
  // driver is an explicit unsupported result, never a raw-content status claim.
  await requireSafeStatus(c);
  const {entries,repositoryStagedTotal}=parseStatus(output.stdout,c.projectPrefix);await unchanged(c);
  const warnings=output.stderr?[output.stderr]:[];
  return {...identity(c),entries:entries.slice(offset,offset+limit),total:entries.length,offset,limit,hasMore:offset+limit<entries.length,complete:warnings.length===0,warnings,repositoryStagedTotal,scope:'project',submoduleDetails:'not-scanned',globalConfiguration:'read-only'};
}

type Snapshot = { meta: GitFileSnapshot; data: Buffer };
const missing=():Snapshot=>({meta:{kind:'missing',bytes:0},data:Buffer.alloc(0)});
function snapshot(data: Buffer, mode?: string, oid?: string): Snapshot {
  let text: string|undefined;
  if(!data.includes(0)){try{text=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(data);}catch{}}
  const kind=mode==='120000'?'symlink':text===undefined?'binary':'text';
  return {data,meta:{kind,bytes:data.length,sha256:createHash('sha256').update(data).digest('hex'),...(mode&&{mode}),...(oid&&{oid}),...(text!==undefined&&{text})}};
}
async function blob(c: Context, oid: string, mode: string, max: number): Promise<Snapshot> {
  if(/^0+$/.test(oid))return missing();
  if(!['100644','100755','120000','160000'].includes(mode))throw new GitWorkspaceError('UNSUPPORTED_FILE','Git entry is not a regular file, symbolic link or submodule');
  if(mode==='160000')return {meta:{kind:'submodule',bytes:0,mode,oid},data:Buffer.alloc(0)};
  if(!/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(oid))throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Invalid Git blob object');
  const size=Number(stripLine((await command(c.git,c.repositoryRoot,c.env,c.timeout,['cat-file','-s',oid],1024)).stdout));
  if(!Number.isSafeInteger(size)||size<0)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Invalid Git blob size');
  if(size>max)throw new GitWorkspaceError('FILE_LIMIT',`Git blob exceeds the ${max} byte limit`);
  return snapshot((await command(c.git,c.repositoryRoot,c.env,c.timeout,['cat-file','blob',oid],max+1)).stdout,mode,oid);
}
async function indexStages(c: Context, path: string, max: number): Promise<{stage:number;snapshot:Snapshot}[]> {
  const output=await command(c.git,c.repositoryRoot,c.env,c.timeout,['ls-files','--stage','-z','--',path]);
  const result:{stage:number;snapshot:Snapshot}[]=[];
  for(const record of pathText(output.stdout).split('\0').filter(Boolean)){
    const tab=record.indexOf('\t');const [mode,oid,stage]=record.slice(0,tab).split(' ');
    if(tab<0||record.slice(tab+1)!==path||!mode||!oid||!stage)throw new GitWorkspaceError('INVALID_GIT_OUTPUT','Invalid Git index entry');
    result.push({stage:Number(stage),snapshot:await blob(c,oid,mode,max)});
  }
  return result;
}
async function headSnapshot(c: Context, path: string, max: number): Promise<Snapshot> {
  if(!c.headOid)return missing();
  const result=await command(c.git,c.repositoryRoot,c.env,c.timeout,['ls-tree','-z',c.headOid,'--',path]);
  for(const record of pathText(result.stdout).split('\0').filter(Boolean)){
    const tab=record.indexOf('\t');if(record.slice(tab+1)!==path)continue;
    const [mode,,oid]=record.slice(0,tab).split(' ');if(mode&&oid)return blob(c,oid,mode,max);
  }
  return missing();
}
async function diskSnapshot(c: Context, path: string, max: number): Promise<Snapshot> {
  let current=c.projectRoot;const parts=path.split('/');
  try {
    for(let i=0;i<parts.length;i++){
      current=join(current,parts[i]!);const info=await lstat(current);
      if(i<parts.length-1){if(!info.isDirectory()||info.isSymbolicLink())throw new GitWorkspaceError('UNSAFE_PATH','File parent is not a regular project directory');continue;}
      if(info.isSymbolicLink()){const target=Buffer.from(await readlink(current));if(target.length>max)throw new GitWorkspaceError('FILE_LIMIT',`Symbolic link exceeds the ${max} byte limit`);return snapshot(target,'120000');}
      if(!info.isFile())throw new GitWorkspaceError('UNSUPPORTED_FILE','Diff supports regular files and symbolic links; directories and submodules are separate Git entries');
      const canonical=await realpath(current);if(!inside(c.projectRoot,canonical))throw new GitWorkspaceError('UNSAFE_PATH','File is outside the project');
      const data=await boundedFile(current,max);const after=await lstat(current);
      if(after.dev!==info.dev||after.ino!==info.ino||after.size!==info.size||after.mtimeMs!==info.mtimeMs||await realpath(current)!==canonical)throw new GitWorkspaceError('CONCURRENT_CHANGE','Working file changed while being read');
      return snapshot(data,info.mode&0o111?'100755':'100644');
    }
  }catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return missing();throw error;}
  return missing();
}
async function stagedSource(c: Context,path: string): Promise<string> {
  if(!c.headOid)return path;
  const output=await command(c.git,c.repositoryRoot,c.env,c.timeout,['diff','--cached','--name-status','-z','--find-renames','--no-ext-diff','--no-textconv','--ignore-submodules=dirty',c.headOid,'--']);
  const tokens=pathText(output.stdout).split('\0');
  for(let i=0;i<tokens.length-1;){const status=tokens[i++]!,first=tokens[i++]!;if(status.startsWith('R')||status.startsWith('C')){const second=tokens[i++]!;if(second===path)return first;if(first===path){if(projectPath(second,c.projectPrefix)===undefined)throw new GitWorkspaceError('OUTSIDE_PROJECT_RENAME','The rename destination is outside the selected project; inspect it from the repository root');throw new GitWorkspaceError('RENAMED_PATH','This path was renamed; request the destination path from current Git status');}}}
  return path;
}
async function patch(c: Context,before: Snapshot,after: Snapshot,max: number): Promise<string> {
  const directory=await mkdtemp(join(tmpdir(),'hive-git-diff-'));
  try {
    await writeFile(join(directory,'before'),before.data);await writeFile(join(directory,'after'),after.data);
    // Outside the project, with metadata discovery explicitly disabled. No
    // project/global config, attributes, diff drivers, or filters enter here.
    const env={...c.env,GIT_CEILING_DIRECTORIES:directory,GIT_CONFIG_COUNT:'0',GIT_CONFIG_GLOBAL:NULL_FILE,GIT_CONFIG_SYSTEM:NULL_FILE,GIT_CONFIG_NOSYSTEM:'1'};
    const output=await command(c.git,directory,env,c.timeout,['diff','--no-index','--no-ext-diff','--no-textconv','--no-color','--no-renames','--src-prefix=a/','--dst-prefix=b/','--','before','after'],max,[0,1]);
    return output.stdout.toString('utf8');
  }finally{await rm(directory,{recursive:true,force:true});}
}
export async function getGitFileDiff(projectRoot: string,path: string,options: GitDiffOptions = {}): Promise<GitFileDiff> {
  validatePath(path);
  const max=integer(options.maxFileBytes,1024*1024,1,MAX_FILE,'maxFileBytes'),diffMax=integer(options.maxDiffBytes,1024*1024,1,MAX_OUTPUT,'maxDiffBytes');
  const comparison=options.comparison??'unstaged';if(!['staged','unstaged','head-worktree'].includes(comparison))throw new GitWorkspaceError('INVALID_ARGUMENT','Invalid Git comparison');
  const c=await context(projectRoot,options),repositoryPath=c.projectPrefix+path;
  const stages=await indexStages(c,repositoryPath,max),conflicts=stages.filter(stage=>stage.stage!==0);
  const index=stages.find(stage=>stage.stage===0)?.snapshot??missing();
  const limitations:string[]=[];
  let headPath=repositoryPath;
  if(comparison!=='unstaged')headPath=await stagedSource(c,repositoryPath);
  if(projectPath(headPath,c.projectPrefix)===undefined)throw new GitWorkspaceError('OUTSIDE_PROJECT_RENAME','The rename source is outside the selected project; inspect it from the repository root');
  const before=comparison==='unstaged'?index:await headSnapshot(c,headPath,max);
  let after:Snapshot;
  if(comparison==='staged')after=index;
  else if(index.meta.kind==='submodule'){after=index;limitations.push('Submodule working directories are not scanned');}
  else after=await diskSnapshot(c,path,max);
  if(conflicts.length)limitations.push('Unmerged index has no stage-zero file; inspect the three conflict stages and working copy');
  if(before.meta.kind==='submodule'||after.meta.kind==='submodule')limitations.push('Submodule changes are commit identifiers, not file content');
  const binary=before.meta.kind==='binary'||after.meta.kind==='binary';
  const output=!binary&&!limitations.length?await patch(c,before,after,diffMax):undefined;
  await unchanged(c);
  return {...identity(c),path,repositoryPath,comparison,beforePath:comparison==='unstaged'?path:projectPath(headPath,c.projectPrefix)!,afterPath:path,before:before.meta,after:after.meta,...(output!==undefined&&{patch:output}),binary,complete:limitations.length===0,limitations,content:'raw',...(conflicts.length&&{conflictStages:conflicts.map(stage=>({stage:stage.stage,snapshot:stage.snapshot.meta}))})};
}
