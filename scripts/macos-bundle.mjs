import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtemp,mkdir,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';

function run(command,args,input) {
 const result=spawnSync(command,args,{encoding:'utf8',input});
 if(result.error||result.status!==0)throw result.error??new Error(`${command} failed: ${result.stderr}`);
 return result.stdout;
}

/** Keep identity shared by fresh bundles and dependency-frozen resource refreshes. */
export async function prepareMacBundle(contents,{version,iconSource}) {
 if(process.platform!=='darwin')throw new Error('macOS bundle preparation requires macOS');
 const numericVersion=typeof version==='string'&&version.match(/^(\d+\.\d+\.\d+)(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$/)?.[1];
 if(!numericVersion)throw new Error('App version must be a three-part numeric version, optionally with a prerelease/build suffix');
 const executable=join(contents,'MacOS/hive-desktop');
 const commands=run('/usr/bin/otool',['-l',executable]);
 const minimum=commands.match(/cmd LC_BUILD_VERSION[\s\S]*?\bminos\s+([0-9.]+)/)?.[1]
  ??commands.match(/cmd LC_VERSION_MIN_MACOSX[\s\S]*?\bversion\s+([0-9.]+)/)?.[1];
 if(!minimum)throw new Error('Native executable has no macOS deployment target');
 const resources=join(contents,'Resources');await mkdir(resources,{recursive:true});
 const temporary=await mkdtemp(join(tmpdir(),'hive-app-icon-'));
 try {
  const iconset=join(temporary,'HiveDesktop.iconset');await mkdir(iconset);
  // The upstream logo is 512px. Preserve that ceiling rather than inventing detail.
  for(const size of [16,32,128,256,512]) {
   run('/usr/bin/sips',['-z',String(size),String(size),iconSource,'--out',join(iconset,`icon_${size}x${size}.png`)]);
   if(size<512)run('/usr/bin/sips',['-z',String(size*2),String(size*2),iconSource,'--out',join(iconset,`icon_${size}x${size}@2x.png`)]);
  }
  run('/usr/bin/iconutil',['-c','icns','-o',join(resources,'HiveDesktop.icns'),iconset]);
 }finally{await rm(temporary,{recursive:true,force:true});}
 const plist={CFBundleExecutable:'hive-desktop',CFBundleIdentifier:'com.hive.desktop.dev',CFBundleName:'Hive Desktop',CFBundleDisplayName:'Hive Desktop',CFBundlePackageType:'APPL',CFBundleVersion:numericVersion,CFBundleShortVersionString:numericVersion,CFBundleGetInfoString:`Hive Desktop ${version} development build`,CFBundleIconFile:'HiveDesktop.icns',LSMinimumSystemVersion:minimum,LSApplicationCategoryType:'public.app-category.developer-tools',NSHighResolutionCapable:true};
 run('/usr/bin/plutil',['-convert','xml1','-o',join(contents,'Info.plist'),'-'],JSON.stringify(plist));
 const hivePackage=JSON.parse(await readFile(join(dirname(dirname(iconSource)),'package.json'),'utf8'));
 const iconSha256=createHash('sha256').update(await readFile(iconSource)).digest('hex');
 await writeFile(join(resources,'hive-icon-source.txt'),`Hive logo from ${hivePackage.name}@${hivePackage.version}, assets/logo.png\nSHA256 ${iconSha256}\nOriginal notices: runtime/node_modules/@tt-a1i/hive/NOTICE, LICENSE.BSL and TRADEMARK.md\n`);
 return {version:numericVersion,minimumMacOS:minimum,iconSha256};
}
