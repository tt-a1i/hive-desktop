import type { ToolDefinition } from '@earendil-works/pi-coding-agent';

export type PiEvent =
  | { type: 'assistant_delta'; sessionId: string; messageId: string; kind: 'text' | 'thinking'; contentIndex: number; delta: string }
  | { type: 'message_end'; sessionId: string; messageId: string; message: unknown }
  | { type: 'tool_start'; sessionId: string; toolCallId: string; toolName: string; args: unknown }
  | { type: 'tool_update'; sessionId: string; toolCallId: string; toolName: string; result: unknown }
  | { type: 'tool_end'; sessionId: string; toolCallId: string; toolName: string; result: unknown; isError: boolean }
  | { type: 'settled'; sessionId: string; aborted: boolean }
  | { type: 'error'; sessionId: string; message: string };

export interface PiSessionOptions {
  cwd: string;
  sessionDir: string;
  agentDir?: string;
  sessionFile?: string;
  model?: { provider: string; id: string };
  onEvent: (event: PiEvent) => void;
  customTools?: ToolDefinition[];
}

export interface PiSessionHandle {
  sessionId: string;
  readonly sessionFile: string | undefined;
  history(): readonly unknown[];
  snapshot(): { history: readonly unknown[]; historyMessageIds: string[]; activeMessage?: { messageId: string; message: unknown } };
  prompt(text: string): Promise<void>;
  abort(): Promise<void>;
  dispose(): void;
  currentModel(): { provider: string; id: string; name: string } | undefined;
  listModels(): { provider: string; id: string; name: string; available: boolean }[];
  setModel(provider: string, id: string): Promise<void>;
}
