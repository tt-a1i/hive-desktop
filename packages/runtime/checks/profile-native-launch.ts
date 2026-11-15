/** Controlled subprocesses only; no native agent CLI, credentials or inference. */
import assert from 'node:assert/strict';
import { mkdtemp, realpath, writeFile, readFile, chmod, mkdir, copyFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CodexTransport } from '../src/agents/codex-transport.js';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';
import { createAgentSession, nativeProfileLaunch } from '../src/agent-session-factory.js';
import type { ResolvedAgentProfile } from '../src/agent-profiles.js';
import { createClaudeSession } from '../src/agents/claude.js';
const root = await realpath(await mkdtemp(join(tmpdir(), 'hive profile native ')));
const before = { ...process.env };
const profile: ResolvedAgentProfile = { ref: { id: 'test-profile', revision: 1 }, driver: 'codex', label: 'Test', enabled: true,
  launch: { command: '/nonexistent/profile command', args: ['--profile', 'space name'], configDir: join(root, 'native config') } };
assert.deepEqual(nativeProfileLaunch(profile).args, ['--profile', 'space name']);
assert.equal(nativeProfileLaunch(profile).env.CODEX_HOME, profile.launch.configDir);
assert.throws(() => nativeProfileLaunch({ ...profile, driver: 'pi' }), /Built-in Pi/);
assert.throws(() => nativeProfileLaunch({ ...profile, driver: 'claude' }), /unsupported/);
await assert.rejects(createAgentSession({ backend: 'pi', profile }), /does not match/);
await assert.rejects(createAgentSession({ backend: 'codex', profile: { ...profile, enabled: false } }), /disabled/);
await assert.rejects(CodexTransport.start({ cwd: root, args: ['--listen=tcp://wrong'] }), /managed stdio/);
for (const arg of ['--session', '--session=wrong', '--session-dir', '--mode=print', '--extension', '-e', '--no-session', '--continue', '-r', '--', '--no-extensions']) {
  await assert.rejects(ExternalPiTransport.start({ cwd: root, sessionDir: root, extensionPath: '', args: [arg] }), /managed RPC/);
}
const peer = join(root, 'native peer.cjs');
await writeFile(peer, `#!/usr/bin/env node
const readline=require('node:readline');
const args=process.argv.slice(2), extension=args[args.indexOf('--extension')+1];
const info={args,codexHome:process.env.CODEX_HOME,piDir:process.env.PI_CODING_AGENT_DIR,bridge:process.env.HIVE_PI_BRIDGE_COMMAND,team:process.env.HIVE_PI_TEAM_TOOL,pid:process.pid};
readline.createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);const send=v=>process.stdout.write(JSON.stringify(v)+'\\n');
if(x.method){if(x.method==='initialized')return;send({id:x.id,result:x.method==='fixture/info'?info:{}});return;}
let result= x.type==='get_commands'?{commands:[{name:info.bridge,source:'extension',sourceInfo:{path:extension}}]}:x.type==='prompt'?{disposition:'handled'}:info;
send({type:'response',command:x.type,id:x.id,success:true,data:result});
});
`);
await chmod(peer, 0o755);
const peerB = join(root, 'second command.cjs'); await copyFile(peer, peerB); await chmod(peerB, 0o755);
for (const driver of ['codex', 'pi-rpc'] as const) {
  const configA = join(root, driver, 'config A'), configB = join(root, driver, 'config B');
  await Promise.all([mkdir(configA, { recursive: true }), mkdir(configB, { recursive: true })]);
  const start = (command: string, configDir: string, tag: string) => driver === 'codex'
    ? CodexTransport.start({ cwd: root, command, configDir, args: ['--profile-tag', tag] })
    : ExternalPiTransport.start({ cwd: root, command, configDir, args: ['--profile-tag', tag], sessionDir: root, extensionPath: peer,
        env: { HIVE_PI_BRIDGE_COMMAND: 'must-not-win', HIVE_PI_TEAM_TOOL: 'must-not-win' } });
  const info = (transport: CodexTransport | ExternalPiTransport) => transport instanceof CodexTransport
    ? transport.sendRequest<Record<string, any>>('fixture/info', {}) : transport.request<Record<string, any>>('get_state');
  const [a, b] = await Promise.all([start(peer, configA, 'A with space'), start(peerB, configB, 'B with space')]);
  try {
    const [ia, ib] = await Promise.all([info(a), info(b)]);
    assert.notEqual(ia.pid, ib.pid);
    assert.equal(ia[driver === 'codex' ? 'codexHome' : 'piDir'], configA);
    assert.equal(ib[driver === 'codex' ? 'codexHome' : 'piDir'], configB);
    assert.deepEqual(ia.args.slice(0, 2), ['--profile-tag', 'A with space']);
    assert.deepEqual(ib.args.slice(0, 2), ['--profile-tag', 'B with space']);
    if (driver === 'codex') assert.deepEqual(ia.args.slice(2), ['app-server', '--listen', 'stdio://']);
    else { assert.notEqual(ia.bridge, 'must-not-win'); assert.notEqual(ia.bridge, ib.bridge); assert.notEqual(ia.team, 'must-not-win'); }
    await a.close(); assert.equal((await info(b)).pid, ib.pid);
    const restored = await start(peer, configA, 'A restored');
    try { assert.equal((await info(restored))[driver === 'codex' ? 'codexHome' : 'piDir'], configA); }
    finally { await restored.close(); }
    assert.ok(!JSON.stringify(a.lifecycleDiagnostics).includes(configA));
  } finally { await Promise.all([a.close(), b.close()]); }
}
// SDK-controlled Claude peers write native JSONL into two isolated config roots.
const claudePeer = fileURLToPath(new URL('./fixtures/claude-modes-peer.cjs', import.meta.url));
const configs = [join(root, 'claude A'), join(root, 'claude B')];
const sessions = await Promise.all(configs.map(async (configDir, index) => {
  const control = join(root, `control ${index}.json`), receipt = join(root, `wire ${index}.jsonl`);
  await writeFile(control, '{}');
  return createClaudeSession({ cwd: root, sessionDir: join(root, `metadata ${index}`), configDir, command: claudePeer,
    env: { HIVE_CLAUDE_MODES_CONTROL: control, HIVE_CLAUDE_MODES_WIRE: receipt }, onEvent() {} });
}));
try {
  await Promise.all(sessions.map((session, index) => session.addContext(`isolated context ${index}`, 'profile-test')));
  assert.notEqual(sessions[0]!.processId, sessions[1]!.processId);
  for (const [index, session] of sessions.entries()) {
    const path = join(configs[index]!, 'projects', root.replace(/[^a-zA-Z0-9]/g, '-'), `${session.sessionId}.jsonl`);
    assert.match(await readFile(path, 'utf8'), new RegExp(`isolated context ${index}`));
    assert.ok(!JSON.stringify(session.snapshot()).includes(`isolated context ${1 - index}`));
  }
  await sessions[0]!.dispose();
  await sessions[1]!.addContext('B after A closes', 'profile-test');
  const restored = await createClaudeSession({ cwd: root, sessionDir: join(root, 'metadata 0'), configDir: configs[0]!, sessionId: sessions[0]!.sessionId,
    command: claudePeer, env: { HIVE_CLAUDE_MODES_CONTROL: join(root, 'control 0.json'), HIVE_CLAUDE_MODES_WIRE: join(root, 'wire 0.jsonl') }, onEvent() {} });
  try { assert.ok(JSON.stringify(restored.snapshot()).includes('isolated context 0')); }
  finally { await restored.dispose(); }
} finally { await Promise.all(sessions.map(session => session.dispose())); }
// The official SDK sets its own version marker on first query. Profile/config
// variables and all existing host values must remain unchanged. Do not dump env.
assert.ok(Object.entries(before).every(([key, value]) => process.env[key] === value), 'Existing host environment changed');
assert.ok(Object.keys(process.env).every(key => key in before || key === 'CLAUDE_AGENT_SDK_VERSION'), 'Profile leaked into host environment');
console.log('PASS native profile subprocess argv/config isolation, parallel lifecycle, fixed-directory restoration, official Claude SDK scoped history; no CLI/model requests');
