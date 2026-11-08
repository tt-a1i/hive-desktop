import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {mkdtemp,readFile,writeFile,rm,realpath} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
const index=process.argv.indexOf('--bundle');
assert.ok(index>=0&&process.argv[index+1],'Pass --bundle PATH to an extracted portable package');
const bundle=resolve(process.argv[index+1]);
const resources=process.platform==='darwin'?join(bundle,'Hive Desktop.app','Contents','Resources'):join(bundle,'resources');
const runtime=join(resources,'runtime'),node=join(resources,'node',process.platform==='darwin'?'bin/node':'node.exe');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const ts=createRequire(join(root,'packages/runtime/package.json'))('typescript');
const sdkRoot=await realpath(join(runtime,'node_modules/@earendil-works/pi-coding-agent'));
const sdkMetadata=JSON.parse(await readFile(join(sdkRoot,'package.json'),'utf8'));
const sdkEntry=join(sdkRoot,sdkMetadata.exports['.'].import);
const directory=await mkdtemp(join(tmpdir(),'hive-packaged-pi-'));
try{
 const probes=['pi-history','pi-user-input',...(process.platform==='win32'?[]:['pi-persistence','pi-external-persistence'])];
 if(process.platform==='win32')console.log('Windows ACL fault injection is not covered; permission checks skipped explicitly.');
 for(const name of probes){
  const source=await readFile(join(root,'packages/runtime/checks',name+'.ts'),'utf8');
  assert.ok(source.includes("from '../src/"),'Probe must import the production runtime');
  const bound=source.replace(/from '\.\.\/src\/([^']+)'/g,(_all,module)=>`from ${JSON.stringify(pathToFileURL(join(runtime,'dist',module)).href)}`)
   .replace(/from '@earendil-works\/pi-coding-agent'/g,`from ${JSON.stringify(pathToFileURL(sdkEntry).href)}`);
  const result=ts.transpileModule(bound,{compilerOptions:{module:ts.ModuleKind.ES2022,target:ts.ScriptTarget.ES2022},reportDiagnostics:true});
  const errors=(result.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.equal(errors.length,0,errors.map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')).join('\n'));
  const file=join(directory,name+'.mjs');await writeFile(file,result.outputText);
  console.log(`[packaged-pi] Running ${name} with bundled Node, SDK and runtime`);
  execFileSync(node,[file],{cwd:directory,stdio:'inherit',timeout:60000,env:{...process.env,HIVE_PI_INPUT_PROOF_DIR:join(directory,'evidence')}});
 }
 console.log('Packaged Pi checks passed: native input/catalogue, uncompacted chat with compacted model context, branch/source recovery, real write failures and durable retry on POSIX (Windows ACL checks excluded). Original external Pi CLI; loopback only, no vendor inference.');
}finally{await rm(directory,{recursive:true,force:true});}
