import type { PromptImage } from '../prompt-images.js';
import type { DeliveryPresentation } from '../delivery-presentation.js';
import type { ToolDefinition } from '@earendil-works/pi-coding-agent';

export interface CodexInteraction {
  id: string; backend: 'codex';
  method: 'item/commandExecution/requestApproval' | 'item/fileChange/requestApproval' | 'item/tool/requestUserInput';
  threadId: string; turnId: string; itemId: string; nativeRequestId: string | number;
  state: 'pending' | 'responding'; params: Record<string, unknown>; item?: unknown;
}
export interface ExternalPiInteraction {
  id: string; backend: 'pi-rpc'; method: 'select' | 'confirm' | 'input' | 'editor';
  nativeRequestId: string; state: 'pending' | 'responding'; params: Record<string, unknown>; expiresAt?: number;
}
export interface ClaudeInteraction {
  id: string; backend: 'claude'; method: 'tool-permission' | 'ask-user-question';
  nativeRequestId: string; turnId: string; itemId: string; state: 'pending' | 'responding'; params: Record<string, unknown>;
}
export type AgentInteraction = CodexInteraction | ExternalPiInteraction | ClaudeInteraction;
export type InteractionResolutionReason = 'resolved' | 'turn-ended' | 'aborted' | 'disconnected' | 'disposed' | 'expired';

export type PiEvent =
  | { type: 'interaction'; sessionId: string; interaction: AgentInteraction }
  | { type: 'interaction_resolved'; sessionId: string; interactionId: string; reason: InteractionResolutionReason }
  | { type: 'assistant_delta'; sessionId: string; messageId: string; kind: 'text' | 'thinking'; contentIndex: number; delta: string }
  | { type: 'message_end'; sessionId: string; messageId: string; message: unknown }
  | { type: 'tool_start'; sessionId: string; toolCallId: string; toolName: string; args: unknown }
  | { type: 'tool_update'; sessionId: string; toolCallId: string; toolName: string; result: unknown }
  | { type: 'tool_end'; sessionId: string; toolCallId: string; toolName: string; result: unknown; isError: boolean }
  | { type: 'settled'; sessionId: string; aborted: boolean }
  | { type: 'error'; sessionId: string; message: string; diagnosticWriteFailed?: boolean };

export interface PiSessionOptions {
  cwd: string;
  sessionDir: string;
  agentDir?: string;
  sessionFile?: string;
  model?: { provider: string; id: string };
  onEvent: (event: PiEvent) => void;
  customTools?: ToolDefinition[];
}

export interface NativeCommand { name: string; description?: string; source: 'extension' | 'prompt' | 'skill'; sourceInfo: import('@earendil-works/pi-coding-agent').SourceInfo }

export interface ReasoningState { level: string; levels: string[]; description?: string }

export interface PiSessionHandle {
  sessionId: string;
  readonly sessionFile: string | undefined;
  history(): readonly unknown[];
  hasContext(customType: string): boolean;
  snapshot(): { history: readonly unknown[]; historyMessageIds: string[]; activeMessage?: { messageId: string; message: unknown } };
  prompt(text: string, options?: { images?: readonly PromptImage[]; automated?: boolean; deliveryId?: string; presentation?: DeliveryPresentation }): Promise<void>;
  addContext(text: string, customType?: string): Promise<void>;
  completedDeliveryIds(): readonly string[];
  interactions?(): readonly AgentInteraction[];
  respondInteraction?(id: string, response: unknown): void | Promise<void>;
  abort(): Promise<void>;
  dispose(): void | Promise<void>;
  supportsImages?(): boolean;
  currentModel(): { provider: string; id: string; name: string } | undefined;
  refreshModels?(): Promise<void>;
  listModels(): { provider: string; id: string; name: string; available: boolean }[];
  setModel(provider: string, id: string): Promise<void>;
  usage?(): import("../agent-usage.js").UsageSnapshot | Promise<import("../agent-usage.js").UsageSnapshot>;
  commands?(): NativeCommand[] | Promise<NativeCommand[]>;
  reasoning?(): ReasoningState | Promise<ReasoningState>;
  setReasoning?(level: string): ReasoningState | Promise<ReasoningState>;
}
