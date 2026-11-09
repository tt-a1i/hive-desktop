import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, writeFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
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
const sdk = await realpath(join(runtime, 'node_modules/@earendil-works/pi-coding-agent'));
const metadata = JSON.parse(await readFile(join(sdk, 'package.json'), 'utf8'));
const sdkEntry = pathToFileURL(join(sdk, metadata.exports['.'].import)).href;
const directory = await mkdtemp(join(tmpdir(), 'hive-packaged-model-config-'));
try {
  for (const name of ['pi-model-config', 'pi-model-overrides', 'pi-model-overrides-wire', 'pi-model-reload', 'pi-default-model']) {
    const source = await readFile(join(root, 'packages/runtime/checks', name + '.ts'), 'utf8');
    const bound = source.replace(/from '\.\.\/src\/([^']+)'/g,
      (_all, module) => `from ${JSON.stringify(pathToFileURL(join(runtime, 'dist', module)).href)}`)
      .replace(/from '@earendil-works\/pi-coding-agent'/g, `from ${JSON.stringify(sdkEntry)}`);
    const result = ts.transpileModule(bound, {
      compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true,
    });
    const errors = (result.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error);
    assert.equal(errors.length, 0, errors.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
    const file = join(directory, name + '.mjs');
    await writeFile(file, result.outputText);
    console.log(`[packaged-models] Running ${name} with bundled Node, SDK and runtime`);
    // The wire probe starts dist/main.js relative to cwd: it must use the package, not the checkout.
    execFileSync(node, [file], { cwd: runtime, stdio: 'inherit', timeout: 60000 });
  }
  console.log('Packaged model checks passed: actual SDK/JSONC configuration, override inheritance/edit/remove, production JSONL and warm-session reload. No model requests.');
} finally {
  await rm(directory, { recursive: true, force: true });
}
