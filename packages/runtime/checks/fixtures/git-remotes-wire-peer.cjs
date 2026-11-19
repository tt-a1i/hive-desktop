/** JSONL integration peer: real Git for local remotes, simulated network only for github remote/gh. */
const fs = require('node:fs');
const cp = require('node:child_process');
const [kind, ...args] = process.argv.slice(2);
const file = process.env.HIVE_GIT_REMOTE_WIRE_STATE;
if (!file) throw Error('wire peer state required');
const read = () => JSON.parse(fs.readFileSync(file, 'utf8'));
let state = read();
fs.appendFileSync(state.log, JSON.stringify({kind, args, cwd: process.cwd()}) + '\n');
const fail = message => {process.stderr.write(message); process.exit(1);};
if (kind === 'git') {
 if (args.includes('ls-remote') && args.at(-2) === 'github') {
  if (args.at(-1) !== 'refs/heads/session-pr') fail('unexpected simulated GitHub branch');
  if (state.remoteOid) {console.log(state.remoteOid + '\trefs/heads/session-pr'); process.exit(0);}
  process.exit(2);
 }
 if ((args.includes('push') || args.includes('fetch')) && args.includes('github')) fail('fixture forbids real hosted writes');
 const result = cp.spawnSync(state.git, args, {stdio: 'inherit'}); process.exit(result.status ?? 1);
}
if (kind !== 'gh') fail('unexpected wire peer');
if (process.env.GH_REPO || process.env.GH_HOST || process.env.GIT_DIR || process.env.GIT_WORK_TREE || process.env.GIT_CONFIG_COUNT) fail('inherited gh routing not cleared');
if (state.mode === 'auth-failure') fail('HTTP 401 Unauthorized\nAuthorization: Bearer wire-private-token\nhttps://user:wire-private@tail-secret@github.com/fixture-owner/fixture-repo?access_token=wire-private-query\nAuthentication failed');
const flag = name => args[args.indexOf(name) + 1];
const repo = 'github.com/fixture-owner/fixture-repo';
const pr = () => ({number: 17, url: 'https://github.com/fixture-owner/fixture-repo/pull/17', title: state.title, body: state.body,
 headRefName: 'session-pr', baseRefName: 'main', headRefOid: state.remoteOid, headRepositoryOwner: {login: 'fixture-owner'}, headRepository: {name: 'fixture-repo'}, isCrossRepository: false, isDraft: state.isDraft ?? false});
if (args[0] === 'repo' && args[1] === 'view') {
 if (args[2] !== 'https://github.com/fixture-owner/fixture-repo') fail('wrong repo view route');
 console.log(JSON.stringify({nameWithOwner: 'fixture-owner/fixture-repo', url: args[2]}));
} else if (args[0] === 'api') {
 if (flag('--hostname') !== 'github.com' || args.at(-1) !== 'repos/fixture-owner/fixture-repo/git/ref/heads/main') fail('wrong API route');
 console.log(JSON.stringify({ref: 'refs/heads/main', object: {sha: state.baseOid}}));
} else if (args[0] === 'pr') {
 if (flag('--repo') !== repo) fail('wrong explicit PR repository');
 if (args[1] === 'list') {
  if (flag('--head') !== 'session-pr' || flag('--base') !== 'main' || flag('--state') !== 'open') fail('wrong PR list filter');
  console.log(JSON.stringify(state.exists ? [pr()] : []));
 } else if (args[1] === 'create' || args[1] === 'edit') {
  if (args[1] === 'create' && (flag('--head') !== 'session-pr' || flag('--base') !== 'main')) fail('explicit create branches required');
  if (args[1] === 'edit' && args[2] !== '17') fail('wrong edit number');
  if (args[1] === 'edit' && args.includes('--draft')) fail('edit must preserve draft status');
  if (args[1] === 'create') state.isDraft = args.includes('--draft');
  if (state.mode === 'unexpected-draft-state') state.isDraft = !state.isDraft;
  const bodyFile = flag('--body-file');
  if (!bodyFile || (fs.statSync(bodyFile).mode & 0o777) !== 0o600 || args.includes('--body')) fail('private multiline body file required');
  state.title = flag('--title'); state.body = fs.readFileSync(bodyFile, 'utf8'); state.bodyFile = bodyFile; state.exists = true;
  fs.writeFileSync(file, JSON.stringify(state));
  if (state.mode === 'partial') fail('HTTP 502 after write; outcome unknown');
  console.log('https://github.com/fixture-owner/fixture-repo/pull/17');
 } else if (args[1] === 'view') {
  if (args[2] !== '17') fail('wrong PR view number'); console.log(JSON.stringify(pr()));
 } else fail('unexpected PR operation');
} else fail('unexpected gh operation');
