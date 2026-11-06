import type { SDKUserMessage } from '@anthropic-ai/claude-agent-sdk';

export class ClaudeInput implements AsyncIterable<SDKUserMessage> {
  private values: SDKUserMessage[] = [];
  private waiting: ((value: IteratorResult<SDKUserMessage>) => void) | undefined;
  private ended = false;
  push(value: SDKUserMessage): void {
    if (this.ended) throw new Error('Claude input is closed');
    if (this.waiting) { const resolve = this.waiting; this.waiting = undefined; resolve({ done: false, value }); }
    else this.values.push(value);
  }
  close(): void { this.ended = true; this.values = []; this.waiting?.({ done: true, value: undefined }); this.waiting = undefined; }
  [Symbol.asyncIterator](): AsyncIterator<SDKUserMessage> {
    return { next: () => {
      const value = this.values.shift();
      if (value) return Promise.resolve({ done: false, value });
      if (this.ended) return Promise.resolve({ done: true, value: undefined });
      return new Promise(resolve => { this.waiting = resolve; });
    } };
  }
}
export async function claudeDeadline<T>(operation: Promise<T>, label: string, timeoutMs = 30000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([operation, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error(`Claude ${label} timed out`)), timeoutMs); })]); }
  finally { clearTimeout(timer); }
}
