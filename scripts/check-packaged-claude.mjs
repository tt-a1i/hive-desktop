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
const resources=process.platform==='darwin'?join(bundle,'Hive Desktop.app/Contents/Resources'):join(bundle,'resources');
const runtime=join(resources,'runtime'),node=join(resources,'node',process.platform==='darwin'?'bin/node':'node.exe');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const ts=createRequire(join(root,'packages/runtime/package.json'))('typescript');
const directory=await mkdtemp(join(tmpdir(),'hive-packaged-claude-'));
try {
 await mkdir(join(directory,'fixtures'));
 await cp(join(root,'packages/runtime/checks/fixtures/claude-reasoning-peer.cjs'),join(directory,'fixtures/claude-reasoning-peer.cjs'));
 const source=await readFile(join(root,'packages/runtime/checks/claude-write-loss.ts'),'utf8');
 assert.ok(source.includes("from '../src/desktop-runtime.js'"));
 const bound=source.replace(/from '\.\.\/src\/([^']+)'/g,(_all,module)=>`from ${JSON.stringify(pathToFileURL(join(runtime,'dist',module)).href)}`);
 const result=ts.transpileModule(bound,{compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022},reportDiagnostics:true});
 const errors=(result.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
 assert.equal(errors.length,0,errors.map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')).join('\n'));
 const file=join(directory,'claude-write-loss.mjs');await writeFile(file,result.outputText);
 execFileSync(node,[file],{cwd:directory,stdio:'inherit',timeout:60000,env:{...process.env,HIVE_CLAUDE_WRITE_LOSS_OUTPUT:process.env.HIVE_CLAUDE_WRITE_LOSS_OUTPUT??join(root,'.artifacts/claude-write-loss/packaged')}});
 console.log('Packaged Claude checks passed: bundled Node/runtime, official SDK, synthetic CLI, real Hive/SQLite/transcripts/spool, scoped write loss and exit recovery. No inference.');
} finally { await rm(directory,{recursive:true,force:true}); }
