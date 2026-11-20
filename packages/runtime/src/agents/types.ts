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
export interface AcpInteraction {
  id: string; backend: 'acp'; method: 'session/request_permission';
  nativeRequestId: string | number; sessionId: string; state: 'pending' | 'responding'; params: Record<string, unknown>;
}
export type AgentInteraction = AcpInteraction | CodexInteraction | ExternalPiInteraction | ClaudeInteraction;
export type InteractionResolutionReason = 'resolved' | 'turn-ended' | 'aborted' | 'disconnected' | 'disposed' | 'expired';

export type PiEvent =
  | { type: 'session_mode_changed'; sessionId: string }
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
  vault?: import('../pi-vault.js').PiVault;
  sessionFile?: string;
  model?: { provider: string; id: string };
  onEvent: (event: PiEvent) => void;
  customTools?: ToolDefinition[];
  onUnexpectedExit?: (error: Error) => void;
}

export interface NativeCommand { name: string; description?: string; source: 'extension' | 'prompt' | 'skill'; sourceInfo: import('@earendil-works/pi-coding-agent').SourceInfo }

export interface ReasoningState { level: string; levels: string[]; description?: string }

export type CodexWorkMode = 'native-default' | 'native-plan';
export function isCodexWorkMode(value: unknown): value is CodexWorkMode {
  return value === 'native-default' || value === 'native-plan';
}

export interface SessionModeState {
  backend: string; selected: string | null; applied: string | null; pendingNextTurn: boolean;
  options: {id: string; label: string; description?: string}[]; description?: string; connectAvailable?: boolean;
}

export interface PiSessionHandle {
  permissionMode?(): SessionModeState | Promise<SessionModeState>;
  setPermissionMode?(mode: string): SessionModeState | Promise<SessionModeState>;
  mode?(): SessionModeState | Promise<SessionModeState>;
  setMode?(mode: string): SessionModeState | Promise<SessionModeState>;
  sessionId: string;
  readonly sessionFile: string | undefined;
  history(): readonly unknown[];
  hasContext(customType: string): boolean;
  snapshot(): { history: readonly unknown[]; historyMessageIds: string[]; activeMessage?: { messageId: string; message: unknown } };
  prompt(text: string, options?: { images?: readonly PromptImage[]; automated?: boolean; deliveryId?: string; presentation?: DeliveryPresentation }): Promise<void>;
  /** Native same-turn delivery. True only after native consumption and successful completion;
   * For explicit user input, false confirms non-delivery; pending/acknowledged/
   * uncertain attempts must not return false. System/report retry semantics remain separate. */
  steer?(text: string, options: { deliveryId: string; presentation: DeliveryPresentation }): Promise<boolean>;
  addContext(text: string, customType?: string): Promise<void>;
  completedDeliveryIds(): readonly string[];
  interactions?(): readonly AgentInteraction[];
  validateInteractionResponse?(id: string, response: unknown): void;
  respondInteraction?(id: string, response: unknown): void | Promise<void>;
  /** Member retirement may finish host tool acknowledgements; direct user stop stays immediate. */
  abort(options?: { settleTools?: boolean }): Promise<void>;
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
