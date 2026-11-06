import { randomUUID } from 'node:crypto';
import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod/v4';
import { createTeamTool } from '../team-tool.js';
export interface ClaudeTeamBinding { workspaceId: string; agentId: string; token: string; hivePort: string; cwd: string }
export function createClaudeTeamServer(options: {
  binding?: () => ClaudeTeamBinding | undefined;
  signal: () => AbortSignal | undefined;
  track: (execution: Promise<unknown>) => void;
}) {
  const name = `hive_${randomUUID().replaceAll('-', '')}`;
  const command = createTeamTool(() => {
    const binding = options.binding?.();
    if (!binding) throw new Error('Hive team binding is not active');
    return binding;
  });
  const definition = tool('team', command.description, { args: z.array(z.string()), stdin: z.string().optional() }, async (args, extra) => {
    const activeSignal = options.signal();
    if (!activeSignal) throw new Error('Claude requested a Hive tool outside an active turn');
    const nativeSignal = typeof extra === 'object' && extra !== null && 'signal' in extra && extra.signal instanceof AbortSignal ? extra.signal : undefined;
    const signals = [activeSignal, nativeSignal].filter((value): value is AbortSignal => value !== undefined);
    const execution = command.execute(randomUUID(), { args: args.args, ...(args.stdin !== undefined && { stdin: args.stdin }) }, signals.length ? AbortSignal.any(signals) : undefined, undefined, undefined as never);
    options.track(execution);
    const result = await execution;
    return { content: result.content, ...(result.details !== undefined && { structuredContent: { details: result.details } }) };
  });
  return { name, toolName: `mcp__${name}__team`, definition, server: createSdkMcpServer({ name, version: '1.0.0', tools: [definition] }) };
}
