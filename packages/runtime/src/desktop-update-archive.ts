import { execFile } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { chmod, lstat, mkdir, readFile, readdir, realpath, symlink } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { promisify } from 'node:util';
import { openPromise } from 'yauzl';
import { desktopPackagePaths, hashDesktopFile } from './desktop-update-installer.js';
export { desktopPackagePaths, hashDesktopFile } from './desktop-update-installer.js';

const maxEntries = 100_000;
const maxExpandedBytes = 4 * 1024 * 1024 * 1024;
const maxLinkBytes = 4096;

function inside(root: string, path: string): boolean {
  const rel = relative(root, path);
  return Boolean(rel) && !isAbsolute(rel) && rel !== '..' && !rel.startsWith(`..${sep}`);
}

function checkPart(part: string) {
  if (!part || part === '.' || part === '..' || /[<>:"\\|?*\x00-\x1f]|[ .]$/.test(part)
    || /^(?:con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/i.test(part)) {
    throw new Error('Desktop update archive contains an invalid path');
  }
}

/** Extract a verified ZIP to a fresh directory. Links are only created after all regular writes. */
export async function extractDesktopUpdate(archive: string, directory: string, signal: AbortSignal): Promise<string> {
  await mkdir(directory, { mode: 0o700 });
  const root = await realpath(directory);
  const zip = await openPromise(archive, { strictFileNames: true, validateEntrySizes: true });
  const paths = new Map<string, { name: string; kind: 'directory' | 'file' | 'link'; explicit: boolean }>();
  const links: { path: string; target: string }[] = [];
  let totalBytes = 0;
  try {
    if (zip.entryCount > maxEntries) throw new Error('Desktop update archive has too many entries');
    for await (const entry of zip.eachEntry()) {
      signal.throwIfAborted();
      const isDirectory = entry.fileName.endsWith('/');
      const parts = entry.fileName.replace(/\/$/, '').split('/');
      parts.forEach(checkPart);
      if (process.platform === 'darwin' && parts[0] === '__MACOSX') continue;
      if (process.platform === 'darwin' && parts[0] !== 'Hive Desktop.app') {
        throw new Error('Expected a Hive Desktop.app update archive');
      }
      const unixMode = (entry.versionMadeBy >>> 8) === 3 ? entry.externalFileAttributes >>> 16 : 0;
      const type = unixMode & 0o170000;
      if (type && type !== 0o100000 && type !== 0o040000 && type !== 0o120000) {
        throw new Error('Desktop update archive contains a special file');
      }
      const kind = type === 0o120000 ? 'link' : isDirectory ? 'directory' : 'file';
      if ((type === 0o040000 && !isDirectory) || (isDirectory && (kind !== 'directory' || entry.uncompressedSize !== 0))) {
        throw new Error('Desktop update archive contains an invalid directory');
      }
      let parent = root;
      for (let i = 0; i < parts.length; i++) {
        const part = parts[i]!;
        const path = join(parent, part);
        const key = path.normalize('NFC').toUpperCase();
        const explicit = i === parts.length - 1;
        const expected = explicit ? kind : 'directory';
        const existing = paths.get(key);
        if (existing && (existing.name !== path || existing.kind !== expected || (explicit && existing.explicit))) {
          throw new Error('Desktop update archive contains colliding paths');
        }
        paths.set(key, { name: path, kind: expected, explicit: explicit || Boolean(existing?.explicit) });
        if (expected === 'directory' && !existing) await mkdir(path, { mode: 0o755 });
        parent = path;
      }
      if (kind === 'directory') continue;
      totalBytes += entry.uncompressedSize;
      if (totalBytes > maxExpandedBytes) throw new Error('Desktop update archive exceeds its expanded size limit');
      if (kind === 'link') {
        if (process.platform !== 'darwin' || entry.uncompressedSize > maxLinkBytes) throw new Error('Unsupported desktop update link');
        const bytes: Buffer[] = [];
        for await (const chunk of await zip.openReadStreamPromise(entry)) {
          signal.throwIfAborted();
          bytes.push(chunk as Buffer);
        }
        const target = new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(bytes));
        if (!target || isAbsolute(target) || /[\\\x00-\x1f]/.test(target) || !inside(root, resolve(dirname(parent), target))) {
          throw new Error('Desktop update link escapes the package');
        }
        links.push({ path: parent, target });
      } else {
        const mode = unixMode & 0o100 ? 0o755 : 0o644;
        await pipeline(await zip.openReadStreamPromise(entry), createWriteStream(parent, { flags: 'wx', mode }), { signal });
        if (process.platform === 'darwin') await chmod(parent, mode);
      }
    }
  } finally {
    zip.close();
  }
  const packageRoot = process.platform === 'darwin' ? join(root, 'Hive Desktop.app') : root;
  for (const {path, target} of links) await symlink(target, path);
  for (const {path} of links) {
    if (!inside(packageRoot, await realpath(path))) throw new Error('Desktop update link escapes the package');
  }
  return packageRoot;
}

export async function verifyDesktopPackage(root: string, expectedVersion: string, signal: AbortSignal) {
  const paths = desktopPackagePaths(root);
  const identityPath = join(paths.resources, 'desktop-update-identity.json');
  for (const path of [identityPath, paths.executable, paths.node, paths.runtime]) {
    const info = await lstat(path);
    if (!info.isFile() || await realpath(path) !== path) throw new Error('Desktop update is missing a real package file');
    if (process.platform === 'darwin' && (path === paths.executable || path === paths.node) && !(info.mode & 0o100)) {
      throw new Error('Desktop update is missing executable permission');
    }
  }
  if ((await lstat(identityPath)).size > 4096) throw new Error('Invalid desktop update identity');
  const identity = JSON.parse(await readFile(identityPath, 'utf8'));
  if (identity?.schemaVersion !== 1 || identity.app !== 'com.hive.desktop.dev' || identity.version !== expectedVersion
    || identity.platform !== process.platform || identity.arch !== process.arch
    || identity.executableSha256 !== await hashDesktopFile(paths.executable, signal)) {
    throw new Error('Desktop update package identity does not match the selected release');
  }
  let files = 0;
  async function count(path: string): Promise<void> {
    signal.throwIfAborted();
    for (const entry of await readdir(path, { withFileTypes: true })) {
      if (entry.isDirectory()) await count(join(path, entry.name));
      else files++;
    }
  }
  await count(root);
  if (identity.files !== files) throw new Error(`Desktop update package file count is incomplete (${files} files found)`);
  if (process.platform === 'darwin') {
    const plist = join(root, 'Contents/Info.plist');
    const info = await lstat(plist);
    if (!info.isFile() || info.size > 128 * 1024 || await realpath(plist) !== plist) {
      throw new Error('Desktop update is missing its macOS application metadata');
    }
    const { stdout } = await promisify(execFile)('/usr/bin/plutil', ['-convert', 'json', '-o', '-', plist], { timeout: 5000, signal });
    const metadata = JSON.parse(stdout);
    if (metadata.CFBundleIdentifier !== identity.app || metadata.CFBundleExecutable !== 'hive-desktop'
      || metadata.CFBundleShortVersionString !== expectedVersion || metadata.CFBundleVersion !== expectedVersion) {
      throw new Error('macOS application metadata does not match the selected release');
    }
  }
  return { executableSha256: identity.executableSha256 as string, installerProtocol: identity.installerProtocol as unknown };
}
