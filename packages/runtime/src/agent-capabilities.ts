import { isAgentDriver, type AgentDriver } from './agent-profiles.js';

export const AGENT_CAPABILITIES = ['text', 'images', 'thinking', 'toolEvents', 'interactions',
  'modelSelection', 'reasoning', 'nativeModes', 'steer', 'historyRead', 'nativeResume', 'teamTool'] as const;
export type AgentCapability = typeof AGENT_CAPABILITIES[number];
export type CapabilitySource = 'driver' | 'handshake' | 'session';
export type AgentCapabilityDeclaration = {
  state: 'supported' | 'unsupported'; source: CapabilitySource; reason?: string;
};
export type AgentCapabilityState = AgentCapabilityDeclaration | { state: 'unknown'; source: 'unknown'; reason: string };
export interface AgentCapabilities { driver: AgentDriver; features: Record<AgentCapability, AgentCapabilityState> }

/** Caller supplies actual driver-contract/negotiation facts. No method-existence inference,
 * persisted capability replay, CLI probing, or implicit claim of a completed handshake.
 */
export function agentCapabilities(driver: AgentDriver,
  declarations: Partial<Record<AgentCapability, AgentCapabilityDeclaration>> = {}): AgentCapabilities {
  if (!isAgentDriver(driver)) throw new Error('Unknown agent capability driver');
  for (const key of Object.keys(declarations)) {
    if (!(AGENT_CAPABILITIES as readonly string[]).includes(key)) throw new Error('Unknown agent capability');
  }
  const features = {} as AgentCapabilities['features'];
  for (const capability of AGENT_CAPABILITIES) {
    const declaration = declarations[capability];
    if (!declaration) { features[capability] = { state: 'unknown', source: 'unknown', reason: 'No capability evidence supplied' }; continue; }
    if (!['supported', 'unsupported'].includes(declaration.state) || !['driver', 'handshake', 'session'].includes(declaration.source)
      || declaration.reason !== undefined && typeof declaration.reason !== 'string') throw new Error('Invalid capability declaration');
    features[capability] = { state: declaration.state, source: declaration.source,
      ...(declaration.reason !== undefined && { reason: declaration.reason }) };
  }
  return { driver, features };
}
