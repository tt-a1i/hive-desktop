import assert from 'node:assert/strict';
import { execFile, execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import { constants } from 'node:fs';
import { copyFile, lstat, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { desktopPackagePaths, hashDesktopFile, isLive, prepareDesktopRecoveryEntry, reclaimDesktopUpdateTransactions, type DesktopInstallPlan } from '../src/desktop-update-installer.js';

const fixture = await realpath(await mkdtemp(join(tmpdir(), 'hive-install-')));
const preload = join(fixture, 'startup.cjs');
await writeFile(preload, `
  const fs = require('node:fs');
  const path = require('node:path');
  if (process.argv[2] === '--cleanup' && process.env.HIVE_TEST_KEEP_ROLLBACK_TRANSACTION) process.exit(0);
  if (process.env.HIVE_TEST_INSTALL_RECEIPT) {
    const io = fs.promises;
    const rename = io.rename;
    io.rename = async (from, to) => {
      if (to === process.env.HIVE_TEST_INSTALL_RECEIPT) throw new Error('Injected terminal receipt failure');
      return rename(from, to);
    };
    require('node:module').syncBuiltinESMExports();
  }
  if (process.argv[2] === '--cleanup' && process.env.HIVE_TEST_LOCKED_UPDATE_ZIP) {
    const io = fs.promises;
    const rm = io.rm;
    io.rm = async (file, options) => {
      if (file === process.env.HIVE_TEST_LOCKED_UPDATE_ZIP) {
        fs.writeFileSync(file + '.locked-fixture', 'cleanup attempted');
        throw Object.assign(new Error('Injected locked update ZIP'), {code:'EBUSY'});
      }
      return rm(file, options);
    };
    require('node:module').syncBuiltinESMExports();
  }
  const fixture = path.join(path.dirname(process.execPath), 'fixture.json');
  if (!fs.existsSync(fixture)) return;
  const {version, healthy} = JSON.parse(fs.readFileSync(fixture));
  const directory = process.env.HIVE_DESKTOP_DATA_DIR;
  const ready = process.env.HIVE_DESKTOP_UPDATE_READY;
  const token = require('node:crypto').randomUUID();
  const recordFile = path.join(directory, 'fixture-process-' + version + '.json');
  const gate = path.join(directory, 'fixture-exit-' + process.pid + '-' + token + '.json');
  const state = {version, pid:process.pid, token, candidate:!!ready, phase:'started', confirmed:false, exitRequested:false};
  const publish = () => {
    fs.writeFileSync(recordFile + '.tmp', JSON.stringify(state));
    fs.renameSync(recordFile + '.tmp', recordFile);
  };
  const read = file => { try { return JSON.parse(fs.readFileSync(file)); } catch { return undefined; } };
  fs.appendFileSync(path.join(directory, 'started'), version + '\\n');
  publish();
  process.on('exit', code => {
    state.phase = 'exited';
    state.exitCode = code;
    publish();
    if (healthy && code === 0) fs.appendFileSync(path.join(directory, 'exited'), version + '\\n');
  });
  if (!healthy) process.exit(37);
  // A restored old fixture can finish naturally after recording startup.
  if (!ready) return;
  let sentReady = false;
  const timer = setInterval(() => {
    const permit = read(gate);
    const addressed = permit && permit.pid === process.pid && permit.token === token;
    if (addressed && permit.action === 'abort') {
      clearInterval(timer);
      process.exitCode = 2;
      return;
    }
    const transaction = path.dirname(ready);
    const journal = read(path.join(transaction, 'install-status.json'));
    if (!sentReady && journal && journal.state === 'starting' && journal.candidatePid === process.pid) {
      fs.writeFileSync(ready + '.tmp', JSON.stringify({version,pid:process.pid}));
      fs.renameSync(ready + '.tmp', ready);
      sentReady = true;
    }
    const receipt = read(path.join(directory, 'desktop-updates/install-result.json'));
    if (sentReady && ((journal && journal.state === 'installed' && journal.candidatePid === process.pid)
      || (receipt && receipt.state === 'installed' && receipt.version === version && receipt.transactionId === path.basename(transaction)))) {
      if (!state.confirmed) { state.confirmed = true; publish(); }
    }
    // The test releases this candidate only after the installer invocation has
    // returned. Keep it alive until the backend has also confirmed its health.
    if (addressed && permit.action === 'exit' && state.confirmed) {
      state.exitRequested = true;
      publish();
      clearInterval(timer);
    }
  }, 20);
`);

const binary = await realpath(process.execPath);
const checksum = await hashDesktopFile(binary);
const previousOptions = process.env.NODE_OPTIONS;
process.env.NODE_OPTIONS = `--require "${preload.replaceAll('\\', '/')}"`;

type FixtureProcess = {version:string; pid:number; token:string; candidate:boolean; phase:'started'|'exited'; confirmed:boolean; exitRequested:boolean; exitCode?:number};
const fixtureDirectories = new Set<string>();
const activeInstallers = new Set<ChildProcess>();
const releasedFixtures = new Set<string>();
let context: {group?:string; root?:string; installation?:string; dataDirectory?:string; transaction?:string} = {};
let phase: {step:string; [key:string]:unknown} = {step:'startup'};
type Invocation = {pid:number|undefined; mode:string; startedAt:number; elapsedMs:number|undefined; signal:string|null|undefined; killed:boolean|undefined};
let lastInvocation: Invocation | undefined;
let failed = false;
async function fixtureProcess(directory:string,version:string):Promise<FixtureProcess|undefined> {
  try {
    const value = JSON.parse(await readFile(join(directory, `fixture-process-${version}.json`),'utf8')) as FixtureProcess;
    assert.equal(value.version,version); assert.ok(Number.isSafeInteger(value.pid) && value.pid > 1);
    assert.match(value.token,/^[a-f0-9-]{36}$/i);
    return value;
  } catch(error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return; throw error; }
}
function fixtureGate(directory:string,value:FixtureProcess) { return join(directory,`fixture-exit-${value.pid}-${value.token}.json`); }
async function invokeInstaller(node:string,program:string,mode:string,env:NodeJS.ProcessEnv) {
  phase = {step:'installer',mode};
  const task = promisify(execFile)(node,[program,mode],{windowsHide:true,timeout:10000,env});
  activeInstallers.add(task.child);
  const invocation:Invocation = {pid:task.child.pid,mode,startedAt:Date.now(),elapsedMs:undefined,signal:undefined,killed:undefined};
  lastInvocation = invocation;
  try { return await task; }
  catch(error) { Object.assign(invocation,{signal:(error as {signal?:string}).signal,killed:(error as {killed?:boolean}).killed}); throw error; }
  finally { invocation.elapsedMs=Date.now()-invocation.startedAt; activeInstallers.delete(task.child); }
}
function processEvidence(pid:number|undefined) {
  if(!Number.isSafeInteger(pid) || !pid || pid<=1)return;
  const evidence:{pid:number;live:boolean;identity?:string}={pid,live:isLive(pid)};
  if(process.platform!=='win32') {
    try {evidence.identity=execFileSync('/bin/ps',['-p',String(pid),'-o','pid=,ppid=,stat=,lstart=,comm='],{encoding:'utf8'}).trim();} catch {}
  }
  return evidence;
}
async function failureEvidence(reason:string) {
  if(failed)return;
  failed = true;
  const read = async (path:string|undefined) => { if(!path)return;try {const text=(await readFile(path,'utf8')).slice(0,2048);try{return JSON.parse(text);}catch{return text;}}catch(error){return {code:(error as NodeJS.ErrnoException).code};} };
  const processes=[];
  for (const version of ['0.1.0','0.1.1']) {
    if(!context.dataDirectory)continue;
    const state=await fixtureProcess(context.dataDirectory,version).catch(error=>({error:String(error)}));
    processes.push({version,state,os:state && 'pid' in state ? processEvidence(state.pid) : undefined});
  }
  console.error('INSTALL_FIXTURE_FAILURE '+JSON.stringify({reason,context,phase,lastInvocation,activeInstallers:[...activeInstallers].map(child=>processEvidence(child.pid)),invocationProcesses:{installer:processEvidence(lastInvocation?.pid),original:processEvidence(Number(lastInvocation?.mode))},expectedExecutableSha256:checksum,
    journal:await read(context.transaction&&join(context.transaction,'install-status.json')),ready:await read(context.transaction&&join(context.transaction,'ready.json')),
    receipt:await read(context.dataDirectory&&join(context.dataDirectory,'desktop-updates/install-result.json')),
    started:await read(context.dataDirectory&&join(context.dataDirectory,'started')),exited:await read(context.dataDirectory&&join(context.dataDirectory,'exited')),processes}));
}
async function releaseOwnedFixtures() {
  for (const directory of fixtureDirectories) for (const version of ['0.1.0','0.1.1']) {
    const state=await fixtureProcess(directory,version).catch(()=>undefined);
    if(!state || !isLive(state.pid))continue;
    await writeFile(fixtureGate(directory,state),JSON.stringify({pid:state.pid,token:state.token,action:'abort'}));
    for(let i=0;i<100&&isLive(state.pid);i++)await delay(30);
    if(isLive(state.pid))throw new Error(`Fixture PID ${state.pid} has not exited; kept its control files in ${fixture}`);
  }
}
process.once('SIGTERM',()=>{void(async()=>{
  await failureEvidence('parent SIGTERM');
  for(const child of activeInstallers)if(child.exitCode===null&&child.signalCode===null)child.kill('SIGTERM');
  await releaseOwnedFixtures();
})().finally(()=>process.exit(143));});

async function copyNodeFixture(path: string) {
  phase={step:'copy-node',path};
  if (process.platform === 'darwin') {
    // The packaged Node does not clone files with COPYFILE_FICLONE on macOS.
    await promisify(execFile)('/bin/cp', ['-c', binary, path]);
  } else {
    await copyFile(binary, path, constants.COPYFILE_FICLONE);
  }
}

async function executable(directory: string, version: string, healthy: boolean) {
  const path = desktopPackagePaths(directory).executable;
  await mkdir(dirname(path), { recursive: true });
  await copyNodeFixture(path);
  await writeFile(join(dirname(path), 'fixture.json'), JSON.stringify({ version, healthy }));
  const node = desktopPackagePaths(directory).node;
  await mkdir(dirname(node), { recursive: true });
  await copyNodeFixture(node);
  return path;
}

async function prepareTransaction(root: string, plan: DesktopInstallPlan) {
  const transaction = await mkdtemp(join(root, '.hive-desktop-install-'));
  const program = join(transaction, 'installer.mjs');
  const node = join(transaction, process.platform === 'win32' ? 'node.exe' : 'node');
  const packagedRuntime = process.env.HIVE_DESKTOP_TEST_RUNTIME;
  const compiled = packagedRuntime ? join(dirname(packagedRuntime), 'desktop-update-installer.js')
    : new URL('../dist/desktop-update-installer.js', import.meta.url);
  await copyFile(compiled, program);
  await copyNodeFixture(node);
  await writeFile(join(transaction, 'install-plan.json'), JSON.stringify(plan));
  context.transaction=transaction;
  return { transaction, program, node, entry: await prepareDesktopRecoveryEntry(transaction) };
}

async function waitForRemoval(path: string) {
  phase={step:'transaction-cleanup',path};
  const deadline = Date.now() + 7000;
  for (;;) {
    if (!await lstat(path).then(() => true, error => { if (error.code === 'ENOENT') return false; throw error; })) return;
    assert.ok(Date.now() < deadline, `Settled transaction was not removed: ${path}`);
    await delay(50);
  }
}

// Keep the existing polling budget; the legacy version marker alone did not
// establish that the fixture process had ended.
async function waitForExit(directory: string, version: string) {
  phase={step:'fixture-exit',version};
  for (let i = 0; i < 100; i++) {
    const state=await fixtureProcess(directory,version);
    if(state) {
      const key=`${state.pid}:${state.token}`;
      if(state.candidate && !releasedFixtures.has(key)) {
        assert.equal(state.phase,'started','Healthy candidate exited before the test released it');
        assert.ok(isLive(state.pid),'Healthy candidate is no longer running');
        await writeFile(fixtureGate(directory,state),JSON.stringify({pid:state.pid,token:state.token,action:'exit'}));
        releasedFixtures.add(key);
      }
      const output = await readFile(join(directory, 'exited'), 'utf8').catch(() => '');
      if (output.split('\n').includes(version) && state.phase==='exited' && !isLive(state.pid)) {
        assert.equal(state.exitCode,0);
        if(state.candidate) {assert.equal(state.confirmed,true);assert.equal(state.exitRequested,true);}
        return;
      }
    }
    await delay(30);
  }
  assert.fail(`Desktop fixture ${version} did not exit`);
}

async function checkTransaction(healthy: boolean, receiptFault = false, lockedCache = false, supersededReceipt = false) {
  const root = join(fixture, receiptFault ? 'receipt-failure' : healthy ? 'healthy' : 'broken');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  context={...(context.group && {group:context.group}),root,installation,dataDirectory};lastInvocation=undefined;
  fixtureDirectories.add(dataDirectory);phase={step:'prepare-fixture'};
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'User draft must survive replacement.');
  const current = await executable(installation, '0.1.0', true);
  const archiveSha256 = 'f'.repeat(64);
  const {transaction, program, node, entry} = await prepareTransaction(root, {
    installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum, archiveSha256,
  });
  await executable(join(transaction, 'next'), '0.1.1', healthy);
  const releaseCache = join(dataDirectory, 'desktop-updates', `${archiveSha256}.zip`);
  const attestation = releaseCache.replace(/\.zip$/, '.manifest.json');
  await writeFile(releaseCache, 'Verified fixture ZIP');
  await writeFile(attestation, '{}');
  const receiptPath = join(dataDirectory, 'desktop-updates/install-result.json');
  const original = spawn(binary, ['-e', ''], { env: process.env, stdio: 'ignore' });
  await once(original, 'exit');
  const run = (mode: string, fault?: string) => invokeInstaller(node,program,mode,{ ...process.env, HIVE_TEST_INSTALL_RECEIPT: fault ?? '',
    HIVE_TEST_LOCKED_UPDATE_ZIP: lockedCache ? releaseCache : '',
    HIVE_TEST_KEEP_ROLLBACK_TRANSACTION: !healthy && receiptFault ? '1' : '' });
  const expected = healthy ? '0.1.1' : '0.1.0';
  const state = healthy ? 'installed' : 'rolledBack';
  const laterReceipt = { state: 'rolledBack', version: expected, requestedVersion: '0.1.2', transactionId: '.hive-desktop-install-later' };
  const startupFailure = 'Updated desktop exited before startup completed (37)';
  if (receiptFault) {
    await assert.rejects(run(String(original.pid), receiptPath), /Injected terminal receipt failure/);
    await assert.rejects(readFile(receiptPath), {code:'ENOENT'});
    assert.match(await readFile(entry, 'utf8'), /--recover/);
    const journal = JSON.parse(await readFile(join(transaction, 'install-status.json'), 'utf8'));
    assert.equal(journal.settledBy, undefined);
    assert.equal(journal.state, state);
    if (healthy) await lstat(desktopPackagePaths(join(transaction, 'previous')).executable);
    else {
      assert.equal(journal.error, startupFailure);
      await assert.rejects(run('--recover', receiptPath), /Injected terminal receipt failure/);
    }
    await waitForExit(dataDirectory, expected);
    if (supersededReceipt) await writeFile(receiptPath, JSON.stringify(laterReceipt));
    const recoveryMessage = healthy ? /0\.1\.1 is installed/ : /The original installation is already available/;
    assert.match((await run('--recover')).stdout, recoveryMessage);
    if (!healthy) {
      // Keep the real transaction around to exercise the settled recovery path too.
      assert.match((await run('--recover')).stdout, recoveryMessage);
      await reclaimDesktopUpdateTransactions(installation, dataDirectory);
    }
  } else await run(String(original.pid));

  const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
  assert.equal(receipt.state, state, JSON.stringify(receipt));
  await waitForExit(dataDirectory, expected);
  assert.equal(receipt.version, expected);
  if (supersededReceipt) assert.deepEqual(receipt, laterReceipt);
  else {
    assert.equal(receipt.requestedVersion, '0.1.1');
    if (receiptFault && !healthy) assert.equal(receipt.error, startupFailure);
  }
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, expected);
  assert.equal(await readFile(join(dataDirectory, 'started'), 'utf8'), healthy ? '0.1.1\n' : '0.1.1\n0.1.0\n');
  assert.equal(await readFile(draft, 'utf8'), 'User draft must survive replacement.');
  if (lockedCache) {
    const lockEvent = releaseCache + '.locked-fixture';
    const deadline = Date.now() + 7000;
    while (await readFile(lockEvent, 'utf8').catch(() => '') !== 'cleanup attempted') {
      assert.ok(Date.now() < deadline, 'The detached cleaner never attempted to release the locked archive');
      await delay(50);
    }
    assert.equal(await readFile(releaseCache, 'utf8'), 'Verified fixture ZIP');
    assert.equal(await readFile(attestation, 'utf8'), '{}');
    assert.ok(JSON.parse(await readFile(join(transaction, 'install-status.json'), 'utf8')).settledBy);
    while (await lstat(transaction).then(() => true, error => { if (error.code === 'ENOENT') return false; throw error; })) {
      assert.ok(Date.now() < deadline, 'The next check did not reclaim the transaction');
      await reclaimDesktopUpdateTransactions(installation, dataDirectory);
      await delay(50);
    }
  }
  await waitForRemoval(transaction);
  await assert.rejects(lstat(entry), {code:'ENOENT'});
  if (healthy) {
    await assert.rejects(readFile(releaseCache), {code:'ENOENT'});
    await assert.rejects(lstat(attestation), {code:'ENOENT'});
  } else assert.equal(await readFile(releaseCache, 'utf8'), 'Verified fixture ZIP');
  await rm(root, { recursive: true, maxRetries: 10, retryDelay: 100 });
}

async function checkInterruptedReplacement(beforeJournal = false) {
  const root = join(fixture, beforeJournal ? 'interrupted-before-journal' : 'interrupted-after-backup');
  const installation = join(root, 'installation');
  const dataDirectory = join(root, 'data');
  context={...(context.group && {group:context.group}),root,installation,dataDirectory};lastInvocation=undefined;
  fixtureDirectories.add(dataDirectory);phase={step:'prepare-fixture'};
  await mkdir(join(dataDirectory, 'desktop-updates'), { recursive: true });
  const draft = join(dataDirectory, 'draft.txt');
  await writeFile(draft, 'Draft before interruption');
  const current = await executable(installation, '0.1.0', true);
  const digest = 'e'.repeat(64);
  const cache = join(dataDirectory, 'desktop-updates', `${digest}.zip`);
  const attestation = cache.replace(/\.zip$/, '.manifest.json');
  const receiptPath = join(dataDirectory, 'desktop-updates/install-result.json');
  const earlierReceipt = '{"transactionId":"earlier-transaction"}';
  await writeFile(cache, 'Verified fixture ZIP');
  await writeFile(attestation, '{}');
  await writeFile(receiptPath, earlierReceipt);
  const {transaction, program, node, entry} = await prepareTransaction(root, {
    installation, dataDirectory, fromVersion: '0.1.0', toVersion: '0.1.1',
    oldExecutableSha256: checksum, newExecutableSha256: checksum, archiveSha256: digest,
  });
  const next = await executable(join(transaction, 'next'), '0.1.1', true);
  const journal = join(transaction, 'install-status.json');
  const previous = join(transaction, 'previous');
  const gate = join(transaction, 'rename-gate.cjs');
  const marker = join(transaction, 'at-rename');
  // Pause only the test installer with the lock held, either before the journal
  // is committed or once the original has actually moved to the backup path.
  await writeFile(gate, `
    const fs = require('node:fs/promises');
    const {syncBuiltinESMExports} = require('node:module');
    const rename = fs.rename;
    fs.rename = async (from, to) => {
      if (to !== process.env.HIVE_TEST_RENAME_TARGET) return rename(from, to);
      if (!process.env.HIVE_TEST_BEFORE_RENAME) await rename(from, to);
      await fs.writeFile(process.env.HIVE_TEST_MARKER, 'paused');
      await new Promise(resolve => setTimeout(resolve, 30000));
    };
    syncBuiltinESMExports();
  `);
  const env: NodeJS.ProcessEnv = { ...process.env };
  delete env.NODE_OPTIONS;
  const original = spawn(binary, ['-e', ''], { env, stdio: 'ignore' });
  await once(original, 'exit');
  const installer = spawn(node, [program, String(original.pid)], {
    stdio: 'ignore',
    env: { ...env, NODE_OPTIONS: `--require "${gate.replaceAll('\\', '/')}"`,
      HIVE_TEST_RENAME_TARGET: beforeJournal ? journal : previous,
      HIVE_TEST_BEFORE_RENAME: beforeJournal ? '1' : '', HIVE_TEST_MARKER: marker },
  });
  activeInstallers.add(installer);
  const exited = once(installer, 'exit');
  const recover = () => invokeInstaller(node,program,'--recover',env);
  try {
    for (let i = 0; i < 200; i++) {
      assert.equal(installer.exitCode, null, 'installer exited before reaching the rename gate');
      if (await readFile(marker, 'utf8').catch(() => '') === 'paused') break;
      await delay(25);
    }
    assert.equal(await readFile(marker, 'utf8'), 'paused');
    if (beforeJournal) {
      await assert.rejects(lstat(journal), { code: 'ENOENT' });
      assert.equal(await hashDesktopFile(current), checksum);
    } else await assert.rejects(lstat(current), { code: 'ENOENT' });
    await assert.rejects(recover(), /Another desktop installer is running/);
  } finally {
    if (installer.exitCode === null && installer.signalCode === null) installer.kill('SIGKILL');
    await exited;
    activeInstallers.delete(installer);
  }
  assert.match(await readFile(entry, 'utf8'), /installer\.mjs.*--recover/);
  if (beforeJournal) {
    await mkdir(previous);
    await assert.rejects(recover(), /Cannot verify an unstarted desktop update/);
    await assert.rejects(lstat(journal), { code: 'ENOENT' });
    assert.equal(await hashDesktopFile(next), checksum);
    await rm(previous, { recursive: true });
    env.NODE_OPTIONS = process.env.NODE_OPTIONS;
    env.HIVE_TEST_KEEP_ROLLBACK_TRANSACTION = '1';
    assert.match((await recover()).stdout, /The pending update was cancelled/);
    assert.match((await recover()).stdout, /The pending update was cancelled/);
    await assert.rejects(invokeInstaller(node,program,String(original.pid),env), /This desktop update has already started/);
    assert.equal(await readFile(receiptPath, 'utf8'), earlierReceipt);
    await reclaimDesktopUpdateTransactions(installation, dataDirectory);
  } else {
    assert.match((await recover()).stdout, /0\.1\.0 was recovered/);
    const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
    assert.equal(receipt.state, 'rolledBack');
  }
  assert.equal(JSON.parse(await readFile(join(dirname(current), 'fixture.json'), 'utf8')).version, '0.1.0');
  assert.equal(await hashDesktopFile(current), checksum);
  assert.equal(await readFile(cache, 'utf8'), 'Verified fixture ZIP');
  assert.equal(await readFile(attestation, 'utf8'), '{}');
  assert.equal(await readFile(draft, 'utf8'), 'Draft before interruption');
  await waitForRemoval(transaction);
  await assert.rejects(lstat(entry), {code:'ENOENT'});
}

const groups = {
  normal: async () => { await checkTransaction(true, false, true); await checkTransaction(false); },
  receipts: async () => { await checkTransaction(true, true); await checkTransaction(false, true); await checkTransaction(false, true, false, true); },
  interrupted: async () => { await checkInterruptedReplacement(); await checkInterruptedReplacement(true); },
};
const groupAt = process.argv.indexOf('--group');
const requested = groupAt < 0 ? undefined : process.argv[groupAt + 1];
try {
  if (groupAt >= 0 && (!requested || !Object.hasOwn(groups,requested))) throw new Error('Select --group normal, receipts or interrupted');
  for (const name of requested ? [requested as keyof typeof groups] : Object.keys(groups) as (keyof typeof groups)[]) {
    context={group:name};
    const started = Date.now();
    await groups[name]();
    console.log(`DESKTOP_UPDATE_INSTALLER_GROUP_OK ${name} ${Date.now() - started}ms`);
  }
  console.log('DESKTOP_UPDATE_INSTALLER_OK: replacement, failed-start rollback, interrupted-installer recovery, and unchanged user data.');
} catch(error) {
  await failureEvidence(error instanceof Error?error.message:String(error));
  throw error;
} finally {
  if (previousOptions === undefined) delete process.env.NODE_OPTIONS;
  else process.env.NODE_OPTIONS = previousOptions;
  try {await releaseOwnedFixtures();await rm(fixture, { recursive: true, force: true });}
  catch(error) {if(!failed)throw error;console.error(String(error));}
}
