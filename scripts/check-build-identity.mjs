import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = await mkdtemp(join(tmpdir(), 'hive-build-identity-'));
const root = join(directory, 'source');
const app = join(root, 'apps/desktop');
const hooks = join(directory, 'empty-hooks');
const config = join(directory, 'empty-gitconfig');
await mkdir(join(app, 'src'), { recursive: true });
await mkdir(hooks);
await writeFile(config, '');
const env = { ...process.env, CARGO_TARGET_DIR: join(directory, 'target'),
  GIT_CONFIG_GLOBAL: config, GIT_CONFIG_NOSYSTEM: '1' };
function run(command, args, cwd = root) {
  return execFileSync(command, args, { cwd, env, encoding: 'utf8', timeout: 60_000, stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}
function git(...args) {
  return run('git', ['-c', `core.hooksPath=${hooks}`, '-c', 'user.name=Hive fixture',
    '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args]);
}
const source = `fn main() {
    println!("revision={}\\ndirty={}\\nprofile={}\\ntarget={}",
        env!("HIVE_DESKTOP_GIT_REVISION"), env!("HIVE_DESKTOP_GIT_DIRTY"),
        env!("HIVE_DESKTOP_BUILD_PROFILE"), env!("HIVE_DESKTOP_BUILD_TARGET"));
}\n`;
const manifest = '[package]\nname="hive-desktop"\nversion="0.0.0"\nedition="2024"\n';
const lock = '# Fixture lock\nversion = 4\n\n[[package]]\nname = "hive-desktop"\nversion = "0.0.0"\n';
const actualBuildScript = fileURLToPath(new URL('../apps/desktop/build.rs', import.meta.url));
async function prepare(path) {
  await mkdir(join(path, 'src'), { recursive: true });
  await writeFile(join(path, 'Cargo.toml'), manifest);
  await writeFile(join(path, 'Cargo.lock'), lock);
  await writeFile(join(path, 'src/main.rs'), source);
  await copyFile(actualBuildScript, join(path, 'build.rs'));
}
function build(path = app) {
  const output = run('cargo', ['run', '--quiet', '--locked', '--manifest-path', join(path, 'Cargo.toml')], path);
  return Object.fromEntries(output.split(/\r?\n/).map(line => line.split('=')));
}
try {
  await prepare(app);
  git('init', '--template=');
  git('add', '.');
  git('commit', '-m', 'initial fixture');
  const first = git('rev-parse', 'HEAD');
  const clean = build();
  assert.equal(clean.revision, first);
  assert.equal(clean.dirty, 'false');
  assert.equal(clean.profile, 'debug');
  assert.ok(clean.target && clean.target !== 'unknown');

  // Stabilize Cargo's cache before changing only untracked entries. The marker
  // explicitly covers tracked edits, rather than claiming a clean filesystem.
  assert.equal(build().dirty, 'false');
  assert.equal(build().dirty, 'false');
  const untracked = join(root, 'untracked-source.rs');
  await writeFile(untracked, '// untracked fixture\n');
  assert.equal(build().dirty, 'false');
  await rm(untracked);
  assert.equal(build().dirty, 'false');
  await writeFile(untracked, '// staged fixture\n');
  git('add', 'untracked-source.rs');
  assert.equal(build().dirty, 'true');
  git('commit', '-m', 'track fixture source');
  assert.equal(build().dirty, 'false');

  await writeFile(join(app, 'src/main.rs'), source + '// uncommitted source edit\n');
  const modified = build();
  assert.equal(modified.revision, git('rev-parse', 'HEAD'));
  assert.equal(modified.dirty, 'true');
  git('add', '.');
  git('commit', '-m', 'second fixture');
  const second = git('rev-parse', 'HEAD');
  assert.notEqual(second, first);
  const committed = build();
  assert.equal(committed.revision, second);
  assert.equal(committed.dirty, 'false');

  // An archive inside an unrelated checkout must not borrow the ancestor HEAD.
  const archive = join(root, 'exported-source/apps/desktop');
  await prepare(archive);
  const unknown = build(archive);
  assert.equal(unknown.revision, 'unknown');
  assert.equal(unknown.dirty, 'unknown');
  const standalone = join(directory, 'non-git/apps/desktop');
  await prepare(standalone);
  assert.equal(build(standalone).revision, 'unknown');
  assert.equal(await readFile(join(app, 'src/main.rs'), 'utf8'), source + '// uncommitted source edit\n');
  console.log('BUILD_IDENTITY_OK: actual Cargo builds track clean, edited and committed source; exported and non-Git builds remain unknown.');
} finally {
  await rm(directory, { recursive: true, force: true, maxRetries: 5 });
}
