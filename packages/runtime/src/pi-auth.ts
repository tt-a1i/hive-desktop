import { randomUUID } from 'node:crypto';
import { CredentialSynchronizationError, ModelRuntime, getAgentDir } from '@earendil-works/pi-coding-agent';

type Interaction = Parameters<ModelRuntime['login']>[2];
type Prompt = Parameters<Interaction['prompt']>[0];
type PromptView<T = Prompt> = T extends Prompt ? Omit<T, 'signal'> & { id: string } : never;
type Notice = Parameters<Interaction['notify']>[0];
type AuthType = Parameters<ModelRuntime['login']>[1];
export interface PiLoginState {
  id: string; providerId: string;
  authType: AuthType;
  state: 'running' | 'succeeded' | 'cancelled' | 'failed';
  notice?: Notice;
  authorization?: Notice;
  prompt?: PromptView;
  message?: string;
}
type Login = {
  view: PiLoginState; controller: AbortController; done: Promise<void>;
  answer?: { id: string; submit(value: string): void };
};

/** The SDK owns authentication and its existing locked credential store. Never serialize credentials. */
export class PiAuth {
  private readonly catalogAbort = new AbortController();
  private models?: Promise<ModelRuntime>;
  private login?: Login;
  private removal?: Promise<Awaited<ReturnType<PiAuth['providers']>> & { message: string }>;
  private closing = false;
  constructor(private readonly createModels = () => ModelRuntime.create({ refreshOnCreate: false })) {}
  private async runtime() {
    if (this.closing) throw new Error('Pi authentication is closing');
    if (!this.models) {
      const pending = this.createModels().catch(error => { if (this.models === pending) delete this.models; throw error; });
      this.models = pending;
    }
    const models = await this.models;
    if (this.closing) throw new Error('Pi authentication is closing');
    return models;
  }
  async providers() {
    // Account metadata must reflect models.json edits; active login keeps its own runtime reference.
    delete this.models;
    const models = await this.runtime();
    return this.catalog(models);
  }
  private async readCredentials(models: ModelRuntime) {
    // Pi's unsignalled read may return the previous snapshot after a disk error.
    // Explicit account queries and logouts must report that error without quoting credential contents.
    return models.listCredentials({ signal: this.catalogAbort.signal }).catch(error => {
      throw new Error('读取本机登录信息失败，请检查 Pi 的 auth.json 文件后重新读取。', { cause: error });
    });
  }
  private async catalog(models: ModelRuntime) {
    const credentials = await this.readCredentials(models);
    const stored = new Map(credentials.map(info => [info.providerId, info.type]));
    const providers = models.getProviders().map(provider => ({
      id: provider.id, name: provider.name,
      methods: [
        ...(provider.auth.oauth ? [{ type: 'oauth' as const, label: provider.auth.oauth.loginLabel ?? provider.auth.oauth.name }] : []),
        ...(provider.auth.apiKey?.login ? [{ type: 'api_key' as const, label: provider.auth.apiKey.name }] : []),
      ],
      storedType: stored.get(provider.id) ?? null, registered: true,
    }));
    for (const [id, type] of stored) {
      if (!models.getProvider(id)) providers.push({ id, name: id, methods: [], storedType: type, registered: false });
    }
    return { agentDir: getAgentDir(), login: this.status(), providers: providers.sort((a, b) => Number(!!b.storedType) - Number(!!a.storedType) || a.name.localeCompare(b.name)) };
  }
  status(): PiLoginState | null { return this.login ? structuredClone(this.login.view) : null; }
  async start(providerId: string, authType: unknown = 'oauth'): Promise<PiLoginState> {
    const models = await this.runtime();
    this.assertIdle();
    if (authType !== 'oauth' && authType !== 'api_key') throw new Error('不支持的认证方式');
    const auth = models.getProvider(providerId)?.auth;
    if (!(authType === 'oauth' ? auth?.oauth : auth?.apiKey?.login)) throw new Error('该提供商不支持此配置方式');
    const controller = new AbortController();
    const login: Login = { view: { id: randomUUID(), providerId, authType, state: 'running' }, controller, done: Promise.resolve() };
    this.login = login;
    const timeout = setTimeout(() => controller.abort(new Error(authType === 'api_key' ? '配置超时，请重新开始' : '登录超时，请重新开始')), 10 * 60_000);
    timeout.unref();
    login.done = (async () => {
      try {
        await models.login(providerId, authType, {
          signal: controller.signal,
          notify: event => {
            if (controller.signal.aborted) return;
            login.view.notice = event;
            if (event.type === 'auth_url' || event.type === 'device_code') login.view.authorization = event;
          },
          prompt: prompt => this.prompt(login, prompt),
        });
        login.view.state = 'succeeded';
        login.view.message = '配置已保存。可返回会话刷新模型列表并选择模型。';
      } catch (error) {
        login.view.state = error instanceof CredentialSynchronizationError ? 'succeeded' : controller.signal.aborted ? 'cancelled' : 'failed';
        login.view.message = error instanceof CredentialSynchronizationError ? '凭据已保存，但模型列表更新失败。请返回会话刷新模型；若仍失败，请检查 Pi 配置。'
          : controller.signal.aborted ? String(controller.signal.reason?.message ?? '登录已取消')
          : error instanceof Error ? error.message : '登录失败';
      } finally {
        clearTimeout(timeout);
        delete login.view.prompt;
        delete login.view.notice;
        delete login.view.authorization;
        delete login.answer;
      }
    })();
    return this.status()!;
  }
  private prompt(login: Login, prompt: Prompt): Promise<string> {
    const signal = prompt.signal ? AbortSignal.any([login.controller.signal, prompt.signal]) : login.controller.signal;
    signal.throwIfAborted();
    if (login.answer) return Promise.reject(new Error('Pi requested overlapping login prompts'));
    const id = randomUUID();
    const { signal: _signal, ...view } = prompt;
    login.view.prompt = { ...view, id };
    return new Promise((resolve, reject) => {
      const cleanup = () => {
        signal.removeEventListener('abort', abort);
        if (login.view.prompt?.id === id) delete login.view.prompt;
        if (login.answer?.id === id) delete login.answer;
      };
      const abort = () => { cleanup(); reject(signal.reason); };
      login.answer = { id, submit: value => { cleanup(); resolve(value); } };
      signal.addEventListener('abort', abort, { once: true });
      if (signal.aborted) abort();
    });
  }
  respond(id: string, promptId: string, value: string) {
    const login = this.login;
    if (!login || login.view.id !== id || login.view.state !== 'running' || login.answer?.id !== promptId) {
      throw new Error('登录步骤已结束，请刷新后继续');
    }
    const prompt = login.view.prompt;
    if (prompt?.type !== 'text' && !value.trim()) throw new Error('请输入登录信息');
    if (prompt?.type === 'select' && !prompt.options.some(option => option.id === value)) throw new Error('无效的登录选项');
    login.answer.submit(value);
    return this.status();
  }
  async cancel(id: string) {
    const login = this.login;
    if (!login || login.view.id !== id) throw new Error('登录已结束');
    if (login.view.state === 'running') login.controller.abort(new Error(login.view.authType === 'api_key' ? '配置已取消' : '登录已取消'));
    await login.done;
    return structuredClone(login.view);
  }
  private assertIdle() {
    if (this.closing) throw new Error('Pi authentication is closing');
    if (this.login?.view.state === 'running' || this.removal) throw new Error('请先完成或取消当前账号操作');
  }
  async logout(providerId: string) {
    const models = await this.runtime();
    this.assertIdle();
    if (!models.getProvider(providerId) && !(await this.readCredentials(models)).some(info => info.providerId === providerId)) throw new Error('提供商不存在');
    this.assertIdle();
    this.removal = (async () => {
      let message = '已移除本机保存的凭据。环境变量等其他登录来源不受影响。';
      try { await models.logout(providerId); }
      catch (error) {
        if (!(error instanceof CredentialSynchronizationError)) throw error;
        message = '凭据已移除，但模型列表更新失败。请返回会话刷新模型。';
      }
      if (this.login?.view.providerId === providerId) delete this.login;
      return { ...await this.catalog(models), message };
    })();
    try { return await this.removal; }
    finally { delete this.removal; }
  }
  async close() {
    this.closing = true;
    this.catalogAbort.abort(new Error('Pi authentication is closing'));
    this.login?.controller.abort(new Error('后台已关闭，登录已取消'));
    await this.login?.done;
    await Promise.allSettled(this.removal ? [this.removal] : []);
  }
}
