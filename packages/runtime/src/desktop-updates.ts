import { createHash, createPublicKey, randomUUID, verify, type KeyObject } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, mkdtemp, open, readFile, readdir, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { extractDesktopUpdate, verifyDesktopPackage } from './desktop-update-archive.js';
import { desktopPackagePaths, hashDesktopFile, type DesktopInstallPlan } from './desktop-update-installer.js';

const manifestLimit = 64 * 1024;
const archiveLimit = 2 * 1024 * 1024 * 1024;

type Policy = { feed: URL; key: KeyObject; installedVersion: string };
type Asset = { platform: string; arch: string; file: string; bytes: number; sha256: string };
type Release = { version: string; assets: Asset[] };
type Candidate = { version: string; asset: Asset };
type Check =
  | { state: 'disabled' }
  | { state: 'current'; installedVersion: string }
  | { state: 'unsupported'; installedVersion: string; version: string }
  | ({ state: 'available'; installedVersion: string; staged?: true } & Candidate);
type Available = Extract<Check, { state: 'available' }>;
type Prepared = { directory: string; packageRoot: string; version: string; sha256: string };
type Installer = { executable: string; program: string; version: string; sha256: string };

function asObject(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid desktop update document');
  return value as Record<string, unknown>;
}

function version(value: unknown): string {
  if (typeof value !== 'string' || !/^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})$/.test(value)) {
    throw new Error('Invalid desktop update version');
  }
  return value;
}

function newer(candidate: string, current: string): boolean {
  const old = current.split('.').map(Number);
  const firstDifference = candidate.split('.').map(Number).map((part, i) => part - old[i]!).find(value => value !== 0);
  return (firstDifference ?? 0) > 0;
}

async function loadPolicy(path: URL): Promise<Policy | undefined> {
  let input: string;
  try {
    input = await readFile(path, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined;
    throw error;
  }
  const raw = asObject(JSON.parse(input));
  if (raw.schemaVersion !== 1 || typeof raw.feedUrl !== 'string' || typeof raw.publicKey !== 'string') {
    throw new Error('Invalid desktop update channel');
  }
  const feed = new URL(raw.feedUrl);
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(feed.hostname);
  if (feed.username || feed.password || feed.hash || feed.search || !(feed.protocol === 'https:' || (loopback && feed.protocol === 'http:'))) {
    throw new Error('The desktop update channel requires HTTPS or a local test server');
  }
  const key = createPublicKey({ key: Buffer.from(raw.publicKey, 'base64'), format: 'der', type: 'spki' });
  if (key.asymmetricKeyType !== 'ed25519') throw new Error('Invalid desktop update verification key');
  return { feed, key, installedVersion: version(raw.installedVersion) };
}

function releaseDocument(data: Buffer, key: KeyObject): Release {
  const envelope = asObject(JSON.parse(data.toString('utf8')));
  if (typeof envelope.payload !== 'string' || typeof envelope.signature !== 'string') throw new Error('Missing signed desktop update manifest');
  const payload = Buffer.from(envelope.payload, 'base64');
  if (!verify(null, payload, key, Buffer.from(envelope.signature, 'base64'))) throw new Error('Desktop update signature verification failed');
  const release = asObject(JSON.parse(payload.toString('utf8')));
  if (release.schemaVersion !== 1 || !Array.isArray(release.assets)) throw new Error('Invalid signed desktop update manifest');
  const targets = new Set<string>();
  const assets = release.assets.map((input: unknown): Asset => {
    const { platform, arch, file, bytes, sha256 } = asObject(input);
    if ((platform !== 'darwin' && platform !== 'win32') || (arch !== 'arm64' && arch !== 'x64')
      || typeof file !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,126}\.zip$/.test(file)
      || typeof bytes !== 'number' || !Number.isSafeInteger(bytes) || bytes < 1 || bytes > archiveLimit
      || typeof sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(sha256)) {
      throw new Error('Invalid desktop update asset');
    }
    const target = `${platform}/${arch}`;
    if (targets.has(target)) throw new Error('The desktop update manifest contains duplicate targets');
    targets.add(target);
    return { platform, arch, file, bytes, sha256 };
  });
  return { version: version(release.version), assets };
}

async function consume(url: URL, limit: number, signal: AbortSignal, receive: (chunk: Uint8Array) => void | Promise<void>): Promise<number> {
  const response = await fetch(url, { redirect: 'error', signal, headers: { 'Accept-Encoding': 'identity' } });
  if (!response.ok || !response.body) {
    await response.body?.cancel().catch(() => {});
    throw new Error(`Desktop update server returned HTTP ${response.status}`);
  }
  const reader = response.body.getReader();
  let bytes = 0;
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) return bytes;
      bytes += next.value.byteLength;
      if (bytes > limit) throw new Error('Desktop update response exceeded its expected size');
      await receive(next.value);
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

/** Downloads a verified portable bundle without opening or applying it. */
export class DesktopUpdates {
  private readonly closing = new AbortController();
  private staging: Promise<{ version: string; sha256: string }> | undefined;
  private preparing: Promise<unknown> | undefined;

  constructor(private readonly dataDirectory: string, private readonly policyPath: URL) {}

  async close(): Promise<void> {
    this.closing.abort();
    await Promise.allSettled([this.staging, this.preparing]);
  }

  private async fetchManifest(policy: Policy): Promise<Buffer> {
    const chunks: Buffer[] = [];
    await consume(policy.feed, manifestLimit, AbortSignal.any([this.closing.signal, AbortSignal.timeout(10_000)]), chunk => {
      chunks.push(Buffer.from(chunk));
    });
    return Buffer.concat(chunks);
  }

  private inspect(policy: Policy, signedManifest: Buffer): Check {
    const release = releaseDocument(signedManifest, policy.key);
    const { installedVersion } = policy;
    if (!newer(release.version, installedVersion)) return { state: 'current', installedVersion };
    const asset = release.assets.find(entry => entry.platform === process.platform && entry.arch === process.arch);
    if (!asset) return { state: 'unsupported', installedVersion, version: release.version };
    return { state: 'available', installedVersion, version: release.version, asset };
  }

  async check(): Promise<Check> {
    const policy = await loadPolicy(this.policyPath);
    if (!policy) return { state: 'disabled' };
    let manifest: Buffer;
    try {
      manifest = await this.fetchManifest(policy);
    } catch (error) {
      this.closing.signal.throwIfAborted();
      const cached = await this.cachedUpdate(policy);
      if (cached) return cached;
      throw error;
    }
    const candidate = this.inspect(policy, manifest);
    return candidate.state === 'available' ? (await this.cachedUpdate(policy, candidate) ?? candidate) : candidate;
  }

  private async cachedUpdate(policy: Policy, selected?: Available): Promise<Available | undefined> {
    const directory = join(this.dataDirectory, 'desktop-updates');
    const names = selected ? [`${selected.asset.sha256}.manifest.json`] : await readdir(directory).catch(error => {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [];
      throw error;
    });
    let best: Available | undefined;
    for (const name of names) {
      if (!/^[a-f0-9]{64}\.manifest\.json$/.test(name)) continue;
      try {
        const attestation = join(directory, name);
        const manifest = await lstat(attestation);
        if (!manifest.isFile() || manifest.size > manifestLimit) continue;
        const candidate = this.inspect(policy, await readFile(attestation));
        if (candidate.state !== 'available' || name !== `${candidate.asset.sha256}.manifest.json`
          || (selected && selected.version !== candidate.version) || (best && !newer(candidate.version, best.version))) continue;
        const path = join(directory, `${candidate.asset.sha256}.zip`);
        const archive = await lstat(path);
        if (archive.isFile() && archive.size === candidate.asset.bytes && await hashDesktopFile(path, this.closing.signal) === candidate.asset.sha256) {
          best = { ...candidate, staged: true };
        }
      } catch {
        this.closing.signal.throwIfAborted();
        // A stale or invalid download can be replaced by another signed download.
      }
    }
    return best;
  }

  stage(expectedVersion: string, expectedSha256: string): Promise<{ version: string; sha256: string }> {
    if (this.staging || this.preparing) throw new Error('A desktop update is already downloading or preparing');
    this.staging = this.download(expectedVersion, expectedSha256).finally(() => { this.staging = undefined; });
    return this.staging;
  }

  private async download(expectedVersion: string, expectedSha256: string) {
    this.closing.signal.throwIfAborted();
    const policy = await loadPolicy(this.policyPath);
    if (!policy) throw new Error('This desktop package has no update channel');
    const signedManifest = await this.fetchManifest(policy);
    const checked = this.inspect(policy, signedManifest);
    if (checked.state !== 'available' || checked.version !== expectedVersion || checked.asset.sha256 !== expectedSha256) {
      throw new Error('The available desktop update changed. Check again before downloading.');
    }
    const { asset } = checked;
    const directory = join(this.dataDirectory, 'desktop-updates');
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const pending = join(directory, `${randomUUID()}.part`);
    const attestation = `${pending}.manifest`;
    const file = await open(pending, 'wx', 0o600);
    try {
      const hash = createHash('sha256');
      const signal = AbortSignal.any([this.closing.signal, AbortSignal.timeout(180_000)]);
      const bytes = await consume(new URL(asset.file, policy.feed), asset.bytes, signal, async chunk => {
        hash.update(chunk);
        await file.writeFile(chunk);
      });
      if (bytes !== asset.bytes || hash.digest('hex') !== asset.sha256) throw new Error('Desktop update archive verification failed');
      await file.sync();
      await file.close();
      this.closing.signal.throwIfAborted();
      await writeFile(attestation, signedManifest, { flag: 'wx', mode: 0o600, flush: true });
      await rename(pending, join(directory, `${asset.sha256}.zip`));
      await rename(attestation, join(directory, `${asset.sha256}.manifest.json`));
      return { version: checked.version, sha256: asset.sha256 };
    } finally {
      await file.close();
      await rm(pending, { force: true });
      await rm(attestation, { force: true });
    }
  }

  prepare(expectedVersion: string, sha256: string): Promise<Prepared> {
    return this.prepareExclusively(() => this.preparePackage(expectedVersion, sha256));
  }

  prepareInstaller(expectedVersion: string, sha256: string): Promise<Installer> {
    return this.prepareExclusively(async () => {
      const policy = await loadPolicy(this.policyPath);
      if (!policy) throw new Error('This desktop package has no update channel');
      const signal = this.closing.signal;
      const resources = dirname(fileURLToPath(this.policyPath));
      const installation = await realpath(resolve(resources, process.platform === 'darwin' ? '../..' : '..'));
      const current = desktopPackagePaths(installation);
      if (await realpath(resources) !== current.resources || await realpath(process.execPath) !== current.node) {
        throw new Error('Updates require the installed desktop and its bundled Node');
      }
      const dataDirectory = await realpath(this.dataDirectory);
      const relativeData = relative(installation, dataDirectory);
      if (!relativeData || (!isAbsolute(relativeData) && relativeData !== '..' && !relativeData.startsWith(`..${sep}`))) {
        throw new Error('Desktop update cannot move an installation containing its own user data');
      }
      await verifyDesktopPackage(installation, policy.installedVersion, signal);
      const prepared = await this.preparePackage(expectedVersion, sha256, dirname(installation));
      try {
        const next = join(prepared.directory, 'next');
        await rename(prepared.packageRoot, next);
        const plan: DesktopInstallPlan = {
          installation, dataDirectory, fromVersion: policy.installedVersion, toVersion: expectedVersion,
          oldExecutableSha256: await hashDesktopFile(current.executable, signal),
          newExecutableSha256: await hashDesktopFile(desktopPackagePaths(next).executable, signal),
        };
        const executable = join(prepared.directory, process.platform === 'win32' ? 'node.exe' : 'node');
        const program = join(prepared.directory, 'installer.mjs');
        await copyFile(current.node, executable);
        if (process.platform === 'darwin') await chmod(executable, 0o700);
        await copyFile(new URL('./desktop-update-installer.js', import.meta.url), program);
        await writeFile(join(prepared.directory, 'install-plan.json'), JSON.stringify(plan), { flag: 'wx', mode: 0o600, flush: true });
        signal.throwIfAborted();
        return { executable, program, version: prepared.version, sha256: prepared.sha256 };
      } catch (error) {
        await rm(prepared.directory, { recursive: true, force: true });
        throw error;
      }
    });
  }

  private prepareExclusively<T>(operation: () => Promise<T>): Promise<T> {
    if (this.staging || this.preparing) throw new Error('A desktop update is already downloading or preparing');
    const task = operation().finally(() => { this.preparing = undefined; });
    this.preparing = task;
    return task;
  }

  private async preparePackage(expectedVersion: string, sha256: string, besideInstallation?: string): Promise<Prepared> {
    if (!/^[a-f0-9]{64}$/.test(sha256)) throw new Error('Invalid desktop update digest');
    version(expectedVersion);
    const signal = this.closing.signal;
    signal.throwIfAborted();
    const policy = await loadPolicy(this.policyPath);
    if (!policy) throw new Error('This desktop package has no update channel');
    const staging = await realpath(join(this.dataDirectory, 'desktop-updates'));
    const attestation = join(staging, `${sha256}.manifest.json`);
    const info = await lstat(attestation);
    if (!info.isFile() || info.size > manifestLimit) throw new Error('Invalid cached desktop update manifest');
    const checked = this.inspect(policy, await readFile(attestation));
    if (checked.state !== 'available' || checked.version !== expectedVersion || checked.asset.sha256 !== sha256) {
      throw new Error('The cached desktop update does not match the selected release');
    }
    const archive = join(staging, `${sha256}.zip`);
    const archiveInfo = await lstat(archive);
    if (!archiveInfo.isFile() || archiveInfo.size !== checked.asset.bytes) throw new Error('Staged desktop update archive verification failed');

    const directory = await mkdtemp(besideInstallation ? join(besideInstallation, '.hive-desktop-install-') : join(staging, 'prepare-'));
    try {
      const snapshot = join(directory, 'bundle.zip');
      await copyFile(archive, snapshot);
      if ((await lstat(snapshot)).size !== checked.asset.bytes || await hashDesktopFile(snapshot, signal) !== sha256) {
        throw new Error('Staged desktop update archive verification failed');
      }
      const packageRoot = await extractDesktopUpdate(snapshot, join(directory, 'package'), signal);
      await verifyDesktopPackage(packageRoot, expectedVersion, signal);
      signal.throwIfAborted();
      await rm(snapshot);
      return { directory, packageRoot, version: expectedVersion, sha256 };
    } catch (error) {
      await rm(directory, { recursive: true, force: true });
      throw error;
    }
  }
}
