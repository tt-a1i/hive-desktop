/** Real isolated Git repositories and local bare remotes. No hosted service or credentials. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { chmod, mkdir, mkdtemp, readFile, writeFile, access, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { previewGitPush, pushGitBranch, listGitRemotes, redactRemoteText, remoteCommand } from '../src/git-remotes.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-git-remotes-')); console.log('GIT_REMOTES_EVIDENCE', directory);
const repo = join(directory, 'repo'), bare = join(directory, 'remote.git'), other = join(directory, 'other');
const old = {PATH: process.env.PATH, GIT_CONFIG_GLOBAL: process.env.GIT_CONFIG_GLOBAL, GIT_CONFIG_NOSYSTEM: process.env.GIT_CONFIG_NOSYSTEM};
const gitPath = execFileSync('/usr/bin/which', ['git'], {encoding: 'utf8'}).trim();
const git = (cwd: string, ...args: string[]) => execFileSync(gitPath, ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd, encoding: 'utf8', stdio: 'pipe'}).trim();
const commit = async (text: string) => {await writeFile(join(repo, 'file'), text); git(repo, 'add', '.'); git(repo, 'commit', '-m', text); return git(repo, 'rev-parse', 'HEAD');};
try {
 await writeFile(join(directory, 'gitconfig'), ''); process.env.GIT_CONFIG_GLOBAL = join(directory, 'gitconfig'); process.env.GIT_CONFIG_NOSYSTEM = '1';
 await mkdir(repo); git(repo, 'init', '--template='); git(repo, 'init', '--bare', '--template=', bare); git(repo, 'remote', 'add', 'origin', bare);
 const base = await commit('base'); const index = await readFile(join(repo, '.git/index')), configuration = await readFile(join(repo, '.git/config'));
 const target = {remote: 'origin', branch: 'review/explicit-target'};
 const initial = await previewGitPush(repo, target); assert.equal(initial.relation, 'new'); assert.equal(initial.headOid, base); assert.equal(initial.commits[0]!.oid, base);
 const first = await pushGitBranch(repo, {...target, expectedVersion: initial.version}); assert.equal(first.pushed, true); assert.equal(first.verified, true); assert.equal(git(bare, 'rev-parse', 'refs/heads/' + target.branch), base);
 assert.deepEqual(await readFile(join(repo, '.git/index')), index); assert.deepEqual(await readFile(join(repo, '.git/config')), configuration); assert.ok(!git(repo, 'config', '--list').split('\n').some(line => line.startsWith('branch.')));
 const next = await commit('fast forward'); const ff = await previewGitPush(repo, target); assert.equal(ff.relation, 'fast-forward'); assert.equal(ff.commits.length, 1); assert.equal(ff.commits[0]!.oid, next);
 await mkdir(join(repo, '.git/hooks'));
 const hook = join(repo, '.git/hooks/pre-push'), hookMarker = join(directory, 'HOOK');
 await writeFile(hook, `#!/bin/sh\nprintf ran > '${hookMarker}'\nexit 1\n`); await chmod(hook, 0o700);
 const rejected = await pushGitBranch(repo, {...target, expectedVersion: ff.version}); assert.equal(rejected.pushed, 'unknown'); assert.equal(rejected.ok, false); assert.equal(await readFile(hookMarker, 'utf8'), 'ran'); assert.equal(git(bare, 'rev-parse', 'refs/heads/' + target.branch), base);
 await rm(hook); const ffAgain = await previewGitPush(repo, target); assert.equal((await pushGitBranch(repo, {...target, expectedVersion: ffAgain.version})).ok, true);
 const stale = await previewGitPush(repo, target); await commit('local changed'); await assert.rejects(pushGitBranch(repo, {...target, expectedVersion: stale.version}), (e: any) => e.code === 'STALE_PREVIEW');
 const configStale = await previewGitPush(repo, target); git(repo, 'config', 'push.default', 'nothing'); await assert.rejects(pushGitBranch(repo, {...target, expectedVersion: configStale.version}), (e: any) => e.code === 'STALE_PREVIEW');
 // Another actor advances the bare branch between preview and confirmation.
 git(repo, 'clone', '--no-checkout', bare, other); git(other, 'checkout', '-b', 'external', 'origin/' + target.branch); await writeFile(join(other, 'file'), 'foreign'); git(other, 'add', '.'); git(other, 'commit', '-m', 'foreign'); git(other, 'push', 'origin', 'HEAD:refs/heads/' + target.branch);
 const observeProxy = join(directory, 'git-no-push-peer'), blockedPushMarker = join(directory, 'BLOCKED_PUSH_STARTED');
 await writeFile(observeProxy, `#!/usr/bin/env node\nconst fs=require('node:fs'),cp=require('node:child_process'),a=process.argv.slice(2);if(a.includes('push'))fs.writeFileSync(${JSON.stringify(blockedPushMarker)},'started');const r=cp.spawnSync(${JSON.stringify(gitPath)},a,{stdio:'inherit'});process.exit(r.status??1);\n`); await chmod(observeProxy, 0o700);
 const observeOptions = {gitExecutable: observeProxy};
 const unknown = await previewGitPush(repo, target, observeOptions); assert.equal(unknown.relation, 'unknown-history'); assert.equal(unknown.canPush, false);
 await assert.rejects(pushGitBranch(repo, {...target, expectedVersion: unknown.version}, observeOptions), (error: any) => error.code === 'REMOTE_HISTORY_REQUIRED'); await assert.rejects(access(blockedPushMarker));
 git(repo, 'fetch', 'origin', target.branch); // Explicit fixture user fetch, never an automatic product fetch.
 const diverged = await previewGitPush(repo, target, observeOptions); assert.equal(diverged.relation, 'diverged'); assert.equal(diverged.canPush, false);
 await assert.rejects(pushGitBranch(repo, {...target, expectedVersion: diverged.version}, observeOptions), (error: any) => error.code === 'NON_FAST_FORWARD'); await assert.rejects(access(blockedPushMarker));
 assert.equal(git(bare, 'rev-parse', 'refs/heads/' + target.branch), git(other, 'rev-parse', 'HEAD'));
 // Replacement objects and legacy grafts must not manufacture the ancestry proof.
 const localHead = git(repo, 'rev-parse', 'HEAD'), remoteHead = git(other, 'rev-parse', 'HEAD');
 git(repo, 'replace', '--graft', localHead, remoteHead); assert.equal((await previewGitPush(repo, target)).relation, 'diverged'); git(repo, 'replace', '-d', localHead);
 await mkdir(join(repo, '.git/info'), {recursive: true}); await writeFile(join(repo, '.git/info/grafts'), `${localHead} ${remoteHead}\n`);
 assert.equal((await previewGitPush(repo, target)).relation, 'diverged'); await rm(join(repo, '.git/info/grafts'));

 // During native pre-push, an actor can advance the remote. Native push rejects its stale old ref.
 const raceTarget = {remote: 'origin', branch: 'race'}; let race = await previewGitPush(repo, raceTarget); await pushGitBranch(repo, {...raceTarget, expectedVersion: race.version}); const oldRace = git(bare, 'rev-parse', 'refs/heads/race');
 const foreignCommit = git(repo, 'commit-tree', git(repo, 'rev-parse', 'HEAD^{tree}'), '-p', oldRace, '-m', 'foreign remote actor');
 git(repo, 'push', 'origin', foreignCommit + ':refs/heads/foreign-object');
 await commit('race-local'); race = await previewGitPush(repo, raceTarget);
 await writeFile(hook, `#!/bin/sh\n'${gitPath}' --git-dir='${bare}' update-ref refs/heads/race '${foreignCommit}' '${oldRace}'\n`); await chmod(hook, 0o700);
 const raced = await pushGitBranch(repo, {...raceTarget, expectedVersion: race.version}); assert.equal(raced.ok, false); assert.equal(git(bare, 'rev-parse', 'refs/heads/race'), foreignCommit); await rm(hook);
 // CLI initialization can race a local HEAD change. The refspec still pushes only the approved OID.
 const proxy = join(directory, 'git-proxy'), approved = await previewGitPush(repo, {remote: 'origin', branch: 'exact'});
 const newHead = git(repo, 'commit-tree', git(repo, 'rev-parse', 'HEAD^{tree}'), '-p', approved.headOid, '-m', 'HEAD changed at launch');
 await writeFile(proxy, `#!/usr/bin/env node\nconst cp=require('node:child_process'),a=process.argv.slice(2);if(a.includes('push'))cp.execFileSync(${JSON.stringify(gitPath)},['update-ref','HEAD',${JSON.stringify(newHead)},${JSON.stringify(approved.headOid)}]);const r=cp.spawnSync(${JSON.stringify(gitPath)},a,{stdio:'inherit'});process.exit(r.status??1);\n`); await chmod(proxy, 0o700);
 const proxyPreview = await previewGitPush(repo, {remote: 'origin', branch: 'exact'}, {gitExecutable: proxy}); assert.equal(proxyPreview.headOid, approved.headOid);
 const exact = await pushGitBranch(repo, {remote: 'origin', branch: 'exact', expectedVersion: proxyPreview.version}, {gitExecutable: proxy}); assert.equal(exact.ok, true); assert.equal(git(bare, 'rev-parse', 'refs/heads/exact'), approved.headOid); assert.equal(git(repo, 'rev-parse', 'HEAD'), newHead);
 // Bind the approved old OID even if the remote moves before Git's handshake.
 const leaseProxy = join(directory, 'git-lease-peer'), leaseRef = 'refs/heads/lease-before-handshake';
 git(bare, 'update-ref', leaseRef, base);
 await writeFile(leaseProxy, `#!/usr/bin/env node\nconst cp=require('node:child_process'),a=process.argv.slice(2);if(a.includes('push')){const ref=a.at(-1).split(':')[1];if(ref==='refs/heads/lease-before-handshake')cp.execFileSync(${JSON.stringify(gitPath)},['--git-dir',${JSON.stringify(bare)},'update-ref',ref,${JSON.stringify(next)},${JSON.stringify(base)}]);else if(ref==='refs/heads/lease-new')cp.execFileSync(${JSON.stringify(gitPath)},['--git-dir',${JSON.stringify(bare)},'update-ref',ref,${JSON.stringify(foreignCommit)}]);}const r=cp.spawnSync(${JSON.stringify(gitPath)},a,{stdio:'inherit'});process.exit(r.status??1);\n`); await chmod(leaseProxy, 0o700);
 const leaseOptions = {gitExecutable: leaseProxy};
 const leasePreview = await previewGitPush(repo, {remote: 'origin', branch: 'lease-before-handshake'}, leaseOptions); assert.equal(leasePreview.relation, 'fast-forward'); assert.equal(leasePreview.remoteOid, base);
 const leaseResult = await pushGitBranch(repo, {remote: 'origin', branch: 'lease-before-handshake', expectedVersion: leasePreview.version}, leaseOptions); assert.equal(leaseResult.ok, false); assert.equal(git(bare, 'rev-parse', leaseRef), next);
 const leaseNew = await previewGitPush(repo, {remote: 'origin', branch: 'lease-new'}, leaseOptions); assert.equal(leaseNew.relation, 'new');
 const createdRace = await pushGitBranch(repo, {remote: 'origin', branch: 'lease-new', expectedVersion: leaseNew.version}, leaseOptions); assert.equal(createdRace.ok, false); assert.equal(git(bare, 'rev-parse', 'refs/heads/lease-new'), foreignCommit);
 if (process.platform !== 'win32') {
  const late = join(directory, 'LATE'), childPid = join(directory, 'CHILD_PID');
  await writeFile(hook, `#!/usr/bin/env node\nconst cp=require('node:child_process'),fs=require('node:fs');const c=cp.spawn(process.execPath,['-e',${JSON.stringify(`setTimeout(()=>require('node:fs').writeFileSync(${JSON.stringify(late)},'orphan'),4000)`)}],{stdio:'inherit'});fs.writeFileSync(${JSON.stringify(childPid)},String(c.pid));setTimeout(()=>{},30000);\n`); await chmod(hook, 0o700);
  const timed = await previewGitPush(repo, {remote: 'origin', branch: 'timeout'}); const timeout = await pushGitBranch(repo, {remote: 'origin', branch: 'timeout', expectedVersion: timed.version}, {timeoutMs: 1500}); assert.equal(timeout.pushed, 'unknown'); assert.equal(timeout.ok, false); await writeFile(join(directory, 'timeout-result.json'), JSON.stringify(timeout));
  const pid = Number(await readFile(childPid, 'utf8')); await new Promise(resolve => setTimeout(resolve, 4500)); assert.throws(() => process.kill(pid, 0)); await assert.rejects(access(late)); await rm(hook);
 }
 const maliciousDirectory = join(repo, 'bin'), maliciousMarker = join(directory, 'REPOSITORY_GIT_EXECUTED'); await mkdir(maliciousDirectory);
 const maliciousGit = join(maliciousDirectory, 'git'); await writeFile(maliciousGit, `#!/bin/sh\nprintf bad > '${maliciousMarker}'\nexit 1\n`); await chmod(maliciousGit, 0o700);
 process.env.PATH = maliciousDirectory + ':' + old.PATH;
 await listGitRemotes(repo); await assert.rejects(access(maliciousMarker));
 await assert.rejects(previewGitPush(repo, target, {gitExecutable: maliciousGit}), (error: any) => error.code === 'UNTRUSTED_EXECUTABLE');
 if (old.PATH === undefined) delete process.env.PATH; else process.env.PATH = old.PATH;
 git(repo, 'remote', 'add', 'secret', 'https://user:secret@github.com/a/b.git?token=abc'); const remotes = await listGitRemotes(repo); assert.ok(JSON.stringify(remotes).includes('[redacted]')); assert.ok(!JSON.stringify(remotes).includes('user:secret')); assert.ok(!redactRemoteText('https://user:token@github.com/a/b?access_token=hidden').includes('hidden'));
 const diagnostics = 'HTTP 401 Unauthorized\nAuthorization: Bearer fixture-private-access-token\nAuthorization: Basic fixture-private-basic-value\nURL https://user:fixture-private@remaining-secret@github.com/owner/repo?access_token=fixture-query-value\nTransport rejected credentials';
 const safe = redactRemoteText(diagnostics);
 assert.ok(!redactRemoteText("https://user:fixture'quoted-secret@github.com/owner/repo").includes('quoted-secret'));
 assert.ok(!redactRemoteText('Authorization: Bearer\n folded-private-secret\nHTTP 401').includes('folded-private-secret'));
 assert.equal(redactRemoteText(safe), safe);
 for (const secret of ['fixture-private-access-token', 'fixture-private-basic-value', 'fixture-private', 'remaining-secret', 'fixture-query-value']) assert.ok(!safe.includes(secret));
 assert.ok(safe.includes('HTTP 401 Unauthorized')); assert.ok(safe.includes('Transport rejected credentials')); assert.ok(safe.includes('https://[redacted]@github.com/owner/repo'));
 await assert.rejects(remoteCommand(process.execPath, ['-e', `process.stderr.write(${JSON.stringify(diagnostics)});process.exit(1)`], directory), (error: any) => {
  assert.ok(error.message.includes('HTTP 401 Unauthorized')); assert.ok(error.message.includes('Transport rejected credentials'));
  return ['fixture-private-access-token', 'fixture-private-basic-value', 'fixture-private', 'remaining-secret', 'fixture-query-value'].every(secret => !error.message.includes(secret));
 });
 await writeFile(join(directory, 'receipt.json'), JSON.stringify({pass: true, localBareOnly: true, newBranch: true, fastForward: true, rejectingHook: true, staleHeadAndConfig: true, remoteConcurrencyRejected: true, exactOldRefLeaseBeforeHandshake: true, concurrentNewBranchPreserved: true, divergedAndUnknownNeverStartPush: true, replaceAndGraftsCannotForgeAncestry: true, exactCommitDespiteHeadSwitch: true, timeoutUnknownAndTreeCleaned: process.platform !== 'win32', nativeTrackingRefsMayUpdate: true, fullAuthorizationAndUserinfoRedacted: true, realStderrRedaction: true}, null, 2));
 console.log('GIT_REMOTES_OK', directory);
} finally {for (const [key, value] of Object.entries(old)) {if (value === undefined) delete process.env[key]; else process.env[key] = value;}}
