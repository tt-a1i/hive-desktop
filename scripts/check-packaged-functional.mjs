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
const checks = ['queued-input-mutations', 'queued-input-failed-rollback', 'queued-input-mutation', 'queued-input-wire',
  'conversation-search', 'conversation-search-codex', 'session-body-search', 'project-files', 'project-file-references'];
const receipt = {pass: false, bundle, actualInference: false, checks: []};
try {
  await mkdir(join(directory, 'agent'));
  await mkdir(join(directory, 'fixtures'));
  await copyFile(join(root, 'packages/runtime/checks/fixtures/codex-queue-peer.cjs'), join(directory, 'fixtures/codex-queue-peer.cjs'));
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
    execFileSync(node, [file], {cwd: directory, stdio: 'inherit', timeout: 60_000,
      env: {...process.env, PI_CODING_AGENT_DIR: join(directory, 'agent'), PI_OFFLINE: '1',
        HIVE_DESKTOP_TEST_RUNTIME: join(runtime, 'dist/main.js')}});
    receipt.checks.push({name, exitCode: 0});
  }
  receipt.pass = true;
  console.log('PACKAGED_FUNCTIONAL_OK: bundled runtime, dependency graph and Node; controlled storage/protocol checks, no model inference.');
} finally {
  if (arg('--receipt')) await writeFile(resolve(arg('--receipt')), JSON.stringify(receipt, null, 2));
  await rm(directory, {recursive: true, force: true});
}
