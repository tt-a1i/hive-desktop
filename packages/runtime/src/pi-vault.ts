import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { accessSync, constants, realpathSync, statSync } from 'node:fs';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { delimiter, dirname, isAbsolute, join } from 'node:path';
import { getAgentDir } from '@earendil-works/pi-coding-agent';
import { ownedWindowsCliLaunch } from './agents/owned-cli-process.js';
import { createConfigRuntime, object, readConfig } from './pi-model-file.js';
import type { ModelRuntime, ModelRuntimeAuthOverrides } from '@earendil-works/pi-coding-agent';

type Binding = { provider: string; reference: string; endpoint: string; api: string; fingerprint: string };
type Store = { revision: string; agentDirectory: string; bindings: Binding[] };
type PendingRead = { binding: Binding; controller: AbortController };
const emptyRevision = 'missing';
const vaultFile = 'pi-vault.json';

export class VaultError extends Error {}

function sameBinding(left: Binding, right: Binding) {
  return left.provider === right.provider && left.reference === right.reference &&
    left.endpoint === right.endpoint && left.api === right.api && left.fingerprint === right.fingerprint;
}

function bindingChanged() {
  return new VaultError('模型配置或密码库引用已变化，请刷新后重试。');
}

function reference(value: unknown): string {
  if (typeof value !== 'string' || value.length > 2048 || !value.startsWith('op://')
    || /[\u0000-\u001f\u007f-\u009f?#\\]/u.test(value) || value.trim() !== value) {
    throw new VaultError('请输入 1Password 字段引用（op://密码库/项目/字段）。');
  }
  const parts = value.slice(5).split('/');
  if ((parts.length !== 3 && parts.length !== 4) || parts.some(part => !part.trim() || part === '.' || part === '..')) {
    throw new VaultError('请输入 1Password 字段引用（op://密码库/项目/字段）。');
  }
  return value;
}

function endpoint(value: unknown): string {
  if (typeof value !== 'string') throw new VaultError('请先为自定义接口保存有效地址。');
  let url: URL;
  try { url = new URL(value); }
  catch { throw new VaultError('请先为自定义接口保存有效地址。'); }
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
  if ((url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) ||
    url.username || url.password || url.hash || url.search) {
    throw new VaultError('密码库凭据仅可用于 HTTPS 或本机回环模型接口。');
  }
  return url.href.replace(/\/$/, '');
}

function resolveOp(): string {
  const win = process.platform === 'win32';
  const name = win ? 'op.exe' : 'op';
  const local = process.env.LOCALAPPDATA;
  const paths = [...(process.env.PATH ?? '').split(delimiter), '/opt/homebrew/bin', '/usr/local/bin',
    ...(win && local ? [join(local, 'Microsoft/WinGet/Links')] : [])];
  for (const path of paths) {
    if (!isAbsolute(path)) continue;
    try {
      const file = realpathSync(join(path, name));
      if (!statSync(file).isFile()) continue;
      accessSync(file, win ? constants.F_OK : constants.X_OK);
      return file;
    } catch { /* Try the next absolute installation directory. */ }
  }
  throw new VaultError('未找到 1Password CLI，请安装 op 并启用桌面应用集成。');
}

async function readOp(value: string, signal: AbortSignal): Promise<string> {
  signal.throwIfAborted();
  const command = resolveOp();
  const args = ['read', '--no-newline', value];
  const windows = process.platform === 'win32';
  const launch = windows ? ownedWindowsCliLaunch(command, args) : { command, args };
  const child = spawn(launch.command, launch.args, {
    cwd: dirname(command), detached: !windows, shell: false, windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'],
  });
  const capacity = 16 * 1024;
  const bytes = Buffer.alloc(capacity);
  const deadline = AbortSignal.any([signal, AbortSignal.timeout(45_000)]);
  let count = 0;
  let overflow = false;
  let startError = false;
  const stop = () => {
    try {
      if (windows || !child.pid) child.kill('SIGKILL');
      else process.kill(-child.pid, 'SIGKILL');
    } catch {
      child.kill('SIGKILL');
    } finally {
      // A helper outside the owned group can still hold a copy of the pipe.
      child.stdout.destroy();
    }
  };
  deadline.addEventListener('abort', stop, { once: true });
  child.on('error', () => { startError = true; });
  child.stdout.on('data', (chunk: Buffer) => {
    if (count + chunk.length > capacity) { overflow = true; stop(); }
    else { chunk.copy(bytes, count); count += chunk.length; }
    chunk.fill(0);
  });
  if (deadline.aborted) stop();
  try {
    const code = await new Promise<number | null>(resolve => child.once('close', resolve));
    signal.throwIfAborted();
    if (deadline.aborted) throw new VaultError('1Password 读取超时，请确认桌面应用已解锁。');
    if (startError) throw new VaultError('无法启动 1Password CLI。');
    if (overflow) throw new VaultError('1Password 字段不是可用的模型密钥。');
    if (code !== 0) throw new VaultError('无法读取 1Password 字段，请检查引用及密码库授权。');
    const secret = bytes.subarray(0, count).toString('utf8');
    if (!secret || secret.trim() !== secret || /[\u0000-\u001f\u007f-\u009f\ufffd]/u.test(secret)) {
      throw new VaultError('1Password 字段不是可用的模型密钥。');
    }
    return secret;
  } finally {
    deadline.removeEventListener('abort', stop);
    bytes.fill(0);
  }
}

function providerFingerprint(value: Record<string, unknown>) {
  return createHash('sha256').update(JSON.stringify(value)).digest('hex');
}

function providerDefinition(config: ReturnType<typeof readConfig>, id: string) {
  const providers = object(config.config.providers);
  const value = Object.hasOwn(providers, id) ? object(providers[id]) : undefined;
  if (!value || !Array.isArray(value.models) || !value.models.length || typeof value.api !== 'string'
    || !['openai-responses', 'openai-completions'].includes(value.api)
    || value.apiKey !== undefined || value.oauth !== undefined || value.headers !== undefined) {
    throw new VaultError('仅支持已有模型、且未设置其他认证的自定义接口。');
  }
  return { endpoint: endpoint(value.baseUrl), api: value.api, fingerprint: providerFingerprint(value) };
}

export class PiVault {
  private tail: Promise<unknown> = Promise.resolve();
  private readonly abort = new AbortController();
  private readonly pending = new Map<Promise<string>, PendingRead>();
  readonly agentDirectory = getAgentDir();

  constructor(private readonly dataDir: string) {}

  private get path() { return join(this.dataDir, vaultFile); }

  private async store(): Promise<Store> {
    let contents: string;
    try { contents = await readFile(this.path, 'utf8'); }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return { revision: emptyRevision, agentDirectory: this.agentDirectory, bindings: [] };
      }
      throw new VaultError('无法读取 Hive 的密码库引用设置。');
    }
    try {
      const raw = JSON.parse(contents) as Store & { version: number };
      if (raw.version !== 1 || typeof raw.revision !== 'string' || !isAbsolute(raw.agentDirectory) || !Array.isArray(raw.bindings)) throw new Error();
      const ids = new Set<string>();
      for (const item of raw.bindings) {
        if (typeof item.provider !== 'string' || ids.has(item.provider) || typeof item.api !== 'string'
          || !/^[a-f0-9]{64}$/.test(item.fingerprint) || endpoint(item.endpoint) !== item.endpoint) throw new Error();
        reference(item.reference);
        ids.add(item.provider);
      }
      return raw;
    } catch { throw new VaultError('Hive 的密码库引用设置无效。'); }
  }

  async view() {
    const store = await this.store();
    const config = readConfig(join(this.agentDirectory, 'models.json'));
    const builtin = await createConfigRuntime(null);
    const eligible = Object.keys(object(config.config.providers)).flatMap(provider => {
      if (builtin.getProvider(provider)) return [];
      try {
        const definition = providerDefinition(config, provider);
        return [{ provider, endpoint: definition.endpoint }];
      } catch { return []; }
    });
    return {
      revision: store.revision, configRevision: config.revision, eligible,
      configured: store.agentDirectory === this.agentDirectory
        ? store.bindings.map(item => ({ provider: item.provider, endpoint: item.endpoint, stale: !this.matches(config, item) })) : [],
    };
  }

  private matches(config: ReturnType<typeof readConfig>, binding: Binding) {
    try { return providerDefinition(config, binding.provider).fingerprint === binding.fingerprint; }
    catch { return false; }
  }

  change(revision: unknown, configRevision: unknown, provider: string, ref: unknown) {
    if (typeof revision !== 'string' || typeof configRevision !== 'string') throw new VaultError('请先读取当前密码库设置。');
    const value = ref === null ? null : reference(ref);
    const result = this.tail.then(async () => {
      this.abort.signal.throwIfAborted();
      const store = await this.store();
      const config = readConfig(join(this.agentDirectory, 'models.json'));
      if (store.revision !== revision || config.revision !== configRevision) {
        throw bindingChanged();
      }
      const bindings = store.agentDirectory === this.agentDirectory
        ? store.bindings.filter(item => item.provider !== provider) : [];
      if (value !== null) {
        if ((await createConfigRuntime(null)).getProvider(provider)) throw new VaultError('内置厂商账号继续使用原有认证。');
        bindings.push({ provider, reference: value, ...providerDefinition(config, provider) });
      }
      this.abort.signal.throwIfAborted();
      if (readConfig(join(this.agentDirectory, 'models.json')).revision !== configRevision) {
        throw bindingChanged();
      }
      await mkdir(this.dataDir, { recursive: true, mode: 0o700 });
      const stage = join(this.dataDir, `.pi-vault-${randomUUID()}.tmp`);
      try {
        await writeFile(stage, JSON.stringify({ version: 1, revision: randomUUID(),
          agentDirectory: this.agentDirectory, bindings }), { flag: 'wx', mode: 0o600, flush: true });
        await rename(stage, this.path);
        for (const {binding, controller} of this.pending.values())
          if (!bindings.some(current => sameBinding(current, binding))) controller.abort(bindingChanged());
      } finally { await rm(stage, { force: true }); }
      return this.view();
    });
    this.tail = result.catch(() => {});
    return result;
  }

  async bindings(agentDirectory: string): Promise<Binding[]> {
    if (agentDirectory !== this.agentDirectory) return [];
    const store = await this.store();
    return store.agentDirectory === agentDirectory ? store.bindings : [];
  }

  assertTarget(binding: Binding, model: { baseUrl: string; api: string }) {
    const config = readConfig(join(this.agentDirectory, 'models.json'));
    if (!this.matches(config, binding) || endpoint(model.baseUrl) !== binding.endpoint || model.api !== binding.api) {
      throw new VaultError('模型接口已变更，请重新确认 1Password 引用。');
    }
  }

  async assertBound(binding: Binding) {
    for (;;) {
      const mutation = this.tail;
      this.abort.signal.throwIfAborted();
      await mutation;
      const store = await this.store();
      this.abort.signal.throwIfAborted();
      // A save can replace the file while it is being read. Check the
      // committed binding after that save, even if the old read succeeded.
      if (mutation !== this.tail) continue;
      if (store.agentDirectory !== this.agentDirectory || !store.bindings.some(current => sameBinding(current, binding)))
        throw bindingChanged();
      return;
    }
  }

  resolve(binding: Binding, signal?: AbortSignal) {
    const controller = new AbortController();
    const lifetime = AbortSignal.any([this.abort.signal, controller.signal, ...(signal ? [signal] : [])]);
    const operation = this.assertBound(binding).then(() => readOp(binding.reference, lifetime));
    this.pending.set(operation, {binding, controller});
    void operation.finally(() => this.pending.delete(operation)).catch(() => {});
    return operation;
  }

  async close() {
    this.abort.abort(new VaultError('桌面后台已关闭。'));
    await Promise.allSettled([this.tail, ...this.pending.keys()]);
  }
}

type Model = NonNullable<ReturnType<ModelRuntime['getModel']>>;

/** Per-session SDK adapter. Reference checks are local and keys exist only during accepted work. */
export class PiVaultSession {
  private current = new Map<string, Binding>();
  private turn: { signal: AbortSignal; keys: Map<string, Promise<string>>; literals: Set<string> } | undefined;

  constructor(private readonly vault: PiVault, private readonly directory: string, private readonly models: ModelRuntime) {
    const getAuth = models.getAuth.bind(models);
    const checkAuth = models.checkAuth.bind(models);
    const hasAuth = models.hasConfiguredAuth.bind(models);
    const available = models.getAvailableSnapshot.bind(models);
    models.getAuth = async (target: Parameters<ModelRuntime['getAuth']>[0] | string, options?: ModelRuntimeAuthOverrides) => {
      const provider = typeof target === 'string' ? target : target.provider;
      const binding = this.current.get(provider);
      if (!binding) return typeof target === 'string' ? getAuth(target, options) : getAuth(target, options);
      const model = typeof target === 'string' ? models.getModels().find(item => item.provider === target) : target;
      if (!model) throw new VaultError('已关联 1Password 的模型不存在。');
      const apiKey = await this.key(binding, model, options?.signal);
      const auth = typeof target === 'string' ? await getAuth(target, { ...options, apiKey })
        : await getAuth(target, { ...options, apiKey });
      await this.vault.assertBound(binding);
      options?.signal?.throwIfAborted();
      this.vault.assertTarget(binding, model);
      if (!auth || (auth.auth.baseUrl && endpoint(auth.auth.baseUrl) !== binding.endpoint)) {
        throw new VaultError('无法为已确认的模型接口配置密码库凭据。');
      }
      return auth;
    };
    models.checkAuth = async (provider, options) => {
      const binding = this.current.get(provider);
      if (!binding) return checkAuth(provider, options);
      return this.hasQualifiedModel(binding) ? { type: 'api_key', source: '1Password' } : undefined;
    };
    models.hasConfiguredAuth = provider => {
      const binding = this.current.get(provider);
      return binding ? this.hasQualifiedModel(binding) : hasAuth(provider);
    };
    models.getAvailableSnapshot = () => this.current.size ? [
      ...available().filter(model => !this.current.has(model.provider)),
      ...models.getModels().filter(model => this.eligible(model)),
    ] : available();
  }

  async reload() {
    this.current = new Map((await this.vault.bindings(this.directory)).map(item => [item.provider, item]));
  }

  private eligible(model: Model) {
    const binding = this.current.get(model.provider);
    if (!binding) return false;
    try { this.vault.assertTarget(binding, model); return true; }
    catch { return false; }
  }

  private hasQualifiedModel(binding: Binding) {
    return this.models.getModels().some(model => model.provider === binding.provider && this.eligible(model));
  }

  async begin(signal: AbortSignal, model: Model | undefined) {
    await this.reload();
    signal.throwIfAborted();
    this.turn = { signal, keys: new Map(), literals: new Set() };
    const binding = model && this.current.get(model.provider);
    if (binding && model) await this.key(binding, model);
  }

  end() { this.turn = undefined; }

  get hasSecret() { return !!this.turn?.literals.size; }

  private async key(binding: Binding, model: { baseUrl: string; api: string }, requestSignal?: AbortSignal) {
    this.vault.assertTarget(binding, model);
    const turn = this.turn;
    if (!turn) throw new VaultError('1Password 凭据仅在发送模型请求时读取。');
    const signal = requestSignal ? AbortSignal.any([turn.signal, requestSignal]) : turn.signal;
    signal.throwIfAborted();
    let readable = turn.keys.get(binding.provider);
    if (!readable) {
      readable = this.vault.resolve(binding, signal).then(secret => {
        turn.literals.add(secret);
        return secret;
      });
      turn.keys.set(binding.provider, readable);
    }
    const secret = await readable;
    await this.vault.assertBound(binding);
    signal.throwIfAborted();
    this.vault.assertTarget(binding, model);
    return secret;
  }

  redact(value: string) {
    if (this.turn) for (const secret of this.turn.literals) value = value.replaceAll(secret, '[REDACTED]');
    return value;
  }

  redactPayload<T>(value: T): T {
    if (!this.hasSecret) return value;
    let changed = false;
    const safe = JSON.stringify(value, (_name, item: unknown) => {
      if (typeof item !== 'string') return item;
      const redacted = this.redact(item);
      changed ||= redacted !== item;
      return redacted;
    });
    return changed ? JSON.parse(safe) as T : value;
  }
}
