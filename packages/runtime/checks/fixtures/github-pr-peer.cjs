/** Controlled gh protocol peer. Never accesses a network or account. */
const fs = require('node:fs');
const statePath = process.env.HIVE_GH_FIXTURE_STATE;
if (!statePath) throw Error('fixture state is required');
const state = JSON.parse(fs.readFileSync(statePath, 'utf8')), args = process.argv.slice(2);
function save() {fs.writeFileSync(statePath, JSON.stringify(state));}
function fail(text) {process.stderr.write(text); process.exit(1);}
if (process.env.GH_REPO || process.env.GH_HOST || process.env.GIT_DIR || process.env.GIT_WORK_TREE || process.env.GIT_CONFIG_COUNT) fail('inherited repository routing was not cleared');
state.calls ??= []; state.calls.push({args, cwd: process.cwd()}); save();
if (state.mode === 'auth-failure') fail('HTTP 401 Unauthorized\nAuthorization: Bearer fixture-private-access-token\nProxy-Authorization: Basic fixture-private-basic-value\nnot logged in: https://user:fixture-secret@remaining-secret@github.com/owner/repo?access_token=fixture-query\nGitHub authentication failed');
const flag = name => args[args.indexOf(name) + 1];
const repo = 'github.com/owner/repo';
const pr = () => ({number: 7, url: 'https://github.com/owner/repo/pull/7', title: state.title ?? 'existing', body: state.body ?? 'body', headRefName: state.head ?? 'published', baseRefName: 'main', headRefOid: state.remoteOid, headRepositoryOwner: {login: 'owner'}, headRepository: {name: 'repo'}, isCrossRepository: state.mode === 'fork', isDraft: state.isDraft ?? false});
if (args[0] === 'repo' && args[1] === 'view') {
 if (args[2] !== 'https://github.com/owner/repo' || args.includes('--repo')) fail('repository view must use supported explicit positional URL');
 console.log(JSON.stringify({nameWithOwner: state.mode === 'wrong-repo' ? 'wrong/repo' : 'owner/repo', url: 'https://github.com/owner/repo'}));
} else if (args[0] === 'api') {
 if (flag('--hostname') !== 'github.com' || args.at(-1) !== 'repos/owner/repo/git/ref/heads/main') fail('wrong API route');
 console.log(JSON.stringify({ref: 'refs/heads/main', object: {sha: state.baseOid}}));
} else if (args[0] === 'pr') {
 if (flag('--repo') !== repo) fail('wrong explicit repository');
 if (['list', 'view'].includes(args[1]) && !flag('--json')?.split(',').includes('isDraft')) fail('draft status must be requested');
 if (args[1] === 'list') {
  if (flag('--head') !== (state.head ?? 'published') || flag('--base') !== 'main' || flag('--state') !== 'open') fail('wrong PR list branch filter');
  console.log(JSON.stringify(state.exists ? [pr()] : []));
 } else if (args[1] === 'create' || args[1] === 'edit') {
  if (args[1] === 'create' && (flag('--head') !== (state.head ?? 'published') || flag('--base') !== 'main')) fail('explicit head/base must disable gh push/fork prompts');
  if (args[1] === 'edit' && args[2] !== '7') fail('wrong PR number');
  if (args.includes('--body') || args.includes('--fill') || args.includes('--web')) fail('unexpected implicit input');
  const file = flag('--body-file'); if (!file || (fs.statSync(file).mode & 0o777) !== 0o600) fail('private body file required');
  if (args[1] === 'edit' && args.includes('--draft')) fail('edit must preserve draft status');
  if (args[1] === 'create') state.isDraft = args.includes('--draft');
  if (state.mode === 'unexpected-draft-state') state.isDraft = !state.isDraft;
  state.title = flag('--title'); state.body = fs.readFileSync(file, 'utf8'); state.bodyFile = file; state.exists = true; save();
  if (state.mode === 'nonzero-after-write') fail('partial response at https://user:fixture-secret@github.com/owner/repo?token=fixture-query');
  if (state.mode === 'timeout') {setTimeout(() => {}, 30000); return;}
  console.log(state.mode === 'wrong-url' ? 'https://github.com/wrong/repo/pull/7' : 'https://github.com/owner/repo/pull/7');
 } else if (args[1] === 'view') {if (args[2] !== '7') fail('wrong PR view number'); console.log(JSON.stringify(pr()));}
 else fail('unexpected PR command');
} else fail('unexpected gh command');
