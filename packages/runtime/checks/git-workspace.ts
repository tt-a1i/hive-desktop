/** Actual temporary Git repositories only. No model, account, fetch or hosted service. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, writeFile, readFile, realpath, rm, chmod, symlink, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { getGitWorkspaceStatus, getGitFileDiff, GitWorkspaceError } from '../src/git-workspace.js';
const exec=promisify(execFile),directory=await realpath(await mkdtemp(join(tmpdir(),'hive-git-read-')));
const previousGlobal=process.env.GIT_CONFIG_GLOBAL,previousSystem=process.env.GIT_CONFIG_SYSTEM,previousNoSystem=process.env.GIT_CONFIG_NOSYSTEM;
const globalConfig=join(directory,'global.gitconfig'),systemConfig=join(directory,'system.gitconfig');await writeFile(globalConfig,'');await writeFile(systemConfig,'');
process.env.GIT_CONFIG_GLOBAL=globalConfig;process.env.GIT_CONFIG_SYSTEM=systemConfig;delete process.env.GIT_CONFIG_NOSYSTEM;
const gitPath=(await exec('which',['git'])).stdout.trim();
const env={...process.env,GIT_CONFIG_GLOBAL:globalConfig,GIT_CONFIG_SYSTEM:systemConfig,GIT_TERMINAL_PROMPT:'0'};
for(const key of Object.keys(env))if(key.startsWith('GIT_')&&!['GIT_CONFIG_NOSYSTEM','GIT_CONFIG_GLOBAL','GIT_CONFIG_SYSTEM','GIT_TERMINAL_PROMPT'].includes(key))delete (env as Record<string,string|undefined>)[key];
async function git(cwd:string,...args:string[]){return exec(gitPath,['-c','user.name=Fixture','-c','user.email=fixture@example.invalid',...args],{cwd,env,maxBuffer:8*1024*1024});}
async function init(path:string){await mkdir(path,{recursive:true});await git(path,'init','--initial-branch=main');}
async function put(root:string,path:string,text:string|Buffer){await mkdir(dirname(join(root,path)),{recursive:true});await writeFile(join(root,path),text);}
async function commit(root:string,message='fixture'){await git(root,'add','--all');await git(root,'commit','-m',message);}
const isCode=(code:string)=>(error:unknown)=>error instanceof GitWorkspaceError&&error.code===code;
try{
 const repo=join(directory,'仓库 space\nroot');await init(repo);
 await put(repo,'old name.txt','alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\n');
 await put(repo,'delete.txt','remove me\n');await put(repo,'edit.txt','before\n');await put(repo,'binary.dat',Buffer.from([0,1,2,3]));
 await put(repo,'project/inside.txt','project before\n');await put(repo,'outside.txt','outside before\n');await put(repo,'.gitignore','ignored\n');
 await commit(repo);
 // Repository, user-global and system ignore rules agree with ordinary Git.
 const globalIgnore=join(directory,'global-ignore'),systemIgnore=join(directory,'system-ignore');await writeFile(globalIgnore,'global-only\n');await writeFile(systemIgnore,'system-only\n');
 await git(repo,'config','--file',systemConfig,'core.excludesFile',systemIgnore);await put(repo,'system-only','ignored by system');
 assert.ok(!(await getGitWorkspaceStatus(repo)).entries.some(entry=>entry.path==='system-only'));
 await git(repo,'config','--file',globalConfig,'core.excludesFile',globalIgnore);await put(repo,'global-only','ignored by global');
 const globalStatus=await getGitWorkspaceStatus(repo);assert.equal(globalStatus.globalConfiguration,'read-only');assert.ok(!globalStatus.entries.some(entry=>entry.path==='global-only'));assert.ok(globalStatus.entries.some(entry=>entry.path==='system-only'));
 const localIgnore=join(directory,'local-ignore');await writeFile(localIgnore,'local-only\n');await git(repo,'config','core.excludesFile',localIgnore);await put(repo,'local-only','ignored by repo override');
 const overridden=await getGitWorkspaceStatus(repo);assert.ok(overridden.entries.some(entry=>entry.path==='global-only'));assert.ok(!overridden.entries.some(entry=>entry.path==='local-only'));
 await git(repo,'config','--unset','core.excludesFile');await rm(join(repo,'global-only'));await rm(join(repo,'system-only'));await rm(join(repo,'local-only'));
 const clean=await getGitWorkspaceStatus(repo);assert.equal(clean.branch,'main');assert.equal(clean.detached,false);assert.equal(clean.unborn,false);assert.deepEqual(clean.entries,[]);assert.equal(clean.repositoryRoot,repo);
 const renamed='重命名\nnew name.txt';await git(repo,'mv','old name.txt',renamed);await put(repo,renamed,'alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\nstaged\n');await git(repo,'add','--',renamed);await put(repo,renamed,'alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\nstaged\nworking\n');
 await put(repo,'edit.txt','staged edit\n');await git(repo,'add','--','edit.txt');await put(repo,'edit.txt','working edit\n');
 await rm(join(repo,'delete.txt'));await put(repo,'binary.dat',Buffer.from([0,9,8,7]));
 await put(repo,'空 格\nnotes.txt','untracked Unicode newline\n');await put(repo,'-leading','leading name\n');await put(repo,'ignored','ignore this');await put(repo,'large.txt','x'.repeat(2048));
 const status=await getGitWorkspaceStatus(repo,{limit:500});assert.equal(status.complete,true);
 const rename=status.entries.find(entry=>entry.path===renamed)!;assert.equal(rename.kind,'rename');assert.equal(rename.originalPath,'old name.txt');assert.equal(rename.indexStatus,'R');assert.equal(rename.worktreeStatus,'M');
 const edit=status.entries.find(entry=>entry.path==='edit.txt')!;assert.equal(edit.indexStatus,'M');assert.equal(edit.worktreeStatus,'M');
 assert.ok(status.entries.some(entry=>entry.path==='空 格\nnotes.txt'&&entry.kind==='untracked'));assert.ok(status.entries.some(entry=>entry.path==='-leading'));
 assert.ok(status.entries.some(entry=>entry.path==='delete.txt'&&entry.worktreeStatus==='D'));assert.ok(!status.entries.some(entry=>entry.path==='ignored'));
 await git(repo,'update-index','--split-index');
 const indexBefore=await readFile(join(repo,'.git','index'));await getGitWorkspaceStatus(repo);assert.deepEqual(await readFile(join(repo,'.git','index')),indexBefore,'Read-only refresh must not rewrite the real split index');
 const page=await getGitWorkspaceStatus(repo,{offset:1,limit:2});assert.equal(page.total,status.total);assert.equal(page.entries.length,2);assert.equal(page.hasMore,true);
 assert.deepEqual((await getGitWorkspaceStatus(repo,{offset:999,limit:2})).entries,[]);
 const staged=await getGitFileDiff(repo,'edit.txt',{comparison:'staged'});assert.equal(staged.before.text,'before\n');assert.equal(staged.after.text,'staged edit\n');assert.ok(staged.patch?.includes('+staged edit'));
 const unstaged=await getGitFileDiff(repo,'edit.txt');assert.equal(unstaged.before.text,'staged edit\n');assert.equal(unstaged.after.text,'working edit\n');
 const renameDiff=await getGitFileDiff(repo,renamed,{comparison:'staged'});assert.equal(renameDiff.before.text,'alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\n');assert.ok(renameDiff.after.text?.includes('staged'));
 const untracked=await getGitFileDiff(repo,'空 格\nnotes.txt');assert.equal(untracked.before.kind,'missing');assert.ok(untracked.patch?.includes('+untracked Unicode newline'));
 const deleted=await getGitFileDiff(repo,'delete.txt');assert.equal(deleted.after.kind,'missing');assert.ok(deleted.patch?.includes('-remove me'));
 const binary=await getGitFileDiff(repo,'binary.dat');assert.equal(binary.binary,true);assert.equal(binary.patch,undefined);assert.equal(binary.after.bytes,4);
 const combined=await getGitFileDiff(repo,'edit.txt',{comparison:'head-worktree'});assert.equal(combined.before.text,'before\n');assert.equal(combined.after.text,'working edit\n');
 await assert.rejects(getGitFileDiff(repo,'large.txt',{maxFileBytes:100}),isCode('FILE_LIMIT'));
 await assert.rejects(getGitFileDiff(repo,'edit.txt',{maxDiffBytes:10}),isCode('OUTPUT_LIMIT'));
 await assert.rejects(getGitWorkspaceStatus(repo,{limit:501}),isCode('INVALID_ARGUMENT'));
 await assert.rejects(getGitWorkspaceStatus(repo,{gitExecutable:join(directory,'missing-git')}),isCode('GIT_NOT_FOUND'));
 const plain=join(directory,'not-a-repo');await mkdir(plain);await assert.rejects(getGitWorkspaceStatus(plain),isCode('NOT_REPOSITORY'));
 // Project scopes can live below the actual repository root; they never expose parent bytes.
 await put(repo,'project/inside.txt','project working\n');await put(repo,'outside.txt','outside working\n');
 const sub=await getGitWorkspaceStatus(join(repo,'project'));assert.equal(sub.projectPrefix,'project/');assert.equal(sub.repositoryRoot,repo);assert.deepEqual(sub.entries.map(entry=>entry.path),['inside.txt']);
 assert.equal((await getGitFileDiff(join(repo,'project'),'inside.txt')).before.text,'project before\n');
 for(const path of ['../outside.txt','.git/config','nested/../../outside.txt','/etc/passwd'])await assert.rejects(getGitFileDiff(join(repo,'project'),path),isCode('INVALID_PATH'));
 await symlink(repo,join(repo,'project','escape'));await assert.rejects(getGitFileDiff(join(repo,'project'),'escape/outside.txt'),isCode('UNSAFE_PATH'));
 await symlink('../outside.txt',join(repo,'project','link'));const link=await getGitFileDiff(join(repo,'project'),'link');assert.equal(link.after.kind,'symlink');assert.equal(link.after.text,'../outside.txt');
 const parent=join(directory,'parent-repository');await init(parent);await put(parent,'project/leaving.txt','leaving content\n');await put(parent,'outside/entering.txt','entering content\n');await commit(parent);
 await git(parent,'mv','project/leaving.txt','outside/left.txt');await git(parent,'mv','outside/entering.txt','project/entered.txt');
 const crossing=await getGitWorkspaceStatus(join(parent,'project'));assert.equal(crossing.entries.length,2);
 assert.equal(crossing.entries.find(entry=>entry.path==='leaving.txt')!.pathRole,'rename-source');assert.equal(crossing.entries.find(entry=>entry.path==='entered.txt')!.pathRole,'target');
 await assert.rejects(getGitFileDiff(join(parent,'project'),'entered.txt',{comparison:'staged'}),isCode('OUTSIDE_PROJECT_RENAME'));
 await assert.rejects(getGitFileDiff(join(parent,'project'),'leaving.txt',{comparison:'staged'}),isCode('OUTSIDE_PROJECT_RENAME'));
 assert.equal((await getGitFileDiff(parent,'project/entered.txt',{comparison:'staged'})).before.text,'entering content\n');
 // Unborn repositories and linked worktrees preserve actual Git identity.
 const unborn=join(directory,'unborn');await init(unborn);await put(unborn,'added.txt','first staged\n');await git(unborn,'add','--','added.txt');
 const initial=await getGitWorkspaceStatus(unborn);assert.equal(initial.unborn,true);assert.equal(initial.headOid,null);assert.equal(initial.branch,'main');assert.equal(initial.entries[0]!.indexStatus,'A');
 assert.equal((await getGitFileDiff(unborn,'added.txt',{comparison:'staged'})).before.kind,'missing');
 const worktree=join(directory,'linked tree');await git(repo,'worktree','add','--detach',worktree,'HEAD');await put(worktree,'edit.txt','linked worktree edit\n');
 const linked=await getGitWorkspaceStatus(worktree);assert.equal(linked.linkedWorktree,true);assert.equal(linked.detached,true);assert.equal(linked.branch,null);assert.equal(linked.repositoryRoot,worktree);assert.notEqual(linked.gitDir,linked.commonDir);assert.equal(linked.entries[0]!.path,'edit.txt');
 await git(repo,'checkout','--detach','HEAD');assert.equal((await getGitWorkspaceStatus(repo)).detached,true);await git(repo,'checkout','main');
 // Actual unresolved merge: all index stages and working conflict text remain inspectable.
 const conflict=join(directory,'conflict');await init(conflict);await put(conflict,'conflict.txt','base\n');await commit(conflict);await git(conflict,'checkout','-b','other');await put(conflict,'conflict.txt','other\n');await commit(conflict);await git(conflict,'checkout','main');await put(conflict,'conflict.txt','main\n');await commit(conflict);
 await assert.rejects(git(conflict,'merge','other'));
 const conflicted=await getGitWorkspaceStatus(conflict);assert.equal(conflicted.entries[0]!.kind,'unmerged');assert.equal(conflicted.entries[0]!.indexStatus,'U');assert.equal(conflicted.entries[0]!.worktreeStatus,'U');assert.equal(conflicted.entries[0]!.conflictStages!.length,3);
 const conflictDiff=await getGitFileDiff(conflict,'conflict.txt');assert.equal(conflictDiff.complete,false);assert.equal(conflictDiff.patch,undefined);assert.deepEqual(conflictDiff.conflictStages!.map(stage=>stage.snapshot.text),['base\n','main\n','other\n']);assert.ok(conflictDiff.after.text?.includes('<<<<<<<'));
 // Every configured helper writes a sentinel if invoked. Only read APIs run now.
 const sentinel=join(directory,'HELPER_RAN'),helper=join(directory,'helper');
 await writeFile(helper,`#!/bin/sh\nprintf invoked > '${sentinel}'\nexit 0\n`);await chmod(helper,0o755);
 // Staged gitlink commits remain changes even while nested file scans are disabled.
 const moduleSource=join(directory,'module-source'),gitlinkParent=join(directory,'gitlink-parent');
 await init(moduleSource);await put(moduleSource,'module.txt','commit A\n');await commit(moduleSource,'module A');const commitA=(await git(moduleSource,'rev-parse','HEAD')).stdout.trim();
 await put(moduleSource,'module.txt','commit B\n');await commit(moduleSource,'module B');const commitB=(await git(moduleSource,'rev-parse','HEAD')).stdout.trim();
 await init(gitlinkParent);await put(gitlinkParent,'parent.txt','parent\n');await commit(gitlinkParent);
 await git(gitlinkParent,'-c','protocol.file.allow=always','submodule','add',moduleSource,'module');
 const moduleWorktree=join(gitlinkParent,'module');await git(moduleWorktree,'checkout','--detach',commitA);await commit(gitlinkParent,'gitlink A');
 await git(gitlinkParent,'update-index','--cacheinfo',`160000,${commitB},module`);
 const pointerAtA=await getGitWorkspaceStatus(gitlinkParent);const atA=pointerAtA.entries.find(entry=>entry.path==='module')!;
 assert.ok(atA,'Staged gitlink pointer cannot be reported clean');assert.equal(atA.indexStatus,'M');assert.equal(atA.headOid,commitA);assert.equal(atA.indexOid,commitB);assert.equal(atA.headMode,'160000');assert.equal(atA.indexMode,'160000');
 await git(moduleWorktree,'checkout','--detach',commitB);
 // A nested filter/fsmonitor would write the same sentinel if a content scan ran.
 await git(moduleWorktree,'config','core.fsmonitor',helper);await git(moduleWorktree,'config','filter.sentinel.clean',helper);await git(moduleWorktree,'config','filter.sentinel.process',helper);
 await put(moduleWorktree,'.gitattributes','*.txt filter=sentinel\n');await put(moduleWorktree,'module.txt','nested tracked dirt\n');await put(moduleWorktree,'untracked.txt','nested untracked dirt\n');
 await git(gitlinkParent,'config','submodule.module.ignore','all');
 const pointerAtB=await getGitWorkspaceStatus(gitlinkParent);const atB=pointerAtB.entries.find(entry=>entry.path==='module')!;
 assert.equal(atB.indexStatus,'M');assert.equal(atB.worktreeStatus,'.');assert.equal(pointerAtB.submoduleDetails,'not-scanned');await assert.rejects(access(sentinel));
 const pointerDiff=await getGitFileDiff(gitlinkParent,'module',{comparison:'staged'});assert.equal(pointerDiff.before.oid,commitA);assert.equal(pointerDiff.after.oid,commitB);assert.equal(pointerDiff.before.kind,'submodule');assert.equal(pointerDiff.after.kind,'submodule');
 await rm(moduleWorktree,{recursive:true,force:true});
 const pointerMissing=await getGitWorkspaceStatus(gitlinkParent);const absent=pointerMissing.entries.find(entry=>entry.path==='module')!;
 assert.ok(absent,'Missing checkout cannot hide staged gitlink change');assert.equal(absent.indexStatus,'M');assert.equal(absent.headOid,commitA);assert.equal(absent.indexOid,commitB);
 const missingPointerDiff=await getGitFileDiff(gitlinkParent,'module',{comparison:'staged'});assert.equal(missingPointerDiff.before.oid,commitA);assert.equal(missingPointerDiff.after.oid,commitB);await assert.rejects(access(sentinel));
 const hooks=join(directory,'hooks');await mkdir(hooks);for(const name of ['post-index-change','pre-commit','post-checkout']){await writeFile(join(hooks,name),`#!/bin/sh\nprintf hook > '${sentinel}'\n`);await chmod(join(hooks,name),0o755);}
 await git(repo,'config','alias.status',`!${helper}`);await git(repo,'config','alias.diff',`!${helper}`);await git(repo,'config','diff.sentinel.command',helper);
 await git(repo,'config','core.fsmonitor',helper);await git(repo,'config','core.hooksPath',hooks);await git(repo,'config','diff.external',helper);await git(repo,'config','diff.sentinel.textconv',helper);await put(repo,'.gitattributes','*.txt diff=sentinel filter=sentinel\n');
 await git(repo,'config','filter.sentinel.clean',helper);await git(repo,'config','filter.sentinel.smudge',helper);await git(repo,'config','filter.sentinel.process',helper);await git(repo,'config','filter.sentinel.required','true');
 await assert.rejects(getGitWorkspaceStatus(repo),isCode('UNSUPPORTED_FILTER'));
 assert.equal((await getGitFileDiff(repo,'edit.txt')).after.text,'working edit\n');assert.equal((await getGitFileDiff(repo,'edit.txt',{comparison:'staged'})).after.text,'staged edit\n');
 await assert.rejects(access(sentinel));
 await git(repo,'config','--remove-section','filter.sentinel');await getGitWorkspaceStatus(repo);await getGitFileDiff(repo,'edit.txt');await assert.rejects(access(sentinel));
 // Missing promisor object must fail locally, never start a transport helper.
 const partial=join(directory,'partial');await init(partial);await put(partial,'partial.txt','missing blob content\n');await commit(partial);
 const blob=(await git(partial,'rev-parse','HEAD:partial.txt')).stdout.trim();await rm(join(partial,'.git','objects',blob.slice(0,2),blob.slice(2)));
 await git(partial,'config','protocol.ext.allow','always');await git(partial,'config','protocol.allow','always');await git(partial,'config','extensions.partialClone','origin');await git(partial,'config','remote.origin.promisor','true');await git(partial,'config','remote.origin.url',`ext::${helper}`);
 await assert.rejects(getGitFileDiff(partial,'partial.txt',{comparison:'staged'}),isCode('GIT_COMMAND_FAILED'));await assert.rejects(access(sentinel));
 // An actor changing the actual index after status must invalidate the result.
 const racing=join(directory,'racing');await init(racing);await put(racing,'file.txt','before\n');await commit(racing);await put(racing,'file.txt','after\n');
 const proxy=join(directory,'git-proxy');await writeFile(proxy,`#!/usr/bin/env node\nconst{execFileSync}=require('node:child_process');const args=process.argv.slice(2);const out=execFileSync(${JSON.stringify(gitPath)},args,{stdio:['ignore','pipe','inherit']});if(args.includes('status'))execFileSync(${JSON.stringify(gitPath)},['add','--','file.txt']);process.stdout.write(out);\n`);await chmod(proxy,0o755);
 await assert.rejects(getGitWorkspaceStatus(racing,{gitExecutable:proxy}),isCode('CONCURRENT_CHANGE'));
 console.log('GIT_WORKSPACE_READ_OK: real Git ordinary/staged/unstaged/rename/delete/untracked/binary/conflict/unborn/detached/worktree/project scopes; Unicode/newline paths; pagination/limits/errors; helper and lazy-fetch sentinels; concurrent index detection.');
}finally{
 if(previousGlobal===undefined)delete process.env.GIT_CONFIG_GLOBAL;else process.env.GIT_CONFIG_GLOBAL=previousGlobal;
 if(previousSystem===undefined)delete process.env.GIT_CONFIG_SYSTEM;else process.env.GIT_CONFIG_SYSTEM=previousSystem;
 if(previousNoSystem===undefined)delete process.env.GIT_CONFIG_NOSYSTEM;else process.env.GIT_CONFIG_NOSYSTEM=previousNoSystem;
 await rm(directory,{recursive:true,force:true});
}
