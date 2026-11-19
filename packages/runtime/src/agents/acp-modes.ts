import { z } from 'zod/v4';
import { object } from './acp-protocol.js';
import type { AcpTransport } from './acp-transport.js';
import type { SessionModeState } from './types.js';

const text = z.string().min(1);
const choice = z.object({ value: text, name: text, description: z.string().nullish() });
const group = z.object({ group: text, name: text, options: z.array(choice) });
const config = z.object({ id: text, name: text, description: z.string().nullish(), type: z.literal('select'),
  currentValue: text, options: z.union([z.array(choice), z.array(group)]) });
const legacy = z.object({ currentModeId: text,
  availableModes: z.array(z.object({ id: text, name: text, description: z.string().nullish() })) });
type Choice = { value: string; name: string; description?: string | undefined | null };
type Selector = { configId: string | null; current: string; choices: Choice[]; description?: string | undefined | null };

function valid(selector: Selector): Selector | undefined {
  const values = selector.choices.map(option => option.value);
  return values.includes(selector.current) && new Set(values).size === values.length ? selector : undefined;
}
function fromConfig(input: unknown): Selector | undefined {
  const parsed = config.safeParse(input);
  if (!parsed.success) return;
  const { id, currentValue, options, description } = parsed.data;
  const choices: Choice[] = [];
  for (const option of options) {
    if ('options' in option) choices.push(...option.options);
    else choices.push(option);
  }
  return valid({ configId: id, current: currentValue, choices, description });
}

export function unavailableAcpMode(description: string): SessionModeState {
  return { backend: 'acp', selected: null, applied: null, pendingNextTurn: false, options: [], description };
}

/** ACP owns these settings. Setup reports native state, and cold load never replays a setter. */
export class AcpModes {
  private standard: Selector | undefined;
  private collaboration: Selector | undefined;
  private legacy: Selector | undefined;

  setup(result: unknown) {
    if (!object(result)) return;
    const old = legacy.safeParse(result.modes);
    if (old.success) this.legacy = valid({ configId: null, current: old.data.currentModeId,
      choices: old.data.availableModes.map(option => ({...option, value: option.id})) });
    if (Array.isArray(result.configOptions)) this.replaceConfig(result.configOptions);
  }
  private replaceConfig(options: unknown[]) {
    this.standard = fromConfig(options.find(value => object(value) && value.type === 'select' && value.category === 'mode'));
    if (this.standard) this.legacy = undefined;
    // Official Codex ACP advertises this separate, vendor-specific axis. Generic
    // agents offering only the standard mode selector keep that single control.
    this.collaboration = fromConfig(options.find(value => object(value) && value.id === 'collaboration_mode' && value.category === 'collaboration_mode'));
  }
  isUpdate(kind: unknown) {
    return kind === 'config_option_update' || kind === 'current_mode_update';
  }
  update(update: Record<string, unknown>): boolean {
    if (update.sessionUpdate === 'config_option_update') {
      if (!Array.isArray(update.configOptions)) return false;
      const before = JSON.stringify([this.standard, this.collaboration]);
      this.replaceConfig(update.configOptions);
      return before !== JSON.stringify([this.standard, this.collaboration]);
    }
    if (update.sessionUpdate === 'current_mode_update' && this.legacy && !this.standard &&
      this.legacy.current !== update.currentModeId && this.legacy.choices.some(option => option.value === update.currentModeId)) {
      this.legacy.current = update.currentModeId as string;
      return true;
    }
    return false;
  }
  private selector(permission: boolean): Selector | undefined {
    const standard = this.standard ?? this.legacy;
    if (permission) return this.collaboration ? standard : undefined;
    return this.collaboration ?? standard;
  }
  private key(selector: Selector, value: string): string {
    return JSON.stringify([selector.configId, value]);
  }
  state(permission = false): SessionModeState {
    const selector = this.selector(permission);
    if (!selector) return unavailableAcpMode(permission ? 'This ACP session does not advertise a separate permission mode.' : 'This ACP session does not advertise a native mode.');
    const current = this.key(selector, selector.current);
    return { backend: 'acp', selected: current, applied: current, pendingNextTurn: false,
      options: selector.choices.map(option => ({id: this.key(selector, option.value), label: option.name,
        ...(option.description && {description: option.description})})), ...(selector.description && {description: selector.description}) };
  }
  async set(transport: AcpTransport, sessionId: string, permission: boolean, id: string) {
    const selector = this.selector(permission);
    const option = selector?.choices.find(choice => this.key(selector, choice.value) === id);
    if (!selector || !option) throw new Error('Choose a mode currently advertised for this ACP session.');
    if (option.value === selector.current) return false;
    const { configId } = selector;
    if (configId === null) {
      await transport.sendRequest('session/set_mode', {sessionId, modeId: option.value}, undefined, result => {
        if (!object(result) || !this.legacy?.choices.some(choice => choice.value === option.value))
          throw new Error('ACP did not acknowledge the selected mode.');
        this.legacy.current = option.value;
      });
    } else {
      await transport.sendRequest('session/set_config_option', {sessionId, configId, value: option.value}, undefined, result => {
        if (!object(result) || !Array.isArray(result.configOptions)) throw new Error('ACP returned no configuration acknowledgement.');
        const acknowledged = fromConfig(result.configOptions.find(value => object(value) && value.id === configId));
        if (acknowledged?.current !== option.value) throw new Error('ACP did not acknowledge the selected mode.');
        this.replaceConfig(result.configOptions);
      });
    }
    return true;
  }
}
