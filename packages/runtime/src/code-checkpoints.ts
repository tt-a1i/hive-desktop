import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, open, realpath, readdir, rename, link, unlink } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, resolve } from 'node:path';
import { getGitWorkspaceStatus, getGitWorkspaceVersion, GitWorkspaceError, type GitReadOptions, type GitWorkspaceVersion } from './git-workspace.js';
import { CodeCheckpointError, checkpointDiskEntry, checkpointFilesystemInventory, checkpointHash, checkpointInside, checkpointPath, readCheckpointRegular, type CheckpointEntry } from './checkpoint-files.js';
export { CodeCheckpointError } from './checkpoint-files.js';
export type { CheckpointEntry } from './checkpoint-files.js';

export interface CheckpointLimits {fileBytes: number; totalBytes: number; entries: number; indexBytes: number; manifestBytes: number}
export interface CodeCheckpointContext {dataDir: string; workspaceId: string; sessionId: string; executionPath: string}
export interface CheckpointCoverage {
  content: 'raw'; tracked: true; nonIgnoredUntracked: true; index: 'separate'; ignored: 'excluded-preserved-at-source';
  emptyDirectories: 'not-recorded'; metadata: 'bytes-symlink-target-permission-bits';
}
export interface CodeCheckpointPreview {
  version: string; workspace: GitWorkspaceVersion; headFingerprint: string; indexFingerprint: string;
  diskFingerprint: string | null; exclusions: string[]; coverage: CheckpointCoverage;
  unsupported: {code: string; message: string}[]; canCapture: boolean; limits: CheckpointLimits;
}
export interface CodeCheckpointManifest {
  /** v2 requires the Git-ignore-aware real filesystem inventory. */
  version: 2; checkpointId: string; requestId: string; workspaceId: string; sessionId: string; capturedAt: number;
  preview: CodeCheckpointPreview; head: CheckpointEntry[]; index: CheckpointEntry[]; disk: CheckpointEntry[];
  rawIndex: {sha256: string; bytes: number} | null;
}
export interface CodeCheckpointOperation {
  version: 1; requestId: string; checkpointId: string; workspaceId: string; sessionId: string; expectedVersion: string;
  state: 'capturing' | 'ready' | 'failed'; manifestSha256?: string; error?: {code: string; message: string};
}
export interface ExportedCodeCheckpoint {
  manifest: CodeCheckpointManifest;
  head: {entry: CheckpointEntry; data: Buffer}[]; index: {entry: CheckpointEntry; data: Buffer}[]; disk: {entry: CheckpointEntry; data: Buffer}[];
  /** Exact source bytes for archive/verification, not an index to install in another worktree.
   * Reconstruct the exported index entries with native Git against the captured HEAD. */
  rawIndex: Buffer | null;
}
const coverage: CheckpointCoverage = {content:'raw',tracked:true,nonIgnoredUntracked:true,index:'separate',ignored:'excluded-preserved-at-source',emptyDirectories:'not-recorded',metadata:'bytes-symlink-target-permission-bits'};
const defaults: CheckpointLimits = {fileBytes:32*1024*1024,totalBytes:128*1024*1024,entries:10000,indexBytes:64*1024*1024,manifestBytes:8*1024*1024};
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i, HASH = /^[0-9a-f]{64}$/, OID = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;
const identity = (value: unknown): value is string => typeof value === 'string' && !!value && value.length <= 256 && !/[\x00-\x1f\x7f]/.test(value);
function fail(code: string, message: string): never {throw new CodeCheckpointError(code,message);}
const text = (bytes: Buffer) => {try {return new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes);} catch {return fail('UNSUPPORTED_PATH_ENCODING','Checkpoint paths must be valid UTF-8');}};
const fields = (bytes: Buffer) => {const values=text(bytes).split('\0');if(values.pop()!=='')fail('INVALID_GIT_OUTPUT','Git path list is not terminated');return values;};
const noFile = (error: unknown) => (error as NodeJS.ErrnoException).code === 'ENOENT';
const sourceVersion = (value: GitWorkspaceVersion) => JSON.stringify(value);
function gitEnvironment(): NodeJS.ProcessEnv {
  const env = {...process.env}, config = {GIT_CONFIG_GLOBAL:env.GIT_CONFIG_GLOBAL,GIT_CONFIG_SYSTEM:env.GIT_CONFIG_SYSTEM,GIT_CONFIG_NOSYSTEM:env.GIT_CONFIG_NOSYSTEM};
  for (const key of Object.keys(env)) if (key.startsWith('GIT_')) delete env[key];
  return {...env,...config,LC_ALL:'C',GIT_OPTIONAL_LOCKS:'0',GIT_TERMINAL_PROMPT:'0',GIT_NO_LAZY_FETCH:'1',GIT_NO_REPLACE_OBJECTS:'1',GIT_ALLOW_PROTOCOL:'',GIT_PROTOCOL_FROM_USER:'0',GIT_LITERAL_PATHSPECS:'1',GIT_PAGER:'cat',PAGER:'cat'};
}
/** Only metadata/raw-object commands. Same no-helper/no-refresh boundary as git-workspace. */
function git(cwd: string, args: string[], options: GitReadOptions, maximum=8*1024*1024, accepted=[0], input?: Buffer): Promise<Buffer> {
  const nil=process.platform==='win32'?'NUL':'/dev/null';
  return new Promise((resolveResult,reject)=>{
    const child=execFile(options.gitExecutable??'git',['--no-pager','-c','core.fsmonitor=false','-c',`core.hooksPath=${nil}`,'-c','core.untrackedCache=false','-c','diff.external=','-c','submodule.recurse=false','-c','protocol.allow=never','-c','maintenance.auto=false','-c','gc.auto=0',...args],
      {cwd,env:{...gitEnvironment(),...(args[0]==='check-ignore'&&{GIT_LITERAL_PATHSPECS:'0'})},timeout:options.timeoutMs??8000,maxBuffer:maximum,encoding:'buffer',windowsHide:true},(error,stdout)=>{
        if(error && !(typeof error.code==='number'&&accepted.includes(error.code))) {
          const code=(error as NodeJS.ErrnoException).code;
          reject(new CodeCheckpointError(code==='ENOENT'?'GIT_NOT_FOUND':code==='ERR_CHILD_PROCESS_STDIO_MAXBUFFER'?'OUTPUT_LIMIT':error.killed?'GIT_TIMEOUT':'GIT_READ_FAILED','A bounded read-only Git operation failed'));return;
        }
        resolveResult(stdout);
      });
    child.stdin?.on('error',error=>{if((error as NodeJS.ErrnoException).code!=='EPIPE')reject(new CodeCheckpointError('GIT_READ_FAILED','Could not supply a raw-object Git request'));});child.stdin?.end(input);
  });
}
interface GitEntry {path:string; mode:CheckpointEntry['mode']; oid:string}
function gitEntry(mode: string, oid: string, path: string): GitEntry {
  checkpointPath(path);
  if(!OID.test(oid))fail('INVALID_GIT_OUTPUT','Invalid Git object identity');
  if(mode==='160000')fail('UNSUPPORTED_SUBMODULE','Submodule contents require a separate checkpoint scope');
  if(!['100644','100755','120000'].includes(mode))fail('UNSUPPORTED_INDEX','Sparse or non-file Git entries are not supported');
  return {path,mode:mode as GitEntry['mode'],oid};
}
function parseIndex(bytes: Buffer): GitEntry[] {
  const value=text(bytes), entries:GitEntry[]=[];let position=0;
  while(position<value.length){
    const end=value.indexOf('\0',position);if(end<0)fail('INVALID_GIT_OUTPUT','Index path record is incomplete');
    const header=/^(\d{6}) ([0-9a-f]+) ([0-3])\t([\s\S]+)$/.exec(value.slice(position,end));
    if(!header)fail('INVALID_GIT_OUTPUT','Invalid index record');
    if(header[3]!=='0')fail('UNMERGED_INDEX','Resolve unmerged index entries before creating a code checkpoint');
    const debug=/^  ctime: [^\n]*\n  mtime: [^\n]*\n  dev: [^\n]*\n  uid: [^\n]*\n  size: [^\n]*\tflags: ([0-9a-f]+)\n/.exec(value.slice(end+1));
    if(!debug)fail('UNSUPPORTED_INDEX','Git index flags could not be classified');
    const flags=BigInt('0x'+debug[1]);
    if(flags&0x20000000n)fail('INTENT_TO_ADD','Intent-to-add entries cannot yet be precisely reconstructed');
    if(flags!==0n)fail('UNSUPPORTED_INDEX_FLAGS','Assume-unchanged, skip-worktree and other non-default index flags are not supported');
    entries.push(gitEntry(header[1]!,header[2]!,header[4]!));position=end+1+debug[0].length;
  }
  return entries;
}
async function syncDirectory(path: string) {
  let handle;
  try {handle=await open(path,'r');await handle.sync();}
  catch(error){if(process.platform!=='win32'||!['EINVAL','EPERM','EISDIR','EACCES'].includes((error as NodeJS.ErrnoException).code??''))throw error;}
  finally{await handle?.close();}
}
async function prospectiveRealpath(path: string): Promise<string> {
  try {return await realpath(path);} catch(error) {
    if(!noFile(error)||dirname(path)===path)throw error;
    return join(await prospectiveRealpath(dirname(path)),basename(path));
  }
}
async function directory(path: string) {
  if(await realpath(dirname(path))!==dirname(path))fail('STORAGE_UNSAFE','Checkpoint storage parent is not canonical');
  try {await mkdir(path,{mode:0o700});await syncDirectory(dirname(path));} catch(error){if((error as NodeJS.ErrnoException).code!=='EEXIST')throw error;}
  const info=await lstat(path);if(!info.isDirectory()||info.isSymbolicLink())fail('STORAGE_UNSAFE','Checkpoint storage is not a private real directory');
}
async function jsonWrite(path: string, value: unknown, exclusive=false) {
  if(await realpath(dirname(path))!==dirname(path))fail('STORAGE_UNSAFE','Checkpoint state parent was replaced or linked');
  const tmp=`${path}.${randomUUID()}.tmp`;const fd=await open(tmp,'wx',0o600);
  try {await fd.writeFile(JSON.stringify(value));await fd.sync();}finally{await fd.close();}
  try {if(await realpath(dirname(path))!==dirname(path))fail('STORAGE_UNSAFE','Checkpoint state directory changed');if(exclusive)await link(tmp,path);else await rename(tmp,path);await syncDirectory(dirname(path));}
  finally{await unlink(tmp).catch(error=>{if(!noFile(error))throw error;});}
}
interface Scan {preview:CodeCheckpointPreview; head:CheckpointEntry[];index:CheckpointEntry[];disk:CheckpointEntry[];rawIndex:Buffer|null;blobs:Map<string,Buffer>}
/** Internal server module. The caller owns identity validation and the shared execution lease.
 * No target writer, worktree manager, native history control or Hive ledger is introduced here. */
export class CodeCheckpoints {
  readonly limits: CheckpointLimits;
  private root: string | undefined;
  constructor(private readonly context: CodeCheckpointContext, private readonly options: GitReadOptions & {limits?:Partial<CheckpointLimits>} = {}) {
    if(!identity(context.workspaceId)||!identity(context.sessionId)||typeof context.dataDir!=='string'||typeof context.executionPath!=='string'||!isAbsolute(context.dataDir)||!isAbsolute(context.executionPath))fail('INVALID_ARGUMENT','Supply validated server workspace/session identities and absolute execution/data paths');
    this.context={...context}; this.options={...options};
    this.limits=Object.freeze({...defaults,...options.limits});
    for(const key of Object.keys(defaults) as (keyof CheckpointLimits)[])if(!Number.isSafeInteger(this.limits[key])||this.limits[key]<1||this.limits[key]>defaults[key])fail('INVALID_ARGUMENT','Checkpoint limits must be positive integers within the supported bounds');
  }
  private async initialise():Promise<string> {
    if(this.root)return this.root;
    // Existing duplicates remain queryable after the execution directory disappears.
    let source:string|undefined;try{source=await realpath(this.context.executionPath);}catch(error){if(!noFile(error))throw error;}
    const data=resolve(this.context.dataDir),prospective=await prospectiveRealpath(data);
    if(checkpointInside(source??resolve(this.context.executionPath),data)||source&&checkpointInside(source,prospective))fail('INVALID_STORAGE','Checkpoint data must not be stored inside the source execution directory');
    await mkdir(data,{recursive:true,mode:0o700});const canonical=await realpath(data);
    if(source&&checkpointInside(source,canonical))fail('INVALID_STORAGE','Checkpoint data must not be stored inside the source execution directory');
    const base=join(canonical,'code-checkpoints');await directory(base);
    const scope=join(base,checkpointHash(JSON.stringify([this.context.workspaceId,this.context.sessionId])));await directory(scope);
    await directory(join(scope,'requests'));await directory(join(scope,'snapshots'));this.root=scope;return scope;
  }
  private async requestPath(requestId:string){if(typeof requestId!=='string'||!UUID.test(requestId))fail('INVALID_ARGUMENT','A UUIDv4 checkpoint request identity is required');return join(await this.initialise(),'requests',requestId+'.json');}
  private async privateRead(path:string,maximum:number):Promise<Buffer>{
    const root=await this.initialise(),parent=dirname(path);
    if(!checkpointInside(root,path)||await realpath(root)!==root||await realpath(parent)!==parent)fail('STORAGE_UNSAFE','Checkpoint storage directories were replaced or linked');
    const data=await readCheckpointRegular(path,maximum);
    if(await realpath(root)!==root||await realpath(parent)!==parent)fail('STORAGE_UNSAFE','Checkpoint storage changed during read');
    return data;
  }
  private async operation(requestId:string):Promise<CodeCheckpointOperation|undefined>{
    const file=await this.requestPath(requestId);let bytes:Buffer;try{bytes=await this.privateRead(file,16*1024);}catch(error){if(noFile(error))return;throw error;}
    try{
      const op=JSON.parse(text(bytes)) as CodeCheckpointOperation;
      if(op.version!==1||op.requestId!==requestId||!UUID.test(op.checkpointId)||op.workspaceId!==this.context.workspaceId||op.sessionId!==this.context.sessionId||!HASH.test(op.expectedVersion)||!['capturing','ready','failed'].includes(op.state)||op.state==='ready'&&!HASH.test(op.manifestSha256??''))throw 0;
      return op;
    }catch{ return fail('CORRUPT','Checkpoint request metadata is invalid; it was not recaptured'); }
  }
  async get(requestId:string):Promise<CodeCheckpointOperation|undefined>{const op=await this.operation(requestId);if(op?.state==='ready')await this.readManifest(op);return op;}
  async list():Promise<{checkpointId:string;capturedAt:number;coverage:CheckpointCoverage;exclusions:string[]}[]> {
    const root=await this.initialise(), requests=join(root,'requests');
    if(await realpath(root)!==root||await realpath(requests)!==requests)fail('STORAGE_UNSAFE','Checkpoint request directory was replaced or linked');
    const names=await readdir(requests), result:{checkpointId:string;capturedAt:number;coverage:CheckpointCoverage;exclusions:string[]}[]=[];
    for(const name of names.sort()) {
      const match=/^([0-9a-f-]+)\.json$/i.exec(name);
      if(!match||!UUID.test(match[1]!))fail('CORRUPT','Unknown checkpoint request metadata');
      const op=await this.operation(match[1]!);if(!op)fail('CORRUPT','Checkpoint request disappeared while listing');
      if(op.state!=='ready')continue;
      const manifest=await this.readManifest(op);
      result.push({checkpointId:manifest.checkpointId,capturedAt:manifest.capturedAt,coverage:structuredClone(manifest.preview.coverage),exclusions:[...manifest.preview.exclusions]});
    }
    return result.sort((a,b)=>b.capturedAt-a.capturedAt||a.checkpointId.localeCompare(b.checkpointId));
  }
  private async scan():Promise<Scan>{
    let workspace:GitWorkspaceVersion;try{workspace=await getGitWorkspaceVersion(this.context.executionPath,this.options);}catch(error){if(['ENOENT','ENOTDIR'].includes((error as NodeJS.ErrnoException).code??''))throw new CodeCheckpointError('SOURCE_UNAVAILABLE','The source execution directory is unavailable',{cause:error});throw error;}
    const empty:Scan={preview:{version:'',workspace,headFingerprint:checkpointHash(workspace.headOid??'unborn'),indexFingerprint:workspace.indexFingerprint,diskFingerprint:null,exclusions:[],coverage:{...coverage},unsupported:[],canCapture:false,limits:{...this.limits}},head:[],index:[],disk:[],rawIndex:null,blobs:new Map()};
    const finish=()=>{empty.preview.version=checkpointHash(JSON.stringify({...empty.preview,version:undefined}));return empty;};
    try{
      if(!workspace.headOid)fail('UNBORN_REPOSITORY','A code checkpoint currently requires an existing HEAD commit');
      for(const base of new Set([workspace.gitDir,workspace.commonDir]))for(const name of ['exclude','attributes']){
        try{const info=await lstat(join(base,'info',name));if(!info.isFile())fail('UNSUPPORTED_SPECIAL_FILE','Git metadata rule files must be regular files');}
        catch(error){if(!noFile(error))throw error;}
      }
      for(const name of ['core.sparseCheckout','index.sparse','core.splitIndex']){
        const configured=text(await git(workspace.projectRoot,['config','--bool','--get',name],this.options,4096,[0,1])).replace(/\n$/,'');
        if(configured==='true')fail(name==='core.splitIndex'?'SPLIT_INDEX':'SPARSE_INDEX','Sparse/split index configurations are not supported by code checkpoints');
      }
      if(text(await git(workspace.projectRoot,['rev-parse','--shared-index-path'],this.options,4096)).replace(/\n$/,''))fail('SPLIT_INDEX','Split index contents require a separately verified snapshot');
      const prefix=workspace.projectPrefix;
      const local=(path:string)=>{if(prefix&&!path.startsWith(prefix))fail('INVALID_GIT_OUTPUT','Git returned a path outside the execution project');const value=path.slice(prefix.length);checkpointPath(value);return value;};
      const index=parseIndex(await git(workspace.repositoryRoot,['ls-files','--stage','--debug','--full-name','-z'],this.options)).filter(entry=>!prefix||entry.path.startsWith(prefix)).map(entry=>({...entry,path:local(entry.path)}));
      const head=fields(await git(workspace.projectRoot,['ls-tree','-r','--full-name','-z',workspace.headOid,'--','.'],this.options)).map(record=>{
        const value=/^(\d{6}) (blob|commit) ([0-9a-f]+)\t([\s\S]+)$/.exec(record);if(!value)fail('INVALID_GIT_OUTPUT','Invalid HEAD tree entry');return gitEntry(value[1]!,value[3]!,local(value[4]!));
      });
      const filesystem = await checkpointFilesystemInventory(workspace.projectRoot, new Set([...head,...index].map(entry=>entry.path)), async paths=>{
        if(!paths.length)return [];
        const output=fields(await git(workspace.projectRoot,['check-ignore','--stdin','-z','--verbose','--non-matching','--no-index'],this.options,8*1024*1024,[0,1],Buffer.from(paths.map(path=>'./'+path).join('\0')+'\0')));
        if(output.length!==paths.length*4)fail('INVALID_GIT_OUTPUT','Git ignore classification did not cover the filesystem inventory');
        return paths.map((path,at)=>{if(output[at*4+3]!=='./'+path)fail('INVALID_GIT_OUTPUT','Git ignore classification returned a different path');const pattern=output[at*4+2]!;return !!pattern&&!pattern.startsWith('!');});
      },this.limits.entries);
      // Reuse the maintained isolated status implementation: configured filters never execute.
      let offset=0;
      do{
        const status=await getGitWorkspaceStatus(workspace.repositoryRoot,{...this.options,offset,limit:500});
        if(!status.complete)fail('INCOMPLETE_GIT_STATUS','Git status did not return complete safe results');
        if(status.total>this.limits.entries)fail('ENTRY_LIMIT','Repository status exceeds the checkpoint entry limit');
        if(workspace.projectPrefix&&status.entries.some(entry=>!entry.repositoryPath.startsWith(workspace.projectPrefix)||(entry.originalRepositoryPath!==undefined&&!entry.originalRepositoryPath.startsWith(workspace.projectPrefix))))fail('OUTSIDE_PROJECT_DIRTY','Changes outside the execution project cannot be silently carried into its checkpoint');
        offset+=status.entries.length;if(!status.hasMore)break;
      }while(true);
      const untracked=fields(await git(workspace.projectRoot,['ls-files','--others','--exclude-standard','--full-name','-z','--','.'],this.options)).map(path=>{if(path.endsWith('/'))fail('UNSUPPORTED_NESTED_REPOSITORY','An embedded repository requires a separate checkpoint scope');return local(path);});
      const ignored=fields(await git(workspace.projectRoot,['ls-files','--others','--ignored','--exclude-standard','--directory','--full-name','-z','--','.'],this.options));
      empty.preview.exclusions=ignored.map(path=>{const suffix=path.endsWith('/')?'/':'';return local(suffix?path.slice(0,-1):path)+suffix;}).filter(path=>!path.endsWith('/')||![...head,...index].some(entry=>entry.path.startsWith(path))).sort();
      const paths=[...new Set([...head,...index].map(entry=>entry.path).concat(untracked))].sort();
      const candidates=new Set(paths);
      if(filesystem.some(entry=>!entry.ignored&&['file','symlink'].includes(entry.kind)&&!candidates.has(entry.path)))
        fail('INCOMPLETE_INVENTORY','Git did not enumerate every non-ignored filesystem file');
      empty.preview.exclusions=[...new Set([...empty.preview.exclusions,...filesystem.filter(entry=>entry.ignored).map(entry=>entry.path+(entry.kind==='directory'?'/':''))])].sort();

      if(head.length+index.length+paths.length+ignored.length>this.limits.entries)fail('ENTRY_LIMIT','Checkpoint inventory exceeds its declared entry limit');
      try{empty.rawIndex=await readCheckpointRegular(workspace.indexPath,this.limits.indexBytes);}catch(error){if(!noFile(error))throw error;}
      if((empty.rawIndex?checkpointHash(empty.rawIndex):'missing')!==workspace.indexFingerprint)fail('CONCURRENT_CHANGE','Index changed while being captured');
      let bytes=0;
      const retain=(data:Buffer)=>{const hash=checkpointHash(data);if(!empty.blobs.has(hash)){bytes+=data.length;if(bytes>this.limits.totalBytes)fail('TOTAL_LIMIT','Checkpoint contents exceed the total byte limit');empty.blobs.set(hash,data);}return hash;};
      if(empty.rawIndex)retain(empty.rawIndex);
      const oids=[...new Set([...head,...index].map(entry=>entry.oid))];const objects=new Map<string,Buffer>();
      if(oids.length){
        const input=Buffer.from(oids.join('\n')+'\n');
        const sizes=text(await git(workspace.projectRoot,['cat-file','--batch-check'],this.options,8*1024*1024,[0],input)).trimEnd().split('\n');
        if(sizes.length!==oids.length)fail('INVALID_GIT_OUTPUT','Git object inventory is incomplete');
        let total=0;
        for(let i=0;i<sizes.length;i++){const row=/^([0-9a-f]+) blob (\d+)$/.exec(sizes[i]!);if(!row||row[1]!==oids[i])fail('UNSUPPORTED_OBJECT','A checkpoint object is missing or is not a blob');const size=Number(row[2]);if(size>this.limits.fileBytes)fail('FILE_LIMIT','A Git blob exceeds the checkpoint file limit');total+=size;if(total+bytes>this.limits.totalBytes)fail('TOTAL_LIMIT','Git objects exceed the checkpoint total byte limit');}
        const body=await git(workspace.projectRoot,['cat-file','--batch'],this.options,total+oids.length*128+1,[0],input);let position=0;
        for(const oid of oids){const newline=body.indexOf(10,position);if(newline<0)fail('INVALID_GIT_OUTPUT','Git object body is incomplete');const row=/^([0-9a-f]+) blob (\d+)$/.exec(text(body.subarray(position,newline)));if(!row||row[1]!==oid)fail('INVALID_GIT_OUTPUT','Git object identity changed');const size=Number(row[2]);position=newline+1;const data=body.subarray(position,position+size);if(size>this.limits.fileBytes||data.length!==size||body[position+size]!==10)fail('INVALID_GIT_OUTPUT','Git object body is incomplete');
          if(createHash(oid.length===40?'sha1':'sha256').update(Buffer.from(`blob ${size}\0`)).update(data).digest('hex')!==oid)fail('INVALID_GIT_OUTPUT','Raw Git object hash does not match its identity');objects.set(oid,data);position+=size+1;}
        if(position!==body.length)fail('INVALID_GIT_OUTPUT','Git returned unexpected object content');
      }
      const entries=(source:GitEntry[]):CheckpointEntry[]=>source.map(entry=>{const data=objects.get(entry.oid)!;return{path:entry.path,kind:entry.mode==='120000'?'symlink':'file',mode:entry.mode,gitOid:entry.oid,bytes:data.length,sha256:retain(data)};});
      empty.head=entries(head);empty.index=entries(index);
      const rootBefore=await lstat(workspace.projectRoot),identities:unknown[]=[];
      for(const path of paths){const value=await checkpointDiskEntry(workspace.projectRoot,path,this.limits.fileBytes);if(value){retain(value.data);empty.disk.push(value.entry);identities.push([path,value.identity]);}}
      const rootAfter=await lstat(workspace.projectRoot);
      if(rootBefore.dev!==rootAfter.dev||rootBefore.ino!==rootAfter.ino)fail('CONCURRENT_CHANGE','The execution directory was replaced');
      const next=await getGitWorkspaceVersion(this.context.executionPath,this.options);if(sourceVersion(next)!==sourceVersion(workspace))fail('CONCURRENT_CHANGE','Repository identity, HEAD, index or config changed during checkpoint read');
      empty.preview.headFingerprint=checkpointHash(JSON.stringify([workspace.headOid,empty.head]));
      empty.preview.diskFingerprint=checkpointHash(JSON.stringify([rootBefore.dev,rootBefore.ino,empty.disk,identities,filesystem]));if(Buffer.byteLength(JSON.stringify({preview:empty.preview,head:empty.head,index:empty.index,disk:empty.disk,rawIndex:empty.rawIndex?{sha256:checkpointHash(empty.rawIndex),bytes:empty.rawIndex.length}:null}))+2048>this.limits.manifestBytes)fail('MANIFEST_LIMIT','Checkpoint metadata exceeds its declared byte limit');empty.preview.canCapture=true;
    }catch(error){
      if((error instanceof CodeCheckpointError||error instanceof GitWorkspaceError)&&['CONCURRENT_CHANGE','UNSAFE_PATH'].includes(error.code))throw error;
      if(!(error instanceof CodeCheckpointError||error instanceof GitWorkspaceError))throw error;
      empty.preview.unsupported.push({code:error.code,message:error.message});empty.preview.canCapture=false;
    }
    return finish();
  }
  private async stableScan():Promise<Scan>{const first=await this.scan();if(!first.preview.canCapture)return first;const next=await this.scan();if(first.preview.version!==next.preview.version)fail('CONCURRENT_CHANGE','Source inventory changed between checkpoint verification passes');return next;}
  async preview():Promise<CodeCheckpointPreview>{return structuredClone((await this.stableScan()).preview);}
  async capture(input:{expectedVersion:string;requestId:string}):Promise<CodeCheckpointOperation>{
    if(!input||Object.keys(input).some(key=>!['expectedVersion','requestId'].includes(key))||typeof input.expectedVersion!=='string'||typeof input.requestId!=='string'||!HASH.test(input.expectedVersion)||!UUID.test(input.requestId))fail('INVALID_ARGUMENT','A preview version and UUIDv4 request identity are required');
    const existing=await this.get(input.requestId);if(existing){if(existing.expectedVersion!==input.expectedVersion)fail('REQUEST_CONFLICT','This request identity belongs to another checkpoint preview');return existing;}
    const op:CodeCheckpointOperation={version:1,requestId:input.requestId,checkpointId:randomUUID(),workspaceId:this.context.workspaceId,sessionId:this.context.sessionId,expectedVersion:input.expectedVersion,state:'capturing'};
    const request=await this.requestPath(input.requestId);
    try{await jsonWrite(request,op,true);}catch(error){if((error as NodeJS.ErrnoException).code==='EEXIST'){const raced=await this.get(input.requestId);if(!raced||raced.expectedVersion!==input.expectedVersion)fail('REQUEST_CONFLICT','Checkpoint request was claimed by another preview');return raced;}throw new CodeCheckpointError('WRITE_FAILED','Checkpoint intent could not be durably claimed; source was not changed',{cause:error});}
    try{
      const scan=await this.stableScan();if(!scan.preview.canCapture)fail('UNSUPPORTED',scan.preview.unsupported.map(value=>value.code).join(', '));
      if(scan.preview.version!==input.expectedVersion)fail('STALE_SOURCE','Source changed since the checkpoint preview');
      const snapshot=join(await this.initialise(),'snapshots',op.checkpointId);await directory(snapshot);await directory(join(snapshot,'blobs'));
      for(const [hash,data] of scan.blobs){const file=join(snapshot,'blobs',hash);if(await realpath(dirname(file))!==dirname(file))fail('STORAGE_UNSAFE','Checkpoint blob storage was replaced or linked');const fd=await open(file,'wx',0o600);try{await fd.writeFile(data);await fd.sync();}finally{await fd.close();}}
      await syncDirectory(join(snapshot,'blobs'));
      const verified=await this.stableScan();if(verified.preview.version!==scan.preview.version)fail('CONCURRENT_CHANGE','Source changed before checkpoint publication');
      const manifest:CodeCheckpointManifest={version:2,checkpointId:op.checkpointId,requestId:op.requestId,workspaceId:op.workspaceId,sessionId:op.sessionId,capturedAt:Date.now(),preview:scan.preview,head:scan.head,index:scan.index,disk:scan.disk,rawIndex:scan.rawIndex?{sha256:checkpointHash(scan.rawIndex),bytes:scan.rawIndex.length}:null};
      if(Buffer.byteLength(JSON.stringify(manifest))>this.limits.manifestBytes)fail('MANIFEST_LIMIT','Checkpoint metadata exceeds its declared byte limit');
      await jsonWrite(join(snapshot,'manifest.json'),manifest,true);
      const ready:CodeCheckpointOperation={...op,state:'ready',manifestSha256:checkpointHash(JSON.stringify(manifest))};await jsonWrite(request,ready);return ready;
    }catch(error){
      const known=error instanceof CodeCheckpointError||error instanceof GitWorkspaceError?error:new CodeCheckpointError('WRITE_FAILED','Checkpoint could not be saved; source was not changed',{cause:error});
      try{await jsonWrite(request,{...op,state:'failed',error:{code:known.code,message:known.message}});}catch{throw new CodeCheckpointError('WRITE_FAILED','Checkpoint state could not be saved; the incomplete request will not be automatically recaptured',{cause:known});}
      throw known;
    }
  }
  private async readManifest(op:CodeCheckpointOperation):Promise<CodeCheckpointManifest>{
    const file=join(await this.initialise(),'snapshots',op.checkpointId,'manifest.json');
    try{
      const bytes=await this.privateRead(file,defaults.manifestBytes);if(checkpointHash(bytes)!==op.manifestSha256)throw 0;
      const manifest=JSON.parse(text(bytes)) as CodeCheckpointManifest;
      if(manifest.version!==2||manifest.checkpointId!==op.checkpointId||manifest.requestId!==op.requestId||manifest.workspaceId!==op.workspaceId||manifest.sessionId!==op.sessionId||!Number.isSafeInteger(manifest.capturedAt)||manifest.capturedAt<0||manifest.preview.version!==op.expectedVersion||manifest.preview.canCapture!==true||JSON.stringify(manifest.preview.coverage)!==JSON.stringify(coverage)||!Array.isArray(manifest.preview.exclusions)||manifest.preview.exclusions.some(value=>typeof value!=='string'))throw 0;
      for(const value of manifest.preview.exclusions)checkpointPath(value.endsWith('/')?value.slice(0,-1):value);
      let count=0;
      for(const list of [manifest.head,manifest.index,manifest.disk]){if(!Array.isArray(list))throw 0;const paths=new Set<string>();for(const entry of list){checkpointPath(entry.path);if(paths.has(entry.path)||++count>defaults.entries||!['file','symlink'].includes(entry.kind)||!['100644','100755','120000'].includes(entry.mode)||entry.kind==='symlink'&&entry.mode!=='120000'||entry.kind==='file'&&entry.mode==='120000'||entry.gitOid!==undefined&&!OID.test(entry.gitOid)||entry.permissions!==undefined&&(!Number.isSafeInteger(entry.permissions)||entry.permissions<0||entry.permissions>0o777)||!HASH.test(entry.sha256)||!Number.isSafeInteger(entry.bytes)||entry.bytes<0||entry.bytes>defaults.fileBytes)throw 0;paths.add(entry.path);}}
      if(manifest.rawIndex&&(!HASH.test(manifest.rawIndex.sha256)||!Number.isSafeInteger(manifest.rawIndex.bytes)||manifest.rawIndex.bytes<0||manifest.rawIndex.bytes>defaults.indexBytes))throw 0;
      // A ready claim is never returned for a missing/half-written or corrupt blob.
      const hashes=new Map<string,number>();for(const entry of [...manifest.head,...manifest.index,...manifest.disk]){if(hashes.has(entry.sha256)&&hashes.get(entry.sha256)!==entry.bytes)throw 0;hashes.set(entry.sha256,entry.bytes);}if(manifest.rawIndex)hashes.set(manifest.rawIndex.sha256,manifest.rawIndex.bytes);
      let total=0;for(const [hash,size] of hashes){total+=size;if(total>defaults.totalBytes)throw 0;const data=await this.privateRead(join(dirname(file),'blobs',hash),size);if(data.length!==size||checkpointHash(data)!==hash)throw 0;}
      return manifest;
    }catch(error){throw new CodeCheckpointError('CORRUPT','Checkpoint is missing or corrupt; it was not treated as ready or recaptured',{cause:error});}
  }
  async export(checkpointId:string):Promise<ExportedCodeCheckpoint>{
    if(typeof checkpointId!=='string'||!UUID.test(checkpointId))fail('INVALID_ARGUMENT','Invalid checkpoint identity');
    const file=join(await this.initialise(),'snapshots',checkpointId,'manifest.json');let raw:CodeCheckpointManifest;
    try{raw=JSON.parse(text(await this.privateRead(file,defaults.manifestBytes))) as CodeCheckpointManifest;}catch(error){throw new CodeCheckpointError('CORRUPT','Checkpoint manifest is unavailable',{cause:error});}
    if(!UUID.test(raw.requestId??''))fail('CORRUPT','Checkpoint request identity is invalid');
    const op=await this.operation(raw.requestId);if(!op||op.state!=='ready'||op.checkpointId!==checkpointId)fail('INCOMPLETE','Checkpoint publication did not complete');
    const manifest=await this.readManifest(op),blob=async(hash:string,size:number)=>{const data=await this.privateRead(join(dirname(file),'blobs',hash),size);if(data.length!==size||checkpointHash(data)!==hash)fail('CORRUPT','Checkpoint content changed during export');return data;};
    const entries=async(list:CheckpointEntry[])=>Promise.all(list.map(async entry=>({entry,data:await blob(entry.sha256,entry.bytes)})));
    return {manifest,head:await entries(manifest.head),index:await entries(manifest.index),disk:await entries(manifest.disk),rawIndex:manifest.rawIndex?await blob(manifest.rawIndex.sha256,manifest.rawIndex.bytes):null};
  }
}
