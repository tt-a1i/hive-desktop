import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, readFile, writeFile, copyFile, symlink, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const arg = name => {const at = process.argv.indexOf(name); return at < 0 ? undefined : process.argv[at + 1];};
assert.ok(arg('--bundle'), 'Pass --bundle to an extracted portable package');
const bundle = resolve(arg('--bundle'));
const resources = process.platform === 'darwin' ? join(bundle, 'Hive Desktop.app/Contents/Resources') : join(bundle, 'resources');
const runtime = join(resources, 'runtime');
const node = join(resources, 'node', process.platform === 'darwin' ? 'bin/node' : 'node.exe');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ts = createRequire(join(root, 'packages/runtime/package.json'))('typescript');
const directory = await mkdtemp(join(tmpdir(), 'hive-packaged-functional-'));
const checks = ['bundled-hive-version', 'delivery-queue', 'queued-input-mutations', 'queued-input-failed-rollback', 'queued-input-mutation', 'queued-input-wire',
  'conversation-search', 'conversation-search-codex', 'codex-metadata-recovery', 'codex-metadata-marker', 'session-body-search', 'project-files', 'project-file-references',
  'conversation-search-tools', 'git-index-wire', 'session-origin-index', 'historical-session-identity', 'html-artifacts',
  'codex-desktop-view',
  'member-worktrees-wire', 'project-script-settings', 'codex-limits', 'pty-live-pid', 'interaction-attachments',
  'codex-history-control', 'codex-fork-provenance', 'session-history-wire', 'session-history-rewind', 'checkpoint-restore',
  'agent-profiles', 'structured-launch', 'profile-runtime-binding', 'profile-runtime-wire',
  'profile-settings-atomic', 'profile-claude-offline-history', 'codex-history-profile-launch',
  'acp-session', 'acp-team-stdio', 'acp-team-hive', 'acp-runtime-registration', 'acp-startup-shutdown',
  'profile-executable-probe',
  'chat-index-snapshot-cas', 'handoff-snapshot', 'session-handoffs', 'handoff-delivery',
  'session-archives', 'session-archives-wire', 'pi-vault',
  'handoff-native-receipts', 'session-handoff-runtime', 'handoff-runtime-failures',
  'handoff-runtime-delivery', 'handoff-queue-retraction', 'desktop-updates', 'desktop-update-installer',
  process.platform === 'win32' ? 'session-terminals-windows' : 'session-terminals',
  ...(process.platform === 'win32' ? ['project-scripts-windows'] : []),
  // These fixtures use POSIX executable peers/hooks. The Windows PTY and
  // JSONL Git checks above run natively rather than passing via a skipped peer.
  ...(process.platform === 'win32' ? [] : ['codex-probe-owner', 'pi-vault-owner', 'queued-user-inputs-peers', 'chat-worktrees', 'git-remotes-wire', 'project-scripts-wire', 'profile-native-launch'])];
const receipt = {pass: false, bundle, actualInference: false, checks: []};
try {
  await mkdir(join(directory, 'agent'));
  await mkdir(join(directory, 'fixtures'));
  await copyFile(join(root, 'packages/runtime/checks/fixtures/codex-queue-peer.cjs'), join(directory, 'fixtures/codex-queue-peer.cjs'));
  if (process.platform !== 'win32') {
    await copyFile(join(root, 'packages/runtime/checks/fixtures/claude-reasoning-peer.cjs'), join(directory, 'fixtures/claude-reasoning-peer.cjs'));
  }
  await copyFile(join(root, 'packages/runtime/checks/fixtures/session-history-peer.cjs'), join(directory, 'fixtures/session-history-peer.cjs'));
  for (const fixture of ['worktree-backend-cwd-peer.cjs', 'claude-modes-peer.cjs', 'acp-peer.cjs', 'acp-team-peer.cjs', 'acp-runtime-peer.cjs', 'pi-vault-op.cs', 'codex-desktop-view-peer.cjs']) {
    await copyFile(join(root, 'packages/runtime/checks/fixtures', fixture), join(directory, 'fixtures', fixture));
  }
  await copyFile(join(root, 'packages/runtime/checks/fixtures/git-remotes-wire-peer.cjs'), join(directory, 'fixtures/git-remotes-wire-peer.cjs'));
  await copyFile(join(root, 'packages/runtime/checks/fixtures/windows-conpty-resize-80x24.bin'), join(directory, 'fixtures/windows-conpty-resize-80x24.bin'));
  // Test-only package resolution: no source-workspace dependencies can satisfy
  // a bare import from these temporary checks.
  await symlink(join(runtime, 'node_modules'), join(directory, 'node_modules'), process.platform === 'win32' ? 'junction' : 'dir');
  for (const name of checks) {
    const source = await readFile(join(root, 'packages/runtime/checks', `${name}.ts`), 'utf8');
    const bound = source.replace(/(['"])\.\.\/src\/([^'"]+)\1/g,
      (_all, _quote, module) => JSON.stringify(pathToFileURL(join(runtime, 'dist', module)).href));
    const result = ts.transpileModule(bound, {compilerOptions: {module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022}, reportDiagnostics: true});
    assert.equal((result.diagnostics ?? []).filter(item => item.category === ts.DiagnosticCategory.Error).length, 0);
    const file = join(directory, `${name}.mjs`);
    await writeFile(file, result.outputText);
    console.log(`[packaged-functional] ${name}`);
    // Registration includes a no-MCP deadline. Handoff runtime checks boot
    // several real runtimes/worktrees; Windows source runs measured 122/92/76s.
    const multipleBoots = ['session-history-rewind', 'session-handoff-runtime', 'handoff-runtime-failures', 'handoff-runtime-delivery'].includes(name);
    // The remote Git check now cold-starts a second runtime to verify removal.
    const timeout = multipleBoots ? 180_000 : ['acp-runtime-registration', 'git-remotes-wire'].includes(name) ? 120_000 : 60_000;
    const options = {cwd: directory, stdio: 'inherit', timeout,
      env: {...process.env, PI_CODING_AGENT_DIR: join(directory, 'agent'), PI_OFFLINE: '1',
        HIVE_DESKTOP_TEST_RUNTIME: join(runtime, 'dist/main.js'),
        ...(['codex-probe-owner', 'pi-vault-owner'].includes(name) && {HIVE_DESKTOP_SHELL_OWNER: join(bundle, 'Hive Desktop.app/Contents/MacOS/hive-desktop')})}};
    execFileSync(node, [file], options);
    receipt.checks.push({name, exitCode: 0});
    if (name === 'pi-vault' && process.platform === 'darwin') {
      const owner = join(bundle, 'Hive Desktop.app/Contents/MacOS/hive-desktop');
      console.log('[packaged-functional] pi-vault-native-owner');
      execFileSync(node, [file], {...options, env: {...options.env, HIVE_DESKTOP_SHELL_OWNER: owner}});
      receipt.checks.push({name: 'pi-vault-native-owner', exitCode: 0});
    }
    if (name === 'session-history-rewind' && process.platform === 'darwin') {
      console.log('[packaged-functional] session-history-rewind-sidecar-crash');
      execFileSync(node, [file, '--sidecar-crash'], options);
      receipt.checks.push({name: 'session-history-rewind-sidecar-crash', exitCode: 0});
    }
  }
  receipt.pass = true;
  console.log('PACKAGED_FUNCTIONAL_OK: bundled runtime, dependency graph and Node; controlled storage/protocol checks, no model inference.');
} finally {
  if (arg('--receipt')) await writeFile(resolve(arg('--receipt')), JSON.stringify(receipt, null, 2));
  await rm(directory, {recursive: true, force: true});
}
