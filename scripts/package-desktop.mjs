import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdtemp, mkdir, cp, readFile, rm, writeFile, realpath, readdir, lstat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, relative, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
import { prepareMacBundle } from './macos-bundle.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const arg = name => { const i = process.argv.indexOf(name); return i < 0 ? undefined : process.argv[i + 1]; };
const executable = arg('--executable');
const runtime = arg('--runtime');
const updateChannel = arg('--update-channel');
const updateArchive = arg('--update-archive');
const macSign = arg('--mac-sign');
if (!runtime || runtime.startsWith('--')) throw new Error('Pass --runtime PATH to a production-deployed runtime');
if (!executable || executable.startsWith('--')) throw new Error('Pass --executable PATH to a native build for this host');
if (process.argv.includes('--update-channel') && (!updateChannel || updateChannel.startsWith('--'))) throw new Error('Pass --update-channel PATH to an update channel file');
if (process.argv.includes('--update-archive') && (!updateChannel || !updateArchive || !/\.zip$/i.test(updateArchive))) throw new Error('Pass --update-channel and --update-archive PATH.zip together');
if (process.argv.includes('--mac-sign') && (process.platform !== 'darwin' || !macSign || macSign.startsWith('--'))) throw new Error('On macOS, pass --mac-sign IDENTITY (use - for ad hoc signing)');
if (!['darwin', 'win32'].includes(process.platform) || !['arm64', 'x64'].includes(process.arch)) throw new Error('Packaging supports native macOS or Windows arm64/x64 hosts only');
const output = resolve(arg('--output') ?? join(root, '.artifacts', `portable-${process.platform}-${process.arch}`));
if (output === root || root.startsWith(output + '/') || output === resolve('/')) throw new Error('Refusing unsafe output path');
const version = '22.23.3';
const platform = process.platform === 'darwin' ? 'darwin' : 'win';
const ext = platform === 'win' ? 'zip' : 'tar.gz';
const basename = `node-v${version}-${platform}-${process.arch}`;
const archive = `${basename}.${ext}`;
const url = `https://nodejs.org/dist/v${version}`;
const temp = await mkdtemp(join(tmpdir(), 'hive-desktop-package-'));
function progress(message) { console.log(`[package] ${message}`); }
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  if (result.error || result.status !== 0) throw result.error ?? new Error(`${command} failed: ${result.status}`);
}
try {
  const binary = await realpath(resolve(executable));
  const deployedRuntime = await realpath(resolve(runtime));
  for (const required of ['dist/main.js', 'package.json', 'node_modules']) await realpath(join(deployedRuntime, required));
  const runtimeRelativeOutput = relative(deployedRuntime, output);
  if (!runtimeRelativeOutput || (!runtimeRelativeOutput.startsWith('..') && !isAbsolute(runtimeRelativeOutput))) throw new Error('Output must be outside deployed runtime');
  try { await lstat(output); throw new Error(`Output already exists: ${output}`); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const app = process.platform === 'darwin' ? join(output, 'Hive Desktop.app', 'Contents') : output;
  const resources = join(app, process.platform === 'darwin' ? 'Resources' : 'resources');
  const bin = process.platform === 'darwin' ? join(app, 'MacOS', 'hive-desktop') : join(app, 'hive-desktop.exe');
  await mkdir(dirname(bin), { recursive: true });
  await mkdir(resources, { recursive: true });
  await cp(binary, bin);
  progress('Copying production-deployed runtime');
  await cp(deployedRuntime, join(resources, 'runtime'), { recursive: true, verbatimSymlinks: true });
  const gpuiNotices = join(resources, 'licenses', 'gpui-pre');
  await mkdir(gpuiNotices, { recursive: true });
  for (const file of ['LICENSE-APACHE', 'HIVE-PATCH.md']) {
    await cp(join(root, 'vendor', 'gpui-pre', file), join(gpuiNotices, file));
  }
  const terminalNotices = join(resources, 'licenses', 'hive-terminal');
  await mkdir(terminalNotices, { recursive: true });
  for (const file of ['LICENSE', 'NOTICE']) {
    await cp(join(root, 'crates', 'hive-terminal', file), join(terminalNotices, file));
  }
  await cp(join(root, 'apps/desktop/licenses/LICENSE-LUCIDE'), join(resources, 'licenses/LICENSE-LUCIDE'));
  await cp(join(root, 'apps/desktop/licenses/LICENSE-T3CODE'), join(resources, 'licenses/LICENSE-T3CODE'));
  const metadata = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
  let updateIdentity;
  if (updateChannel) {
    const { feedUrl, publicKey } = JSON.parse(await readFile(resolve(updateChannel), 'utf8'));
    if (typeof feedUrl !== 'string' || typeof publicKey !== 'string') throw new Error('Update channel needs a feed URL and a public verification key');
    const probe = spawnSync(binary, ['--desktop-update-identity'], {encoding: 'utf8', timeout: 10_000, windowsHide: true});
    if (probe.error || probe.status !== 0) throw probe.error ?? new Error('Could not read native update identity');
    const identity = JSON.parse(probe.stdout);
    if (identity.schemaVersion !== 1 || identity.app !== 'com.hive.desktop.dev' || identity.version !== metadata.version
      || identity.platform !== process.platform || identity.arch !== process.arch) throw new Error('Native update identity does not match package version or target');
    if (macSign) {
      // The outer signature covers this manifest and changes the executable
      // bytes. The updater records the executable digest after verification.
      updateIdentity = {...identity, schemaVersion: 2};
    } else {
      const executableHash = createHash('sha256');
      for await (const chunk of createReadStream(bin)) executableHash.update(chunk);
      updateIdentity = {...identity, executableSha256: executableHash.digest('hex')};
    }
    await writeFile(join(resources, 'desktop-update-channel.json'), JSON.stringify({schemaVersion: 1, feedUrl, publicKey, installedVersion: metadata.version}));
  }
  if (process.platform === 'darwin') {
    await prepareMacBundle(app, { version: metadata.version,
      iconSource: join(resources, 'runtime', 'node_modules', '@tt-a1i', 'hive', 'assets', 'logo.png') });
  }
  progress('Runtime copy completed');
  progress(`Downloading official Node archive ${archive}`);
  run('curl', ['-fL', '--connect-timeout', '20', '--max-time', '120', '--retry', '2', '--retry-max-time', '300', '-o', join(temp, archive), `${url}/${archive}`]);
  progress('Node archive downloaded');
  progress('Downloading official checksums');
  run('curl', ['-fL', '--connect-timeout', '20', '--max-time', '120', '--retry', '2', '--retry-max-time', '300', '-o', join(temp, 'SHASUMS256.txt'), `${url}/SHASUMS256.txt`]);
  progress('Checksums downloaded; verifying archive');
  const sums = await readFile(join(temp, 'SHASUMS256.txt'), 'utf8');
  const expected = sums.split(/\r?\n/).find(line => line.trim().endsWith(`  ${archive}`))?.split(/\s+/)[0];
  const actual = createHash('sha256').update(await readFile(join(temp, archive))).digest('hex');
  if (!expected || expected !== actual) throw new Error('Official Node archive SHA256 verification failed');
  progress('Node SHA256 verified; extracting archive');
  run('tar', ['-xf', join(temp, archive), '-C', temp]);
  progress('Node archive extracted');
  const nodeDir = join(resources, 'node');
  await mkdir(nodeDir, { recursive: true });
  if (platform === 'win') await cp(join(temp, basename, 'node.exe'), join(nodeDir, 'node.exe'));
  else { await mkdir(join(nodeDir, 'bin')); await cp(join(temp, basename, 'bin', 'node'), join(nodeDir, 'bin', 'node')); }
  await cp(join(temp, basename, 'LICENSE'), join(nodeDir, 'LICENSE'));
  // Deployment may use links, but no target may escape the portable bundle.
  const base = await realpath(output);
  let files = 0;
  async function checkLinks(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = join(path, entry.name);
      if (entry.isSymbolicLink()) { const target = await realpath(child); const rel = relative(base, target); if (rel.startsWith('..') || isAbsolute(rel)) throw new Error(`External link in bundle: ${child}`); }
      else if (entry.isDirectory()) await checkLinks(child);
      if (!entry.isDirectory()) files++;
    }
  }
  progress('Checking bundle links');
  await checkLinks(output);
  if (updateIdentity) {
    await writeFile(join(resources, 'desktop-update-identity.json'), JSON.stringify({...updateIdentity, files: files + 1}));
  }
  if (macSign) {
    const bundle = dirname(app);
    run('/usr/bin/codesign', ['--force', '--sign', macSign, bundle]);
    run('/usr/bin/codesign', ['--verify', '--deep', '--strict', '-R=identifier "com.hive.desktop.dev"', bundle]);
  }
  if (updateArchive) {
    const destination = resolve(updateArchive);
    const within = relative(base, destination);
    if (!within || (!within.startsWith('..') && !isAbsolute(within))) throw new Error('Update archive must be outside the bundle');
    try { await lstat(destination); throw new Error('Update archive already exists'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (process.platform === 'darwin') run('/usr/bin/zip', ['-q', '-y', '-r', destination, 'Hive Desktop.app'], { cwd: base });
    else run('tar.exe', ['-a', '-cf', destination, '-C', base, 'hive-desktop.exe', 'resources']);
  }
  progress('Bundle link check completed');
  console.log(`Packaged ${output}\nNode archive SHA256: ${actual}`);
} finally { await rm(temp, { recursive: true, force: true }); }
