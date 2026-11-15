import { createAcpSession, type AcpSessionOptions } from './agents/acp.js';
import type { AgentDriver, ResolvedAgentProfile } from './agent-profiles.js';
import { createPiSession } from './agents/pi.js';
import { createCodexSession, type CodexSessionOptions } from './agents/codex.js';
import { createClaudeSession, type ClaudeSessionOptions } from './agents/claude.js';
import { createExternalPiSession, type ExternalPiSessionOptions } from './agents/external-pi.js';
import type { PiSessionHandle, PiSessionOptions } from './agents/types.js';

/** Native child configuration only. Never mutate the desktop host environment. */
export function nativeProfileLaunch(profile: ResolvedAgentProfile): { command?: string; args: string[]; env: NodeJS.ProcessEnv } {
  const { command, args, configDir } = profile.launch;
  if (profile.driver === 'acp' && configDir) throw new TypeError('ACP has no universal configuration directory; use explicit nonsecret native arguments');
  if (profile.driver === 'pi' && (command || args.length)) throw new TypeError('Built-in Pi does not accept a command or CLI arguments');
  if (profile.driver === 'claude' && args.length) throw new TypeError('Claude profile arguments are unsupported by the SDK adapter');
  const configVariable = profile.driver === 'codex' ? 'CODEX_HOME' : profile.driver === 'claude' ? 'CLAUDE_CONFIG_DIR' : 'PI_CODING_AGENT_DIR';
  return { ...(command && { command }), args: [...args], env: { ...process.env, ...(configDir && { [configVariable]: configDir }) } };
}

export interface AgentSessionFactoryOptions {
  backend: AgentDriver;
  profile?: ResolvedAgentProfile;
  pi?: PiSessionOptions;
  codex?: CodexSessionOptions;
  externalPi?: ExternalPiSessionOptions;
  claude?: ClaudeSessionOptions;
  acp?: AcpSessionOptions;
}

/** Keep backend lifecycle, native modes, tools, context and restore contracts intact. */
export async function createAgentSession(options: AgentSessionFactoryOptions): Promise<PiSessionHandle> {
  const profile = options.profile;
  if (profile && profile.driver !== options.backend) throw new TypeError('Agent profile driver does not match session backend');
  if (profile && !profile.enabled) throw new Error('Agent profile is disabled');
  const native = profile && nativeProfileLaunch(profile);
  const launch = profile && { ...(native!.command && { command: native!.command }), args: native!.args,
    ...(profile.launch.configDir && { configDir: profile.launch.configDir }) };
  switch (options.backend) {
    case 'acp':
      if (!options.acp || !native?.command) throw new TypeError('ACP requires explicitly configured session/profile options');
      return createAcpSession({...options.acp, command: native.command, args: native.args, env: native.env});
    case 'pi': {
      if (!options.pi) throw new TypeError('Missing built-in Pi session options');
      return createPiSession({ ...options.pi, ...(profile?.launch.configDir && { agentDir: profile.launch.configDir }) });
    }
    case 'codex':
      if (!options.codex) throw new TypeError('Missing Codex session options');
      return createCodexSession({ ...options.codex, ...launch });
    case 'pi-rpc':
      if (!options.externalPi) throw new TypeError('Missing external Pi session options');
      return createExternalPiSession({ ...options.externalPi, ...launch });
    case 'claude':
      if (!options.claude) throw new TypeError('Missing Claude session options');
      return createClaudeSession({ ...options.claude, ...launch });
  }
}
