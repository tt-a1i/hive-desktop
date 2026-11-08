import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
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
 for(const name of ['codex-interrupted-tools','codex-paginated-history']) {
  const source=await readFile(join(root,'packages/runtime/checks',name+'.ts'),'utf8');
  const original="from '../src/agents/codex.js'";
  assert.ok(source.includes(original),'The source probe must import the production Codex adapter');
  const bound=source.replace(original,`from ${JSON.stringify(pathToFileURL(join(runtime,'dist/agents/codex.js')).href)}`);
  // Reuse the same behavioral probes; only erase types and bind the release adapter.
  const result=ts.transpileModule(bound,{compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022},reportDiagnostics:true});
  const errors=(result.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.equal(errors.length,0,errors.map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')).join('\n'));
  const file=join(directory,name+'.mjs');await writeFile(file,result.outputText);
  console.log(`[packaged-codex] Running ${name} with bundled Node and packaged runtime`);
  execFileSync(node,[file],{cwd:directory,stdio:'inherit',timeout:60000});
 }
 console.log('Packaged Codex checks passed: native-history contract, cumulative partial output, interruption, background completion and restart. Synthetic JSONL peers; no vendor/model requests.');
}finally{await rm(directory,{recursive:true,force:true});}
