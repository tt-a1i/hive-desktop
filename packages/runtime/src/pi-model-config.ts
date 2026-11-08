import { join } from 'node:path';
import { getAgentDir } from '@earendil-works/pi-coding-agent';
import { z } from 'zod';
import { commitConfig, createConfigRuntime, edit, object, PiModelConfigError, readConfig, type ObjectValue } from './pi-model-file.js';

const apis = ['openai-completions', 'openai-responses', 'anthropic-messages', 'google-generative-ai'] as const;
const name = z.string().trim().max(256).nullable();
const id = z.string().min(1).max(512).refine(value => !!value.trim() && !/[\u0000-\u001f]/.test(value), 'ID 不能为空或包含控制字符');
const provider = z.object({ action: z.literal('provider'), id, name, baseUrl: z.string().trim().max(4096).nullable(), api: name, firstModelId: id.nullable() }).strict();
const model = z.object({ action: z.literal('model'), id, modelId: id, name,
  contextWindow: z.number().int().positive().nullable(), maxTokens: z.number().int().positive().nullable(), reasoning: z.boolean().nullable(), vision: z.boolean().nullable() }).strict();
const removeProvider = z.object({ action: z.literal('removeProvider'), id }).strict();
const removeModel = z.object({ action: z.literal('removeModel'), id, modelId: id }).strict();
const changeSchema = z.discriminatedUnion('action', [provider, model, removeProvider, removeModel]);
const string = (value: unknown) => typeof value === 'string' ? value : null;
const number = (value: unknown) => typeof value === 'number' ? value : null;
function models(provider: ObjectValue) {
  if (provider.models === undefined) return [];
  if (!Array.isArray(provider.models)) throw new PiModelConfigError('提供商 models 字段应为数组');
  const result = provider.models.map(object);
  if (result.some(model => typeof model.id !== 'string' || !model.id)) throw new PiModelConfigError('模型缺少有效 ID，请先修复配置');
  if (new Set(result.map(model => model.id)).size !== result.length) throw new PiModelConfigError('提供商包含重复模型 ID，请先修复配置');
  return result;
}
function modelOverride(provider: ObjectValue, id: string): ObjectValue {
  if (provider.modelOverrides === undefined) return {};
  const overrides = object(provider.modelOverrides);
  return Object.hasOwn(overrides, id) ? object(overrides[id]) : {};
}

/** Only common form fields cross the wire; credentials and advanced fields stay in models.json. */
export class PiModelConfig {
  private pending: Promise<unknown> = Promise.resolve();
  private closing = false;
  constructor(private readonly path = join(getAgentDir(), 'models.json')) {}
  get() {
    if (this.closing) throw new PiModelConfigError('模型配置正在关闭');
    return this.snapshot();
  }
  private snapshot() {
    const state = readConfig(this.path);
    return { path: this.path, revision: state.revision, apis: [...apis], providers: Object.entries(object(state.config.providers)).map(([id, value]) => {
      const provider = object(value);
      const entries = models(provider);
      const overrides = provider.modelOverrides === undefined ? {} : object(provider.modelOverrides);
      const ids = [...new Set([...entries.map(model => String(model.id)), ...Object.keys(overrides)])];
      return { id, name: string(provider.name), baseUrl: string(provider.baseUrl), api: string(provider.api), models: ids.map(modelId => {
        const raw = entries.find(model => model.id === modelId);
        const explicit = { ...raw, ...modelOverride(provider, modelId) };
        return { id: modelId, name: string(explicit.name), contextWindow: number(explicit.contextWindow), maxTokens: number(explicit.maxTokens),
          reasoning: !raw && explicit.reasoning === undefined ? null : explicit.reasoning === true,
          vision: !raw && explicit.input === undefined ? null : Array.isArray(explicit.input) && explicit.input.includes('image'),
          overrideOnly: !raw };
      }) };
    }).sort((a, b) => a.id.localeCompare(b.id)) };
  }
  async change(revision: unknown, value: unknown) {
    if (this.closing) return Promise.reject(new PiModelConfigError('模型配置正在关闭'));
    if (typeof revision !== 'string' || !/^[a-f0-9]{64}$/.test(revision)) return Promise.reject(new PiModelConfigError('请先读取当前配置'));
    const input = changeSchema.parse(value);
    const operation = this.pending.then(async () => {
      const state = readConfig(this.path);
      if (state.revision !== revision) return { conflict: true, snapshot: this.snapshot() };
      const providers = object(state.config.providers);
      const existing = Object.hasOwn(providers, input.id) ? object(providers[input.id]) : undefined;
      let text = state.text;
      const set = (path: (string | number)[], value: unknown) => { text = edit(text, ['providers', input.id, ...path], value); };
      if (input.action === 'provider') {
        if (!existing) {
          if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$/.test(input.id)) throw new PiModelConfigError('新提供商 ID 请使用字母、数字、点、下划线或连字符');
          const builtin = await createConfigRuntime(null);
          if (!builtin.getProvider(input.id) && (!input.api || !input.baseUrl || !input.firstModelId)) throw new PiModelConfigError('新接口需要类型、地址和首个模型 ID');
          set([], {});
        }
        if (input.api && !apis.includes(input.api as typeof apis[number]) && input.api !== existing?.api) throw new PiModelConfigError('请选择支持的接口类型');
        if (input.baseUrl) {
          let url: URL;
          try { url = new URL(input.baseUrl); } catch { throw new PiModelConfigError('请输入有效的 HTTP 或 HTTPS 接口地址'); }
          if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new PiModelConfigError('接口地址需要 HTTP 或 HTTPS，且不能包含用户名或密码');
        }
        for (const key of ['name', 'baseUrl', 'api'] as const) set([key], input[key] || undefined);
        if (!existing && input.firstModelId) set(['models'], [{ id: input.firstModelId }]);
      } else {
        if (!existing) throw new PiModelConfigError('提供商配置已不存在，请重新读取');
        if (input.action === 'removeProvider') set([], undefined);
        else {
          const entries = models(existing);
          const modelId = input.modelId;
          const index = entries.findIndex(model => model.id === modelId);
          const overrideOnly = index < 0 && !!existing.modelOverrides && Object.hasOwn(object(existing.modelOverrides), modelId);
          if (input.action === 'removeModel') {
            if (index < 0 && !overrideOnly) throw new PiModelConfigError('模型配置已不存在，请重新读取');
            if (index >= 0) set(['models', index], undefined);
            if (existing.modelOverrides && Object.hasOwn(object(existing.modelOverrides), modelId)) set(['modelOverrides', modelId], undefined);
            // Pi 1.4 rejects a provider containing only an empty models/override map.
            const remainingModels = entries.length - (index >= 0 ? 1 : 0);
            const remainingOverrides = Object.keys(object(existing.modelOverrides ?? {})).filter(id => id !== modelId).length;
            if (!remainingModels && !remainingOverrides && !existing.baseUrl && !existing.headers && !existing.compat
              && !existing.apiKey && !existing.oauth && existing.authHeader === undefined) set([], undefined);
          } else {
            const position = index < 0 ? entries.length : index;
            const override = modelOverride(existing, input.modelId);
            const field = (key: string, value: unknown) => {
              if (overrideOnly) { set(['modelOverrides', modelId, key], value); return; }
              set(['models', position, key], value);
              if (Object.hasOwn(override, key)) set(['modelOverrides', input.modelId, key], value);
            };
            if (index < 0 && !overrideOnly) set(['models', -1], { id: input.modelId });
            for (const key of ['name', 'contextWindow', 'maxTokens'] as const) field(key, input[key] === null || input[key] === '' ? undefined : input[key]);
            if (overrideOnly || input.reasoning === true || entries[index]?.reasoning !== undefined || override.reasoning !== undefined) field('reasoning', input.reasoning ?? undefined);
            // Preserve any other SDK input modalities while changing the image checkbox.
            const prior = override.input ?? entries[index]?.input;
            if (overrideOnly || input.vision === true || prior !== undefined) field('input', input.vision === null ? undefined : [...new Set([...(Array.isArray(prior) ? prior.filter(value => value !== 'image') : ['text']), ...(input.vision ? ['image'] : [])])]);
          }
        }
      }
      const saved = await commitConfig(this.path, revision, text);
      return { conflict: !saved, snapshot: this.snapshot() };
    });
    this.pending = operation.catch(() => {});
    return operation;
  }
  async close() { this.closing = true; await this.pending; }
}
