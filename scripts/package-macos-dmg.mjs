import {spawnSync} from 'node:child_process';
import {mkdtemp,cp,symlink,lstat,rm,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,join,dirname,relative,isAbsolute} from 'node:path';

const arg=name=>{const i=process.argv.indexOf(name);return i<0?undefined:process.argv[i+1];};
if(process.platform!=='darwin')throw new Error('DMG packaging requires macOS');
const app=arg('--app'),output=arg('--output');
if(!app||!app.endsWith('.app')||!output||!output.endsWith('.dmg'))throw new Error('Pass --app PATH.app --output NEW_PATH.dmg');
const source=resolve(app),target=resolve(output);
const relativeTarget=relative(source,target);
if(!relativeTarget||(!relativeTarget.startsWith('..')&&!isAbsolute(relativeTarget)))throw new Error('DMG output must be outside the source app');
if(!(await lstat(source)).isDirectory())throw new Error('App must be a directory');
for(const required of ['Info.plist','MacOS/hive-desktop','Resources/HiveDesktop.icns','Resources/node/bin/node','Resources/runtime/dist/main.js'])await lstat(join(source,'Contents',required));
try{await lstat(target);throw new Error('DMG output already exists');}catch(error){if(error.code!=='ENOENT')throw error;}
await mkdir(dirname(target),{recursive:true});
const temporary=await mkdtemp(join(tmpdir(),'hive-desktop-dmg-'));
function run(args){const r=spawnSync('/usr/bin/hdiutil',args,{stdio:'inherit'});if(r.error||r.status!==0)throw r.error??new Error(`hdiutil failed: ${r.status}`);}
try {
 await cp(source,join(temporary,'Hive Desktop.app'),{recursive:true,verbatimSymlinks:true});
 await symlink('/Applications',join(temporary,'Applications'));
 run(['create','-volname','Hive Desktop','-srcfolder',temporary,'-format','UDZO','-imagekey','zlib-level=9',target]);
 run(['verify',target]);
 console.log(`Created unsigned development disk image: ${target}`);
}finally{await rm(temporary,{recursive:true,force:true});}
