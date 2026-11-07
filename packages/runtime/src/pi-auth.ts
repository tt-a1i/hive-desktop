import { randomUUID } from 'node:crypto';
import { CredentialSynchronizationError, ModelRuntime, getAgentDir } from '@earendil-works/pi-coding-agent';

type Interaction = Parameters<ModelRuntime['login']>[2];
type Prompt = Parameters<Interaction['prompt']>[0];
type PromptView<T = Prompt> = T extends Prompt ? Omit<T, 'signal'> & { id: string } : never;
type Notice = Parameters<Interaction['notify']>[0];
export interface PiLoginState {
  id: string; providerId: string;
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

/** The SDK owns OAuth and its existing locked credential store. Never serialize credentials. */
export class PiAuth {
  private models?: Promise<ModelRuntime>;
  private login?: Login;
  private closing = false;
  constructor(private readonly createModels = () => ModelRuntime.create({ refreshOnCreate: false })) {}
  private async runtime() {
    if (this.closing) throw new Error('Pi authentication is closing');
    this.models ??= this.createModels().catch(error => { delete this.models; throw error; });
    const models = await this.models;
    if (this.closing) throw new Error('Pi authentication is closing');
    return models;
  }
  async providers() {
    const models = await this.runtime();
    const stored = new Map((await models.listCredentials()).map(info => [info.providerId, info.type]));
    return { agentDir: getAgentDir(), login: this.status(), providers: models.getProviders().filter(provider => provider.auth.oauth).map(provider => ({
      id: provider.id, name: provider.name, label: provider.auth.oauth!.loginLabel ?? provider.auth.oauth!.name,
      storedType: stored.get(provider.id) ?? null,
    })).sort((a, b) => Number(!!b.storedType) - Number(!!a.storedType) || a.name.localeCompare(b.name)) };
  }
  status(): PiLoginState | null { return this.login ? structuredClone(this.login.view) : null; }
  async start(providerId: string): Promise<PiLoginState> {
    const models = await this.runtime();
    if (this.login?.view.state === 'running') throw new Error('请先完成或取消当前登录');
    if (!models.getProvider(providerId)?.auth.oauth) throw new Error('该提供商不支持 OAuth 登录');
    const controller = new AbortController();
    const login: Login = { view: { id: randomUUID(), providerId, state: 'running' }, controller, done: Promise.resolve() };
    this.login = login;
    const timeout = setTimeout(() => controller.abort(new Error('登录超时，请重新开始')), 10 * 60_000);
    timeout.unref();
    login.done = (async () => {
      try {
        await models.login(providerId, 'oauth', {
          signal: controller.signal,
          notify: event => {
            if (controller.signal.aborted) return;
            login.view.notice = event;
            if (event.type === 'auth_url' || event.type === 'device_code') login.view.authorization = event;
          },
          prompt: prompt => this.prompt(login, prompt),
        });
        login.view.state = 'succeeded';
        login.view.message = '登录已保存。可返回会话刷新模型列表并选择模型。';
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
    if (!value.trim()) throw new Error('请输入登录信息');
    const prompt = login.view.prompt;
    if (prompt?.type === 'select' && !prompt.options.some(option => option.id === value)) throw new Error('无效的登录选项');
    login.answer.submit(value);
    return this.status();
  }
  async cancel(id: string) {
    const login = this.login;
    if (!login || login.view.id !== id) throw new Error('登录已结束');
    if (login.view.state === 'running') login.controller.abort(new Error('登录已取消'));
    await login.done;
    return structuredClone(login.view);
  }
  async close() {
    this.closing = true;
    this.login?.controller.abort(new Error('后台已关闭，登录已取消'));
    await this.login?.done;
  }
}
