import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {mkdtemp,readFile,writeFile,rm,mkdir,cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';

const index=process.argv.indexOf('--bundle');
assert.ok(index>=0&&process.argv[index+1],'Pass --bundle PATH to an extracted portable package');
const bundle=resolve(process.argv[index+1]);
const resources=process.platform==='darwin'?join(bundle,'Hive Desktop.app','Contents','Resources'):join(bundle,'resources');
const runtime=join(resources,'runtime');
const node=join(resources,'node',process.platform==='darwin'?'bin/node':'node.exe');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const require=createRequire(join(root,'packages/runtime/package.json'));
const ts=require('typescript');
const directory=await mkdtemp(join(tmpdir(),'hive-packaged-codex-'));
try {
 await mkdir(join(directory,'fixtures'));
 await cp(join(root,'packages/runtime/checks/fixtures/codex-settings-peer.cjs'),join(directory,'fixtures/codex-settings-peer.cjs'));
 await cp(join(root,'packages/runtime/checks/fixtures/codex-queue-peer.cjs'),join(directory,'fixtures/codex-queue-peer.cjs'));
 for(const name of ['codex-settings','codex-interrupted-tools','codex-interrupt-rejection','codex-paginated-history','codex-exit-cleanup']) {
  const source=await readFile(join(root,'packages/runtime/checks',name+'.ts'),'utf8');
  const original=name==='codex-exit-cleanup'?"from '../src/desktop-runtime.js'":"from '../src/agents/codex.js'";
  assert.ok(source.includes(original),'The source probe must import its production adapter/runtime');
  const bound=source.replace(/from '\.\.\/src\/([^']+)'/g,(_all,module)=>`from ${JSON.stringify(pathToFileURL(join(runtime,'dist',module)).href)}`);
  // Reuse the same behavioral probes; only erase types and bind the release adapter.
  const result=ts.transpileModule(bound,{compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022},reportDiagnostics:true});
  const errors=(result.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.equal(errors.length,0,errors.map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')).join('\n'));
  const file=join(directory,name+'.mjs');await writeFile(file,result.outputText);
  console.log(`[packaged-codex] Running ${name} with bundled Node and packaged runtime`);
  execFileSync(node,[file],{cwd:directory,stdio:'inherit',timeout:60000});
 }
 console.log('Packaged Codex checks passed: applied settings acknowledgement, native-history contract, cumulative partial output, interruption/rejection cleanup, durable pending-delivery recovery, background completion, original exit diagnostics and restart. Synthetic JSONL peers; no vendor/model requests.');
}finally{await rm(directory,{recursive:true,force:true});}
