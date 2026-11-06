import type { HiveHost } from './hive-host.js';

export interface DesktopExecutables { piCommand: string | null; codexCommand: string | null; claudeCommand: string | null }
export interface DesktopExecutableEnvironment { readonly HIVE_PI_COMMAND?: string; readonly HIVE_CODEX_COMMAND?: string; readonly HIVE_CLAUDE_COMMAND?: string }
type EffectiveCommand = { command: string; source: 'env' | 'app' | 'path' };
export interface DesktopSettingsSnapshot extends DesktopExecutables {
  effective: { pi: EffectiveCommand; codex: EffectiveCommand; claude: EffectiveCommand };
}
export class DesktopSettingsValidationError extends Error {
  constructor(message: string) { super(message); this.name = 'DesktopSettingsValidationError'; }
}
const path = '/api/settings/app-state/desktop_executables';
const defaults: DesktopExecutables = { piCommand: null, codexCommand: null, claudeCommand: null };
const maxCommandLength = 4096;
function command(value: unknown, field: string): string | null {
  if (value === null) return null;
  if (typeof value !== 'string' || value.length > maxCommandLength || /[\u0000-\u001f\u007f-\u009f]/.test(value) || !value.trim()) {
    throw new DesktopSettingsValidationError(`${field} must be null or a nonempty executable name/path of at most ${maxCommandLength} characters, without control characters`);
  }
  return value.trim();
}
function executables(value: unknown): DesktopExecutables {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new DesktopSettingsValidationError('Expected desktop executable settings');
  const record = value as Record<string, unknown>;
  return { piCommand: command(record.piCommand, 'piCommand'), codexCommand: command(record.codexCommand, 'codexCommand'),
    claudeCommand: record.claudeCommand === undefined ? null : command(record.claudeCommand, 'claudeCommand') };
}

/** App-local settings use Hive's existing atomic SQLite app-state row. No cache or agent-global writes. */
export class DesktopSettings {
  constructor(private readonly hive: Pick<HiveHost, 'request'>,
    private readonly environment: DesktopExecutableEnvironment = process.env) {}
  private snapshot(saved: DesktopExecutables): DesktopSettingsSnapshot {
    const effective = (environmentValue: string | undefined, appValue: string | null, fallback: string): EffectiveCommand => {
      if (environmentValue !== undefined && environmentValue !== '') {
        return { command: command(environmentValue, `HIVE_${fallback.toUpperCase()}_COMMAND`)!, source: 'env' };
      }
      return appValue ? { command: appValue, source: 'app' } : { command: fallback, source: 'path' };
    };
    return { ...saved, effective: {
      pi: effective(this.environment.HIVE_PI_COMMAND, saved.piCommand, 'pi'),
      codex: effective(this.environment.HIVE_CODEX_COMMAND, saved.codexCommand, 'codex'),
      claude: effective(this.environment.HIVE_CLAUDE_COMMAND, saved.claudeCommand, 'claude'),
    } };
  }
  async get(): Promise<DesktopSettingsSnapshot> {
    const stored = await this.hive.request('GET', path) as { value: string | null };
    return this.snapshot(stored.value === null ? { ...defaults } : executables(JSON.parse(stored.value)));
  }
  async update(value: unknown): Promise<DesktopSettingsSnapshot> {
    let saved = executables(value);
    if ((value as Record<string, unknown>).claudeCommand === undefined) saved = { ...saved, claudeCommand: (await this.get()).claudeCommand };
    const result = this.snapshot(saved);
    await this.hive.request('PUT', path, { value: JSON.stringify(saved) });
    return result;
  }
}
