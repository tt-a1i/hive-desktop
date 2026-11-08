import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const at = process.argv.indexOf('--bundle');
assert.ok(at >= 0 && process.argv[at + 1], 'Pass --bundle PATH to an extracted portable package');
const bundle = resolve(process.argv[at + 1]);
const resources = process.platform === 'darwin'
  ? join(bundle, 'Hive Desktop.app', 'Contents', 'Resources') : join(bundle, 'resources');
const runtime = join(resources, 'runtime');
const node = join(resources, 'node', process.platform === 'darwin' ? 'bin/node' : 'node.exe');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ts = createRequire(join(root, 'packages/runtime/package.json'))('typescript');
const directory = await mkdtemp(join(tmpdir(), 'hive-packaged-remote-'));
try {
  let source = await readFile(join(root, 'packages/runtime/checks/remote-settings.ts'), 'utf8');
  // All production imports, crypto, WS and child CLI processes come from the
  // extracted bundle. The checkout contributes only this executable check.
  const replacements = [
    ["'../src/hive-host.js'", JSON.stringify(pathToFileURL(join(runtime, 'dist/hive-host.js')).href)],
    ["'../dist/remote-commands.js'", JSON.stringify(pathToFileURL(join(runtime, 'dist/remote-commands.js')).href)],
    ["new URL('../dist/remote-cli.js', import.meta.url)", `new URL(${JSON.stringify(pathToFileURL(join(runtime, 'dist/remote-cli.js')).href)})`],
    ['createRequire(import.meta.url)', `createRequire(${JSON.stringify(join(runtime, 'package.json'))})`],
    ["from 'ws'", `from ${JSON.stringify(pathToFileURL(join(runtime, 'node_modules/ws/wrapper.mjs')).href)}`],
  ];
  for (const [original, replacement] of replacements) {
    assert.equal(source.split(original).length - 1, 1, `Expected exactly one package binding: ${original}`);
    source = source.replace(original, replacement);
  }
  const result = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true,
  });
  const errors = (result.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.equal(errors.length, 0, errors.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
  const file = join(directory, 'remote-settings.mjs');
  await writeFile(file, result.outputText);
  execFileSync(node, [file], { cwd: runtime, stdio: 'inherit', timeout: 60_000 });
  console.log('Packaged remote check passed: bundled Node, CLI, Hive HTTP/SQLite, X25519/SAS, encrypted config-key rejection and malformed-stream recovery, durable revoke retry. No production account or model request.');
} finally {
  await rm(directory, { recursive: true, force: true });
}
