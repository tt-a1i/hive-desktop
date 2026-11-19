import { isDeepStrictEqual } from 'node:util';
import { CodexRpcError, type CodexTransport } from './codex-transport.js';
import type { SessionModeState } from './types.js';

export type CodexThreadModeReceipt = {
  approvalPolicy?: unknown;
  sandbox?: { type?: unknown } | null;
  collaborationMode?: { mode?: unknown } | null;
};

type Policy = { approvalPolicy: string; sandbox: string; sandboxPolicy: Record<string, unknown> };
const policies: Record<string, Policy> = {
  'read-only': { approvalPolicy: 'on-request', sandbox: 'read-only', sandboxPolicy: { type: 'readOnly', networkAccess: false } },
  'workspace-write': { approvalPolicy: 'on-request', sandbox: 'workspace-write', sandboxPolicy: { type: 'workspaceWrite', writableRoots: [], networkAccess: false, excludeTmpdirEnvVar: false, excludeSlashTmp: false } },
  'full-access': { approvalPolicy: 'never', sandbox: 'danger-full-access', sandboxPolicy: { type: 'dangerFullAccess' } },
};
const labels: Record<string, string> = { 'read-only': '只读', 'workspace-write': '工作区写入', 'full-access': '完全访问', 'native-plan': '计划', 'native-default': '默认协作' };
const validApproval = (value: unknown) => {
  if (typeof value === 'string') return ['untrusted', 'on-request', 'never'].includes(value);
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const granular = (value as {granular?: unknown}).granular;
  return !!granular && typeof granular === 'object' && !Array.isArray(granular) &&
    ['sandbox_approval', 'rules', 'skill_approval', 'request_permissions', 'mcp_elicitations']
      .every(key => typeof (granular as Record<string, unknown>)[key] === 'boolean');
};
const unsupported = (error: unknown) => error instanceof CodexRpcError && error.code === -32601;

export class CodexSavedModeError extends Error {
  constructor(readonly code: 'CODEX_SAVED_MODE_UNAVAILABLE' | 'CODEX_PERMISSION_ACK_UNCONFIRMED', message: string) {
    super(message);
  }
}

type RecoveryAxis = {saved: string; options: SessionModeState['options']; canInherit: boolean};
export type CodexModeRecoveryChoices = {mode?: RecoveryAxis; permissionMode?: RecoveryAxis};
export type CodexModeReplacement = {mode?: string | null; permissionMode?: string};

/** Permission and collaboration are independent sticky native settings. */
export class CodexModes {
  private collaboration: string | null;
  private permission: string | null;
  private appliedCollaboration: string | null = null;
  private appliedPermission: string | null = null;
  private collaborationOptions: string[] = [];
  private permissionOptions: string[] = [];
  private collaborationDescription?: string;
  private permissionDescription?: string;
  private permissionsLoaded = false;
  private collaborationLoaded = false;
  private loading?: Promise<void>;
  constructor(private transport: CodexTransport, collaboration?: string, permission?: string) {
    this.collaboration = collaboration ?? null; this.permission = permission ?? null;
  }
  async load() {
    if (this.permissionsLoaded && this.collaborationLoaded) return;
    if (this.loading) return this.loading;
    const loading = this.loadOptions();
    this.loading = loading;
    try { await loading; }
    finally { if (this.loading === loading) delete this.loading; }
  }
  private async loadOptions() {
    // Restoring startup permissions requires persisting the exact native baseline.
    // Omitted permission mode leaves existing native defaults intact.
    const permissionOptions: string[] = [];
    if (!this.permissionsLoaded) try {
      const result = await this.transport.sendRequest<{requirements: {allowedApprovalPolicies?: unknown[] | null; allowedSandboxModes?: string[] | null} | null}>('configRequirements/read', {});
      if (!result || !('requirements' in result)) throw new Error('Invalid Codex requirements response');
      const req = result.requirements;
      if (req !== null && (typeof req !== 'object' || Array.isArray(req) ||
          (req.allowedApprovalPolicies != null && (!Array.isArray(req.allowedApprovalPolicies) || req.allowedApprovalPolicies.some(value => !validApproval(value)))) ||
          (req.allowedSandboxModes != null && (!Array.isArray(req.allowedSandboxModes) || req.allowedSandboxModes.some(value => !['read-only', 'workspace-write', 'danger-full-access'].includes(value))))))
        throw new Error('Invalid Codex requirements response');
      for (const [id, policy] of Object.entries(policies)) {
        if ((req?.allowedApprovalPolicies == null || req.allowedApprovalPolicies.includes(policy.approvalPolicy)) &&
            (req?.allowedSandboxModes == null || req.allowedSandboxModes.includes(policy.sandbox))) permissionOptions.push(id);
      }
      this.permissionOptions = permissionOptions;
      delete this.permissionDescription;
      this.permissionsLoaded = true;
    } catch (error) {
      this.permissionsLoaded = unsupported(error);
      this.permissionDescription = unsupported(error) ? 'Codex 未提供权限策略要求。' : '无法确定 Codex 权限策略要求。';
    }
    const collaborationOptions: string[] = [];
    if (!this.collaborationLoaded) try {
      const result = await this.transport.sendRequest<{data: {mode: string | null}[]}>('collaborationMode/list', {});
      if (!Array.isArray(result?.data) || result.data.some(entry =>
        !entry || typeof entry !== 'object' || (entry.mode !== null && typeof entry.mode !== 'string')))
        throw new Error('Invalid Codex collaboration mode response');
      for (const id of ['default', 'plan']) if (result.data.some(entry => entry.mode === id)) collaborationOptions.push(`native-${id}`);
      this.collaborationOptions = collaborationOptions;
      delete this.collaborationDescription;
      this.collaborationLoaded = true;
    } catch (error) {
      this.collaborationLoaded = unsupported(error);
      this.collaborationDescription = unsupported(error) ? '此 Codex 程序不支持原生协作模式。' : '无法确定 Codex 协作模式。';
    }
  }
  private state(selected: string | null, applied: string | null, options: string[], description?: string): SessionModeState {
    return { backend: 'codex', selected, applied, pendingNextTurn: selected !== null && selected !== applied, options: options.map(id => ({id, label: labels[id]!})), ...(description && {description}) };
  }
  mode() { return this.state(this.collaboration, this.appliedCollaboration, this.collaborationOptions, this.collaborationDescription); }
  permissionMode() { return this.state(this.permission, this.appliedPermission, this.permissionOptions, this.permissionDescription); }
  setMode(id: string) { if (!this.collaborationOptions.includes(id)) throw new TypeError(`Unsupported Codex collaboration mode: ${id}`); this.collaboration = id; return this.mode(); }
  setPermissionMode(id: string) {
    if (!this.permissionOptions.includes(id)) throw new TypeError(`Unsupported Codex permission mode: ${id}`);
    this.permission = id; return this.permissionMode();
  }
  async validateInitial() {
    await this.load();
    if (this.collaboration && !this.collaborationOptions.includes(this.collaboration))
      throw new CodexSavedModeError('CODEX_SAVED_MODE_UNAVAILABLE', this.collaborationLoaded
        ? `Unsupported Codex collaboration mode: ${this.collaboration}`
        : 'Could not verify the saved Codex collaboration mode.');
    if (this.permission && !this.permissionOptions.includes(this.permission))
      throw new CodexSavedModeError('CODEX_SAVED_MODE_UNAVAILABLE', this.permissionsLoaded
        ? `Unsupported Codex permission mode: ${this.permission}`
        : 'Could not verify the saved Codex permission mode.');
  }
  async recoveryChoices(): Promise<CodexModeRecoveryChoices> {
    await this.load();
    if ((this.collaboration && !this.collaborationLoaded) || (this.permission && !this.permissionsLoaded))
      throw new Error('Could not verify the saved modes against the current Codex backend. No settings were changed.');
    const choices: CodexModeRecoveryChoices = {};
    if (this.collaboration && !this.collaborationOptions.includes(this.collaboration)) {
      const options = this.mode().options;
      choices.mode = {saved: this.collaboration, options, canInherit: options.length === 0};
    }
    if (this.permission && !this.permissionOptions.includes(this.permission)) {
      const options = this.permissionMode().options;
      choices.permissionMode = {saved: this.permission, options, canInherit: false};
    }
    return choices;
  }
  threadParams() { const p = this.permission && policies[this.permission]; return p ? { approvalPolicy: p.approvalPolicy, sandbox: p.sandbox } : {}; }
  confirmThreadSettings(receipt: CodexThreadModeReceipt) {
    const { approvalPolicy, sandbox, collaborationMode } = receipt;
    const expected = this.permission && policies[this.permission];
    if (expected && (approvalPolicy !== expected.approvalPolicy || sandbox?.type !== expected.sandboxPolicy.type)) {
      throw new CodexSavedModeError('CODEX_PERMISSION_ACK_UNCONFIRMED',
        'Codex did not confirm the requested thread sandbox and approval policy');
    }
    // Thread start/resume selects the native sandbox class. Local configuration
    // can still fill in its network access or writable roots until turn/start.
    this.appliedPermission = Object.entries(policies).find(([, policy]) =>
      approvalPolicy === policy.approvalPolicy && isDeepStrictEqual(sandbox, policy.sandboxPolicy))?.[0] ?? null;
    const mode = collaborationMode?.mode;
    this.appliedCollaboration = mode === 'default' || mode === 'plan' ? `native-${mode}` : null;
  }
  turnParams(model: string, effort: string | null) {
    const p = this.permission && policies[this.permission];
    const policy = p ? { approvalPolicy: p.approvalPolicy, sandboxPolicy: p.sandboxPolicy } : undefined;
    return { ...policy, ...(this.collaboration && { collaborationMode: { mode: this.collaboration.slice(7), settings: { model, reasoning_effort: effort, developer_instructions: null } } }) };
  }
  needsReasoning() { return this.collaboration !== null; }
  accepted() {
    this.appliedCollaboration = this.collaboration ?? this.appliedCollaboration;
    this.appliedPermission = this.permission ?? this.appliedPermission;
  }
}
