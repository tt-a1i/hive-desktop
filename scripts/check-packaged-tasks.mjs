import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const index=process.argv.indexOf('--bundle');
assert.ok(index>=0&&process.argv[index+1],'Pass --bundle PATH to an extracted portable package');
const bundle=resolve(process.argv[index+1]);
const resources=process.platform==='darwin'?join(bundle,'Hive Desktop.app/Contents/Resources'):join(bundle,'resources');
const runtime=join(resources,'runtime'),node=join(resources,'node',process.platform==='darwin'?'bin/node':'node.exe');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const ts=createRequire(join(root,'packages/runtime/package.json'))('typescript');
const directory=await mkdtemp(join(tmpdir(),'hive-packaged-tasks-'));
try{
 const source=await readFile(join(root,'packages/runtime/checks/tasks-watcher-lifecycle.ts'),'utf8');
 // Resolve Hive from the deployed package, rather than from this checkout or the temporary probe.
 const bound=source.replace('createRequire(import.meta.url)',`createRequire(${JSON.stringify(join(runtime,'package.json'))})`);
 const result=ts.transpileModule(bound,{compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022},reportDiagnostics:true});
 const errors=(result.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
 assert.equal(errors.length,0,errors.map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')).join('\n'));
 const file=join(directory,'tasks-watcher-lifecycle.mjs');await writeFile(file,result.outputText);
 execFileSync(node,[file],{cwd:runtime,stdio:'inherit',timeout:60000});
 console.log('Packaged tasks watcher passed: real native filesystem startup/cancel/restart and direct/atomic notifications. No inference.');
}finally{await rm(directory,{recursive:true,force:true});}
