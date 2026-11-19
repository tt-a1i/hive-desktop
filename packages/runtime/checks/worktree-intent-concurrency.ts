/** Real local Git and separate Node processes; no model, account or remote calls. */
import assert from 'node:assert/strict';
import {execFileSync, spawn} from 'node:child_process';
import fs from 'node:fs';
import {access, mkdtemp, mkdir, readFile, realpath, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {syncBuiltinESMExports} from 'node:module';
import {setTimeout as delay} from 'node:timers/promises';

const defaultModule = fileURLToPath(new URL('../src/git-worktrees.js', import.meta.url));
const modulePath = process.env.WORKTREE_OWNER_MODULE ?? (fs.existsSync(defaultModule) ? defaultModule : defaultModule.replace(/\.js$/, '.ts'));
const moduleUrl = pathToFileURL(modulePath).href;
const childLoader = modulePath.endsWith('.ts') ? ['--import', fileURLToPath(import.meta.resolve('tsx'))] : [];
const {createManagedWorktree, recoverManagedWorktree, removeManagedWorktree, readWorktreeIntent} = await import(moduleUrl);
const expectBug = process.env.EXPECT_WORKTREE_RACE === '1';
const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-intent-concurrency-')));
const project = join(root, 'repo'), data = join(root, 'data'); await mkdir(project);
const git = (...args: string[]) => execFileSync('git', ['-C', project, ...args], {encoding:'utf8'}).trim();
const exists = async (path: string) => { try { await access(path); return true; } catch { return false; } };
async function until(test: () => Promise<boolean>, label: string) {
  for (let i = 0; i < 500; i++) { if (await test()) return; await delay(10); }
  throw new Error(`Timed out: ${label}`);
}
const children = new Set<ReturnType<typeof spawn>>();
const receipts: unknown[] = [];
async function interrupted(id: string, versioned = false) {
  const record = await createManagedWorktree({projectRoot:project, dataRoot:data, chatId:id,
    ...(versioned && {creation:{version:1,workspaceId:'isolated-workspace',backend:'pi',identityState:'new'}})});
  const intent = join(dirname(record.managedWorktreeRoot), 'intent.json');
  await writeFile(intent, JSON.stringify({...await readWorktreeIntent(data, id), state:'creating'}));
  return {record, intent};
}
async function pausedRecovery(entry: Awaited<ReturnType<typeof interrupted>>, mode: 'rename'|'before-lock'|'removed-receipt', suffix: string) {
  const folder = join(root, suffix); await mkdir(folder);
  const paused = join(folder,'paused'), release = join(folder,'release'), input = join(folder,'input.json'), runner = join(folder,'runner.mjs');
  await writeFile(input, JSON.stringify({data, record:entry.record}));
  await writeFile(runner, `import fs from 'node:fs';import {syncBuiltinESMExports} from 'node:module';import {readFile,writeFile,access} from 'node:fs/promises';import {setTimeout as delay} from 'node:timers/promises';
const rename=fs.promises.rename,lstat=fs.promises.lstat;let blocked=false;
async function pause(){blocked=true;await writeFile(${JSON.stringify(paused)},'paused');for(;;){try{await access(${JSON.stringify(release)});break;}catch{await delay(10);}}}
fs.promises.rename=async(src,dest)=>{if(!blocked&&${JSON.stringify(mode)}!=='before-lock'&&String(dest)===${JSON.stringify(entry.intent)}&&(${JSON.stringify(mode)}!=='removed-receipt'||JSON.parse(await readFile(src,'utf8')).removed))await pause();return rename(src,dest);};
fs.promises.lstat=async(path,...args)=>{if(!blocked&&${JSON.stringify(mode)}==='before-lock'&&String(path)===${JSON.stringify(join(dirname(entry.intent),'intent-lock.sqlite'))})await pause();return lstat(path,...args);};
syncBuiltinESMExports();const {recoverManagedWorktree,removeManagedWorktree}=await import(${JSON.stringify(moduleUrl)});const {data,record}=JSON.parse(await readFile(${JSON.stringify(input)},'utf8'));try{if(${JSON.stringify(mode)}==='removed-receipt')await removeManagedWorktree(data,record,{active:false});else await recoverManagedWorktree(data,record);console.log(JSON.stringify({ok:true}));}catch(error){console.log(JSON.stringify({ok:false,code:error.code,message:error.message}));}`);
  const child = spawn(process.execPath, [...childLoader, runner], {stdio:['ignore','pipe','pipe']}); children.add(child);
  let output = '', stderr = ''; child.stdout!.on('data', value => output += value); child.stderr!.on('data', value => stderr += value);
  const done = new Promise<{code:number|null;signal:string|null;result?:{ok:boolean;code?:string}}>((resolve,reject) => {
    child.once('error',reject); child.once('close',(code,signal) => {
      children.delete(child); if (code && !signal) reject(new Error(stderr));
      else resolve({code,signal,...(output.trim() && {result:JSON.parse(output.trim())})});
    });
  });
  await until(() => exists(paused), 'recovery publication pause');
  return {child, done, resume:() => writeFile(release,'resume')};
}
try {
  git('init','-q'); git('config','user.name','Fixture'); git('config','user.email','fixture@example.invalid');
  await writeFile(join(project,'a.txt'),'base'); git('add','.'); git('commit','-qm','base');
  const racing = await interrupted('same-publication');
  const a = await pausedRecovery(racing,'rename','same-publication-A');
  let competingCode: string | undefined;
  try { await recoverManagedWorktree(data,racing.record); await removeManagedWorktree(data,racing.record,{active:false}); }
  catch (error) { competingCode = (error as {code?:string}).code; }
  if (expectBug) {
    assert.equal(competingCode,undefined); assert.equal((await readWorktreeIntent(data,racing.record.chatId)).removed,true);
    await a.resume(); const aResult=await a.done;
    const after=await readWorktreeIntent(data,racing.record.chatId);
    assert.equal(after.removed,undefined); assert.equal(after.removal,undefined); assert.equal(await exists(racing.record.executionPath),false);
    receipts.push({case:'stale-recovery-overwrites-removal', reproduced:true, finalState:after.state,removed:false, aResult});
  } else {
    assert.equal(competingCode,'WORKTREE_METADATA_BUSY');
    assert.equal((await readWorktreeIntent(data,racing.record.chatId)).state,'creating');
    await a.resume(); assert.equal((await a.done).result?.ok,true);
    await recoverManagedWorktree(data,racing.record); await removeManagedWorktree(data,racing.record,{active:false});
    assert.equal((await readWorktreeIntent(data,racing.record.chatId)).removed,true);
    receipts.push({case:'writer-held-during-publication',competingCode,retryRemoved:true});

    const stale=await interrupted('stale-before-lock',true);
    const delayed=await pausedRecovery(stale,'before-lock','stale-before-lock-A');
    await recoverManagedWorktree(data,stale.record); await removeManagedWorktree(data,stale.record,{active:false});
    const terminal=await readFile(stale.intent,'utf8'); await delayed.resume();
    assert.equal((await delayed.done).result?.code,'WORKTREE_REMOVED');
    assert.equal(await readFile(stale.intent,'utf8'),terminal);
    assert.equal((await readWorktreeIntent(data,stale.record.chatId)).removed,true);
    receipts.push({case:'stale-snapshot-before-lock',rejected:'WORKTREE_REMOVED',terminalBytesPreserved:true});

    const crashed=await interrupted('killed-writer',true);
    const killed=await pausedRecovery(crashed,'rename','killed-writer-A');
    await assert.rejects(recoverManagedWorktree(data,crashed.record),(error:any)=>error.code==='WORKTREE_METADATA_BUSY');
    killed.child.kill('SIGKILL'); assert.equal((await killed.done).signal,'SIGKILL');
    await recoverManagedWorktree(data,crashed.record); await removeManagedWorktree(data,crashed.record,{active:false});
    assert.equal((await readWorktreeIntent(data,crashed.record.chatId)).removed,true);
    receipts.push({case:'killed-lock-holder',coldRetryRemoved:true});

    const removedCrash=await interrupted('killed-after-git-remove'); await recoverManagedWorktree(data,removedCrash.record);
    const removedWriter=await pausedRecovery(removedCrash,'removed-receipt','killed-removal-A');
    assert.equal(await exists(removedCrash.record.executionPath),false);
    await assert.rejects(readWorktreeIntent(data,removedCrash.record.chatId),(error:any)=>error.code==='WORKTREE_METADATA_BUSY');
    removedWriter.child.kill('SIGKILL'); await removedWriter.done;
    assert.equal((await readWorktreeIntent(data,removedCrash.record.chatId)).removed,true);
    await assert.rejects(removeManagedWorktree(data,removedCrash.record,{active:false}),(error:any)=>error.code==='WORKTREE_REMOVED');
    assert.equal(git('rev-parse','refs/heads/'+removedCrash.record.branch),removedCrash.record.baseOid);
    receipts.push({case:'killed-after-git-before-receipt',coldReconciliationRemoved:true,retry:'WORKTREE_REMOVED',branchPreserved:true});

    const left=await interrupted('independent-left'), right=await interrupted('independent-right');
    const pausedLeft=await pausedRecovery(left,'rename','independent-left-A');
    await recoverManagedWorktree(data,right.record); await removeManagedWorktree(data,right.record,{active:false});
    assert.equal((await readWorktreeIntent(data,right.record.chatId)).removed,true);
    await pausedLeft.resume(); await pausedLeft.done;
    await removeManagedWorktree(data,left.record,{active:false});
    receipts.push({case:'unrelated-worktree',notBlocked:true});

    const removing=await interrupted('concurrent-remove'); await recoverManagedWorktree(data,removing.record);
    async function removeProcess(name:string) {
      const folder=join(root,name); await mkdir(folder); const paused=join(folder,'paused'), release=join(folder,'release'),runner=join(folder,'runner.mjs');
      await writeFile(runner,`import {writeFile,access} from 'node:fs/promises';import {setTimeout as delay} from 'node:timers/promises';const {removeManagedWorktree}=await import(${JSON.stringify(moduleUrl)});try{await removeManagedWorktree(${JSON.stringify(data)},${JSON.stringify(removing.record)},{active:false,beforeRemove:async()=>{await writeFile(${JSON.stringify(paused)},'paused');for(;;){try{await access(${JSON.stringify(release)});break;}catch{await delay(10);}}}});console.log(JSON.stringify({ok:true}));}catch(error){console.log(JSON.stringify({ok:false,code:error.code}));}`);
      const child=spawn(process.execPath,[...childLoader,runner],{stdio:['ignore','pipe','pipe']});children.add(child);
      let output='';child.stdout!.on('data',v=>output+=v);
      const done=new Promise<{ok:boolean;code?:string}>((resolve,reject)=>{child.once('error',reject);child.once('close',code=>{children.delete(child);if(code)reject(Error('Removal child failed'));else resolve(JSON.parse(output.trim()));});});
      await until(()=>exists(paused),'concurrent removal preview');
      return {done,resume:()=>writeFile(release,'resume')};
    }
    const removeA=await removeProcess('remove-A'),removeB=await removeProcess('remove-B');
    await Promise.all([removeA.resume(),removeB.resume()]);const removalResults=await Promise.all([removeA.done,removeB.done]);
    assert(removalResults.some(result=>result.ok)); assert.equal((await readWorktreeIntent(data,removing.record.chatId)).removed,true);
    assert.equal(await exists(removing.record.executionPath),false);assert.equal(git('rev-parse','refs/heads/'+removing.record.branch),removing.record.baseOid);
    receipts.push({case:'concurrent-removals',results:removalResults,terminalReceiptPreserved:true,branchPreserved:true});

    const invalid=await interrupted('invalid-json'); const good=await readFile(invalid.intent,'utf8');
    await writeFile(invalid.intent,'{"state":'); await assert.rejects(recoverManagedWorktree(data,invalid.record));
    assert.equal(await readFile(invalid.intent,'utf8'),'{"state":'); assert(await exists(invalid.record.executionPath));
    await writeFile(invalid.intent,good); await recoverManagedWorktree(data,invalid.record);
    const lockPath=join(dirname(invalid.intent),'intent-lock.sqlite'), validLock=await readFile(lockPath);
    await writeFile(invalid.intent,good); await writeFile(lockPath,'not a sqlite database');
    await assert.rejects(recoverManagedWorktree(data,invalid.record)); assert.equal(await readFile(invalid.intent,'utf8'),good);
    assert(await exists(invalid.record.executionPath)); await writeFile(lockPath,validLock);
    await recoverManagedWorktree(data,invalid.record); await removeManagedWorktree(data,invalid.record,{active:false});
    await assert.rejects(recoverManagedWorktree(data,invalid.record),(error:any)=>error.code==='WORKTREE_REMOVED');
    receipts.push({case:'corrupt-json-and-lock',preserved:true,restoredRetryRemoved:true,removedRetry:'WORKTREE_REMOVED'});

    const originalRename=fs.promises.rename;
    fs.promises.rename=async(source,destination)=>{if(String(destination) === join(data, 'worktrees', 'first-write-failure', 'intent.json'))throw Object.assign(new Error('Injected pre-publication failure'),{code:'EIO'});return originalRename(source,destination);};
    syncBuiltinESMExports();
    try { await assert.rejects(createManagedWorktree({projectRoot:project,dataRoot:data,chatId:'first-write-failure'})); }
    finally {fs.promises.rename=originalRename;syncBuiltinESMExports();}
    assert.equal(await exists(join(data,'worktrees','first-write-failure')),false);
    const fresh=await createManagedWorktree({projectRoot:project,dataRoot:data,chatId:'first-write-failure'});
    await removeManagedWorktree(data,fresh,{active:false});
    receipts.push({case:'initial-receipt-write-failure',sameIdentityRetryRemoved:true});
  }
  console.log(JSON.stringify({status:expectBug?'WORKTREE_INTENT_RACE_REPRODUCED':'WORKTREE_INTENT_CONCURRENCY_OK',receipts},null,2));
} finally {
  const active = [...children];
  const exited = active.map(child => new Promise<void>(resolve => child.once('close', () => resolve())));
  for (const child of active) child.kill('SIGKILL');
  await Promise.all(exited);
  await rm(root,{recursive:true,force:true});
}
