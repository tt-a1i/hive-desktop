import type { AgentLaunchConfigInput } from './hive-runtime-contract.js';
import {
  AgentProfileError, defaultAgentProfileId, isAgentDriver, validateAgentProfileRef,
  type AgentDriver, type AgentProfileRef, type AgentProfiles, type ResolvedAgentProfile,
} from './agent-profiles.js';

export type StructuredProfileSelection = { kind: 'explicit'; ref: AgentProfileRef }
  | { kind: 'legacy-default'; id: string };
export interface ParsedStructuredLaunch {
  /** Exact copy of the Hive launch configuration, including generic fields/arguments. */
  config: AgentLaunchConfigInput;
  driver: AgentDriver;
  profile: StructuredProfileSelection;
  model?: { provider: string; id: string };
  /** Uninterpreted Hive/native arguments, in original order. Never discard them at serialization. */
  extraArgs: string[];
}
export interface ResolvedStructuredLaunch extends ParsedStructuredLaunch { resolvedProfile: ResolvedAgentProfile }
export class StructuredLaunchError extends Error {
  constructor(message: string) { super(message); this.name = 'StructuredLaunchError'; }
}
const flags = ['--provider', '--model', '--profile-id', '--profile-revision'] as const;
type Flag = typeof flags[number];
function parse(config: AgentLaunchConfigInput): ParsedStructuredLaunch {
  if (!config || typeof config !== 'object' || Array.isArray(config)
    || typeof config.command !== 'string' || !config.command.startsWith('hive:') || !isAgentDriver(config.command.slice(5)))
    throw new StructuredLaunchError('Expected one of the four structured Hive backend commands');
  const driver = config.command.slice(5) as AgentDriver;
  if (config.args !== undefined && !Array.isArray(config.args)) throw new StructuredLaunchError('Launch arguments must be an array');
  const args = config.args ?? [];
  if (args.some(value => typeof value !== 'string' || /\u0000/u.test(value))) throw new StructuredLaunchError('Launch arguments must be strings without NUL');
  const values = new Map<Flag, string>();
  const extraArgs: string[] = [];
  for (let index = 0; index < args.length; index++) {
    const arg = args[index]!;
    if (arg === '--') { extraArgs.push(...args.slice(index)); break; }
    if ((flags as readonly string[]).includes(arg)) {
      const flag = arg as Flag;
      if (values.has(flag)) throw new StructuredLaunchError(`Duplicate ${flag}`);
      const value = args[index + 1];
      if (value === undefined || !value.trim() || value.startsWith('-') || /[\u0000-\u001f\u007f-\u009f]/u.test(value))
        throw new StructuredLaunchError(`Missing or invalid value for ${flag}`);
      values.set(flag, value);
      index++;
    } else {
      if (arg.startsWith('--profile') || arg.startsWith('--provider=') || arg.startsWith('--model='))
        throw new StructuredLaunchError('Use separate, exact structured flags and values');
      extraArgs.push(arg);
    }
  }
  const provider = values.get('--provider'), modelId = values.get('--model');
  if ((provider === undefined) !== (modelId === undefined)) throw new StructuredLaunchError('Structured model selection requires both --provider and --model');
  const id = values.get('--profile-id'), revisionText = values.get('--profile-revision');
  if ((id === undefined) !== (revisionText === undefined)) throw new StructuredLaunchError('Structured profile selection requires both ID and revision');
  let profile: StructuredProfileSelection = { kind: 'legacy-default', id: defaultAgentProfileId(driver) };
  if (id !== undefined && revisionText !== undefined) {
    if (!/^[1-9][0-9]*$/u.test(revisionText)) throw new StructuredLaunchError('Profile revision must be a positive integer');
    try { profile = { kind: 'explicit', ref: validateAgentProfileRef({ id, revision: Number(revisionText) }) }; }
    catch (error) {
      if (!(error instanceof AgentProfileError)) throw error;
      throw new StructuredLaunchError('Invalid structured profile reference');
    }
  }
  return { config: structuredClone(config), driver, profile, extraArgs,
    ...(provider !== undefined && modelId !== undefined && { model: { provider, id: modelId } }) };
}
/** Only interprets desktop-owned flags. Generic Hive config fields/argv round-trip unchanged. */
export function parseStructuredLaunch(config: AgentLaunchConfigInput): ParsedStructuredLaunch { return parse(config); }
/** Legacy selects migration revision 1, never a newly edited current/default revision. */
export async function resolveStructuredLaunch(parsed: ParsedStructuredLaunch, profiles: AgentProfiles,
  options: { forExecution?: boolean } = {}): Promise<ResolvedStructuredLaunch> {
  // Reparse the original instead of trusting a caller-supplied driver/profile projection.
  const validated = parseStructuredLaunch(parsed.config);
  const ref = validated.profile.kind === 'explicit' ? validated.profile.ref : await profiles.legacyRef(validated.driver);
  const resolvedProfile = options.forExecution === false ? await profiles.getRef(ref, validated.driver)
    : await profiles.resolveForExecution(ref, validated.driver);
  return { ...validated, resolvedProfile };
}
/** Replace only profile flags, retaining model, generic flags, order and all Hive config fields. */
export function withStructuredProfile(config: AgentLaunchConfigInput, ref: AgentProfileRef): AgentLaunchConfigInput {
  validateAgentProfileRef(ref);
  parse(config);
  const args: string[] = [];
  const original = config.args ?? [];
  let separator = original.length;
  for (let index = 0; index < original.length; index++) {
    const arg = original[index]!;
    if (arg === '--') { separator = args.length; args.push(...original.slice(index)); break; }
    if (arg === '--profile-id' || arg === '--profile-revision') { index++; continue; }
    args.push(arg);
  }
  args.splice(Math.min(separator, args.length), 0, '--profile-id', ref.id, '--profile-revision', String(ref.revision));
  const result = { ...structuredClone(config), args };
  parseStructuredLaunch(result);
  return result;
}
