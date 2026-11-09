import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const index = process.argv.indexOf('--bundle');
assert.ok(index >= 0 && process.argv[index + 1], 'Pass --bundle PATH to an extracted portable package');
const bundle = resolve(process.argv[index + 1]);
const resources = process.platform === 'darwin'
  ? join(bundle, 'Hive Desktop.app', 'Contents', 'Resources') : join(bundle, 'resources');
const runtime = join(resources, 'runtime');
const node = join(resources, 'node', process.platform === 'darwin' ? 'bin/node' : 'node.exe');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ts = createRequire(join(root, 'packages/runtime/package.json'))('typescript');
// Resolve with the packaged Node's ESM loader: Pi SDK has import-only exports.
const packagedImports = new Map(['typebox', '@earendil-works/pi-coding-agent'].map(dependency => [dependency,
  execFileSync(node, ['--input-type=module', '-e', 'process.stdout.write(import.meta.resolve(process.argv[1]))', dependency],
    {cwd: runtime, encoding: 'utf8', timeout: 10000}),
]));
const directory = await mkdtemp(join(tmpdir(), 'hive-packaged-dispatches-'));
try {
  // Erase only check types. Every production import and runtime dependency is
  // resolved from the extracted package, and execution uses its actual Node.
  for (const check of ['reported-dispatch-queue', 'codex-tool-retirement', 'codex-steering', 'empty-session-recovery', 'user-input-recovery', 'workflow-shutdown']) {
    const source = await readFile(join(root, `packages/runtime/checks/${check}.ts`), 'utf8');
    const bound = source.replace(/(['"])\.\.\/src\/([^'"]+)\1/g,
      (_all, _quote, module) => JSON.stringify(pathToFileURL(join(runtime, 'dist', module)).href))
      .replace(/(['"])(typebox|@earendil-works\/pi-coding-agent)\1/g,
        (_all, _quote, dependency) => JSON.stringify(packagedImports.get(dependency)));
    const result = ts.transpileModule(bound, {
      compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true,
    });
    const errors = (result.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error);
    assert.equal(errors.length, 0, errors.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
    const file = join(directory, `${check}.mjs`);
    await writeFile(file, result.outputText);
    console.log(`[packaged-dispatches] ${check}: bundled Node, runtime, Hive and dependencies`);
    execFileSync(node, [file], { cwd: runtime, stdio: 'inherit', timeout: 60000 });
  }
} finally { await rm(directory, { recursive: true, force: true }); }
