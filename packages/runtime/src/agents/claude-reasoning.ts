import type { ModelInfo, Query } from '@anthropic-ai/claude-agent-sdk';
import { claudeDeadline } from './claude-input.js';
import type { ReasoningState } from './types.js';

/** SDK 0.3.317 has this public runtime control but omits it from Query's .d.ts.
 * Keep the narrow version-bound bridge here; never use the private request/transport.
 */
export async function readClaudeReasoning(query: Query, models: readonly ModelInfo[]): Promise<ReasoningState & {modelId:string}> {
  const control = query as Query & {getSettings?:()=>Promise<{applied?:{model?:unknown;effort?:unknown}}>} ;
  if (typeof control.getSettings !== 'function') throw new Error('此 Claude SDK 没有原生配置读取能力，请更新已配置的版本。');
  const result = await claudeDeadline(control.getSettings(), 'read reasoning');
  const applied = result.applied;
  if (!applied || typeof applied.model !== 'string' || (applied.effort != null && typeof applied.effort !== 'string')) throw new Error('Claude 未返回有效的会话模型和推理设置。');
  const model = models.find(model=>model.value === applied.model || model.resolvedModel === applied.model);
  const levels = model?.supportsEffort === false ? [] : model?.supportedEffortLevels ?? [];
  return {modelId:applied.model,level:typeof applied.effort === 'string' ? applied.effort : 'native', levels:[...levels],
    description:'读取原生实际强度；环境、组织限制或模型能力可能调整请求值。'};
}
