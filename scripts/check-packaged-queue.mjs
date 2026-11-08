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
const directory = await mkdtemp(join(tmpdir(), 'hive-packaged-queue-'));
try {
  const source = await readFile(join(root, 'packages/runtime/checks/queued-user-inputs.ts'), 'utf8');
  const bound = source.replace(/import\('\.\.\/src\/([^']+)'\)/g,
    (_all, module) => `import(${JSON.stringify(pathToFileURL(join(runtime, 'dist', module)).href)})`);
  const result = ts.transpileModule(bound, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true,
  });
  const errors = (result.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.equal(errors.length, 0, errors.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
  const file = join(directory, 'queued-user-inputs.mjs');
  await writeFile(file, result.outputText);
  for (const backend of ['pi', 'pi-rpc']) {
    console.log(`[packaged-queue] Running ${backend} against bundled Node and runtime`);
    execFileSync(node, [file], {
      cwd: runtime, stdio: 'inherit', timeout: 60000, env: { ...process.env, HIVE_QUEUE_BACKEND: backend },
    });
  }
  console.log('Packaged queue checks passed: real Pi/Hive/SQLite, FIFO, attachments, stop and paused cold restore. Local synthetic responses only; original external Pi required.');
} finally {
  await rm(directory, { recursive: true, force: true });
}
