import { z } from 'zod/v4';
export const acpInitialize = z.object({ protocolVersion: z.literal(1), agentCapabilities: z.object({ loadSession: z.boolean().optional(), promptCapabilities: z.object({ image: z.boolean().optional() }).passthrough().optional() }).passthrough().optional() }).passthrough();
export const acpSession = z.object({ sessionId: z.string().min(1) }).passthrough();
export const acpPermission = z.object({ sessionId: z.string(), toolCall: z.object({ toolCallId: z.string() }).passthrough(), options: z.array(z.object({ optionId: z.string(), name: z.string(), kind: z.string() }).passthrough()).min(1) }).passthrough();
export const acpUpdate = z.object({ sessionId: z.string(), update: z.object({ sessionUpdate: z.string() }).passthrough() });
export function object(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }
