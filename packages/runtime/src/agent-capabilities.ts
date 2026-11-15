import type { AcpSessionHandle } from './agents/acp.js';
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

/** Reflect only this adapter's implemented protocol and current successful session negotiation. */
export function acpCapabilities(handle?: Pick<AcpSessionHandle,'nativeResume'|'teamConnected'>, hasHistory=false): AgentCapabilities {
  const declarations: Partial<Record<AgentCapability,AgentCapabilityDeclaration>>={};
  for(const feature of ['images','modelSelection','reasoning','nativeModes','steer'] as const)
    declarations[feature]={state:'unsupported',source:'driver',reason:`ACP ${feature} is not implemented by this adapter`};
  if(hasHistory)declarations.historyRead={state:'supported',source:'driver',reason:'Local ACP transcript is available; this does not imply native restore'};
  if(handle){
    for(const feature of ['text','thinking','toolEvents','interactions'] as const)declarations[feature]={state:'supported',source:'session'};
    declarations.interactions={state:'supported',source:'session',reason:'ACP session/request_permission with native option IDs only'};
    declarations.nativeResume={state:handle.nativeResume?'supported':'unsupported',source:'handshake',...(!handle.nativeResume&&{reason:'The agent did not advertise loadSession'})};
    declarations.teamTool={state:handle.teamConnected()?'supported':'unsupported',source:'session',...(!handle.teamConnected()&&{reason:'The agent has not connected the Hive MCP team bridge'})};
  }
  return agentCapabilities('acp',declarations);
}
