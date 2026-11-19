// Isolated local repository and bundled runtime. No agent, remote, or model.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
const [root, resources, mode = 'seed'] = process.argv.slice(2);
const project = join(root, 'project'), data = join(root, 'state'), pi = join(root, 'offline-pi');
const target = 'z-staged.txt', draft = 'Git preview must preserve this draft';
Object.assign(process.env, { GIT_CONFIG_GLOBAL:'NUL', GIT_CONFIG_NOSYSTEM:'1', GIT_OPTIONAL_LOCKS:'0', PI_OFFLINE:'1', PI_CODING_AGENT_DIR:pi });
const git = (...args) => execFileSync('git', ['-C',project,...args], {env:process.env,stdio:'pipe'});
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
async function snapshot() {
  const names = (await readdir(project)).filter(name => /^(a\d{3}|z-staged)\.txt$/.test(name)).sort();
  return {head:git('rev-parse','HEAD').toString().trim(), indexSha256:hash(await readFile(join(project,'.git','index'))),
    status:git('status','--porcelain=v2','-z').toString('hex'), stagedSha256:hash(git('diff','--cached','--binary')),
    files:Object.fromEntries(await Promise.all(names.map(async name => [name,hash(await readFile(join(project,name)))])))};
}
if (mode === 'seed') {
  await Promise.all([project,data,pi].map(path => mkdir(path,{recursive:true})));
  git('init','--template=','-q');git('config','user.name','Fixture');git('config','user.email','fixture@example.invalid');git('config','commit.gpgSign','false');
  await writeFile(join(project,target),'base\n');
  await Promise.all(Array.from({length:101},(_,i) => writeFile(join(project,`a${String(i).padStart(3,'0')}.txt`),`base ${i}\n`)));
  git('add','--all');git('commit','-qm','private fixture');
  await writeFile(join(project,target),'base\nSTAGED_PRIVATE_FIXTURE\n');git('add',target);
  await Promise.all(Array.from({length:101},(_,i) => writeFile(join(project,`a${String(i).padStart(3,'0')}.txt`),`local ${i}\n`)));
  const {createDesktopRuntime} = await import(pathToFileURL(join(resources,'runtime','dist','desktop-runtime.js')).href);
  const runtime = await createDesktopRuntime(data,()=>{});
  let workspace,chat;
  try {
    workspace = await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Git preview fixture',autostart_orchestrator:false});
    chat = await runtime.chats.create(workspace.id,'pi');runtime.chats.rename(chat.id,'Git preview fixture');
    runtime.hive.saveNavigation(JSON.stringify({workspace:workspace.id,sessions:{[workspace.id]:chat.id}}));
    const status = await runtime.chats.gitStatus(workspace.id,0,100);
    assert.equal(status.status.repositoryStagedTotal,1);assert.equal(status.status.entries.length,100);
    assert.ok(status.status.entries.every(row => row.repositoryPath !== target));
  } finally { await runtime.close(); }
  await writeFile(join(data,'desktop-language.json'),JSON.stringify('en'));
  await mkdir(join(data,'desktop-drafts'),{recursive:true});
  await writeFile(join(data,'desktop-drafts',`session-${chat.id}.json`),JSON.stringify({key:`session:${chat.id}`,input:draft,failed:[]}));
  await writeFile(join(root,'git-preview-seed.json'),JSON.stringify({project,sessionId:chat.id,stagedPath:target,draft,snapshot:await snapshot(),environment:{GIT_CONFIG_GLOBAL:'NUL',GIT_CONFIG_NOSYSTEM:'1',GIT_OPTIONAL_LOCKS:'0'}}));
} else {
  const seed = JSON.parse(await readFile(join(root,'git-preview-seed.json'),'utf8'));
  const actual = await snapshot();
  const saved = JSON.parse(await readFile(join(data,'desktop-drafts',`session-${seed.sessionId}.json`),'utf8'));
  const result = {mode,headUnchanged:actual.head===seed.snapshot.head,indexUnchanged:actual.indexSha256===seed.snapshot.indexSha256,
    stagedUnchanged:actual.stagedSha256===seed.snapshot.stagedSha256,statusUnchanged:actual.status===seed.snapshot.status,
    filesUnchanged:JSON.stringify(actual.files)===JSON.stringify(seed.snapshot.files),draftUnchanged:saved.input===draft};
  await writeFile(join(root,`git-preview-${mode}.json`),JSON.stringify(result));
  if (mode !== 'failure') assert.ok(Object.entries(result).filter(([key])=>key!=='mode').every(([,value])=>value),JSON.stringify(result));
}
