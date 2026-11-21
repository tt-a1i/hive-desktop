import { accessSync, constants, existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, delimiter, isAbsolute, join } from 'node:path';
import type { AgentProfiles } from './agent-profiles.js';
import type { DesktopSettings } from './desktop-settings.js';

export type ReadinessLevel =
  | 'ready'
  | 'missing_executable'
  | 'invalid_path'
  | 'missing_credentials'
  | 'unconfigured'
  | 'config_error';

export interface McpResourceStatus {
  name: string;
  source: 'pi' | 'codex' | 'claude' | 'pi-rpc' | 'acp' | 'project';
  scope: 'global' | 'project';
  transport: string;
  detail?: string;
  configPath?: string;
  status: 'configured' | 'disabled' | 'error';
  error?: string;
}

export interface BackendReadiness {
  backend: 'pi' | 'codex' | 'claude' | 'pi-rpc' | 'acp';
  level: ReadinessLevel;
  executable?: string;
  credentialState: 'present' | 'missing' | 'unknown' | 'not_applicable';
  configPath?: string;
  configError?: string;
  remediationKey: string;
  mcpCount: number;
}

export interface ReadinessReport {
  checkedAt: number;
  backends: BackendReadiness[];
  mcpResources: McpResourceStatus[];
}

function isExecutableFile(filePath: string): boolean {
  try {
    const st = statSync(filePath);
    if (!st.isFile()) return false;
    if (process.platform === 'win32') return true;
    accessSync(filePath, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function resolveExecutable(command: string, env: NodeJS.ProcessEnv = process.env): { found: boolean; invalidPath: boolean; resolved?: string } {
  const trimmed = command.trim();
  if (!trimmed) return { found: false, invalidPath: false };
  const hasSep = trimmed.includes('/') || trimmed.includes('\\') || isAbsolute(trimmed);
  if (hasSep) {
    if (isExecutableFile(trimmed)) return { found: true, invalidPath: false, resolved: trimmed };
    return { found: false, invalidPath: true };
  }
  const pathEnv = env.PATH ?? env.Path ?? '';
  const exts = process.platform === 'win32'
    ? (env.PATHEXT ?? '.EXE;.CMD;.BAT;.COM').split(';').filter(Boolean)
    : [''];
  for (const dir of pathEnv.split(delimiter).filter(Boolean)) {
    for (const ext of exts) {
      const candidate = join(dir, ext && !trimmed.toLowerCase().endsWith(ext.toLowerCase()) ? `${trimmed}${ext}` : trimmed);
      if (isExecutableFile(candidate)) return { found: true, invalidPath: false, resolved: candidate };
    }
  }
  return { found: false, invalidPath: false };
}

function safeHostOrBasename(value: unknown): string | undefined {
  if (typeof value !== 'string' || !value.trim()) return undefined;
  const trimmed = value.trim();
  if (/^https?:\/\//i.test(trimmed)) {
    try {
      return new URL(trimmed).host;
    } catch {
      return 'url';
    }
  }
  return basename(trimmed);
}

function stripTomlComment(line: string): string {
  let inSingle = false;
  let inDouble = false;
  let escaped = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i]!;
    if (escaped) {
      escaped = false;
      continue;
    }
    if (inDouble && ch === '\\') {
      escaped = true;
      continue;
    }
    if (!inDouble && ch === '\'') {
      inSingle = !inSingle;
      continue;
    }
    if (!inSingle && ch === '"') {
      inDouble = !inDouble;
      continue;
    }
    if (!inSingle && !inDouble && ch === '#') {
      return line.slice(0, i).trim();
    }
  }
  return line.trim();
}

function parseCodexMcpToml(
  filePath: string,
  scope: 'global' | 'project' = 'global',
): { resources: McpResourceStatus[]; error?: string } {
  if (!existsSync(filePath)) return { resources: [] };
  let raw: string;
  try {
    raw = readFileSync(filePath, 'utf8');
  } catch (error) {
    return { resources: [], error: error instanceof Error ? error.message : String(error) };
  }
  const resources: McpResourceStatus[] = [];
  let current: { name: string; command?: string; url?: string; enabled?: boolean } | undefined;
  const flush = () => {
    if (!current) return;
    const transport = current.url ? 'http' : 'stdio';
    const detail = safeHostOrBasename(current.url ?? current.command);
    resources.push({
      name: current.name,
      source: 'codex',
      scope,
      transport,
      ...(detail && { detail }),
      configPath: filePath,
      status: current.enabled === false ? 'disabled' : 'configured',
    });
    current = undefined;
  };
  for (const rawLine of raw.split(/\r?\n/)) {
    const line = stripTomlComment(rawLine);
    if (!line) continue;
    const header = line.match(/^\[([^\]]+)\]$/);
    if (header) {
      flush();
      const section = header[1]!.trim();
      // Only match top-level [mcp_servers.<name>] tables; nested subtables like
      // [mcp_servers.<name>.env] or [mcp_servers.<name>.headers] belong to the server
      // config rather than declaring separate MCP servers.
      const mcpMatch = section.match(/^mcp_servers\.(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))$/);
      if (mcpMatch) {
        current = { name: mcpMatch[1] ?? mcpMatch[2] ?? mcpMatch[3]! };
      }
      continue;
    }
    if (!current) continue;
    const kv = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
    if (!kv) continue;
    const key = kv[1]!;
    const val = kv[2]!.trim();
    if (key === 'command') {
      const m = val.match(/^["'](.*)["']$/);
      if (m) current.command = m[1]!;
    } else if (key === 'url') {
      const m = val.match(/^["'](.*)["']$/);
      if (m) current.url = m[1]!;
    } else if (key === 'enabled') {
      if (val === 'false') current.enabled = false;
      else if (val === 'true') current.enabled = true;
    }
  }
  flush();
  return { resources };
}

function parseJsonMcpServers(
  filePath: string,
  source: 'claude' | 'pi' | 'project',
  scope: 'global' | 'project' = 'global',
): { resources: McpResourceStatus[]; error?: string } {
  if (!existsSync(filePath)) return { resources: [] };
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(filePath, 'utf8'));
  } catch (error) {
    return {
      resources: [{
        name: basename(filePath),
        source,
        scope,
        transport: 'config',
        configPath: filePath,
        status: 'error',
        error: error instanceof Error ? error.message : String(error),
      }],
      error: error instanceof Error ? error.message : String(error),
    };
  }
  if (!parsed || typeof parsed !== 'object') return { resources: [] };
  const servers = (parsed as Record<string, unknown>).mcpServers;
  if (!servers || typeof servers !== 'object') return { resources: [] };
  const resources: McpResourceStatus[] = [];
  for (const [name, entry] of Object.entries(servers as Record<string, unknown>)) {
    if (!entry || typeof entry !== 'object') continue;
    const obj = entry as Record<string, unknown>;
    const disabled = obj.disabled === true || obj.enabled === false;
    const transport = typeof obj.type === 'string' && obj.type.trim()
      ? obj.type.trim()
      : typeof obj.url === 'string'
        ? 'http'
        : 'stdio';
    const detail = safeHostOrBasename(obj.url ?? obj.command);
    resources.push({
      name,
      source,
      scope,
      transport,
      ...(detail && { detail }),
      configPath: filePath,
      status: disabled ? 'disabled' : 'configured',
    });
  }
  return { resources };
}

function inspectPiExtensions(agentDir: string): McpResourceStatus[] {
  const extDir = join(agentDir, 'extensions');
  if (!existsSync(extDir)) return [];
  try {
    return readdirSync(extDir, { withFileTypes: true })
      .filter(entry => !entry.name.startsWith('.'))
      .slice(0, 32)
      .map(entry => ({
        name: entry.name,
        source: 'pi' as const,
        scope: 'global' as const,
        transport: 'extension',
        detail: entry.isDirectory() ? 'dir' : 'file',
        configPath: join(extDir, entry.name),
        status: 'configured' as const,
      }));
  } catch {
    return [];
  }
}

export async function collectReadinessReport(options: {
  settings: DesktopSettings;
  profiles: AgentProfiles;
  workspacePath?: string;
  env?: NodeJS.ProcessEnv;
  homeDir?: string;
}): Promise<ReadinessReport> {
  const env = options.env ?? process.env;
  const home = options.homeDir ?? homedir();
  const desktopSettings = await options.settings.get();
  const mcpResources: McpResourceStatus[] = [];

  // 1. Built-in Pi (read-only auth.json & extensions check)
  const piAgentDir = env.PI_CODING_AGENT_DIR?.trim() || join(home, '.pi', 'agent');
  const piAuthPath = join(piAgentDir, 'auth.json');
  let piCredState: BackendReadiness['credentialState'] = 'missing';
  let piConfigError: string | undefined;
  if (existsSync(piAuthPath)) {
    try {
      const auth = JSON.parse(readFileSync(piAuthPath, 'utf8')) as Record<string, unknown>;
      piCredState = auth && typeof auth === 'object' && Object.keys(auth).length > 0 ? 'present' : 'missing';
    } catch (error) {
      piConfigError = error instanceof Error ? error.message : String(error);
      piCredState = 'unknown';
    }
  }
  const piMcp = parseJsonMcpServers(join(piAgentDir, 'settings.json'), 'pi');
  const piExts = inspectPiExtensions(piAgentDir);
  mcpResources.push(...piMcp.resources, ...piExts);
  if (piMcp.error && !piConfigError) piConfigError = piMcp.error;

  const piBackend: BackendReadiness = {
    backend: 'pi',
    level: piConfigError ? 'config_error' : piCredState === 'present' ? 'ready' : 'missing_credentials',
    credentialState: piCredState,
    configPath: piAuthPath,
    ...(piConfigError && { configError: piConfigError }),
    remediationKey: piConfigError
      ? '修复内置 Pi 配置文件格式'
      : piCredState === 'present'
        ? '已就绪'
        : '前往“内置 Pi”添加订阅登录或 API 密钥',
    mcpCount: piMcp.resources.length + piExts.length,
  };

  // 2. Codex (read-only executable + ~/.codex/config.toml & auth.json check)
  const codexCmd = desktopSettings.effective.codex.command;
  const codexExec = resolveExecutable(codexCmd, env);
  const codexHome = env.CODEX_HOME?.trim() || join(home, '.codex');
  const codexConfigPath = join(codexHome, 'config.toml');
  const codexMcp = parseCodexMcpToml(codexConfigPath);
  mcpResources.push(...codexMcp.resources);
  const codexHasAuth = existsSync(join(codexHome, 'auth.json')) || Boolean(env.OPENAI_API_KEY?.trim());
  const codexBackend: BackendReadiness = {
    backend: 'codex',
    level: codexExec.invalidPath
      ? 'invalid_path'
      : !codexExec.found
        ? 'missing_executable'
        : codexMcp.error
          ? 'config_error'
          : codexHasAuth
            ? 'ready'
            : 'missing_credentials',
    executable: codexExec.resolved ?? codexCmd,
    credentialState: codexHasAuth ? 'present' : 'unknown',
    configPath: codexConfigPath,
    ...(codexMcp.error && { configError: codexMcp.error }),
    remediationKey: codexExec.invalidPath
      ? '检查 Codex 可执行文件路径是否有效'
      : !codexExec.found
        ? '安装 Codex CLI 或在“外部程序”中指定路径'
        : codexMcp.error
          ? '检查 Codex 配置文件权限或格式'
          : codexHasAuth
            ? '已就绪'
            : '在终端登录 Codex 或核对本机凭据',
    mcpCount: codexMcp.resources.length,
  };

  // 3. Claude Code (read-only executable + ~/.claude.json MCP config; never logs in!)
  const claudeCmd = desktopSettings.claudeCommand?.trim() || 'claude';
  const claudeExec = resolveExecutable(claudeCmd, env);
  const claudeConfigPath = join(home, '.claude.json');
  const claudeMcp = parseJsonMcpServers(claudeConfigPath, 'claude');
  mcpResources.push(...claudeMcp.resources);
  const claudeBackend: BackendReadiness = {
    backend: 'claude',
    level: claudeExec.invalidPath
      ? 'invalid_path'
      : !claudeExec.found
        ? 'missing_executable'
        : claudeMcp.error
          ? 'config_error'
          : 'ready',
    executable: claudeExec.resolved ?? claudeCmd,
    credentialState: 'unknown',
    configPath: claudeConfigPath,
    ...(claudeMcp.error && { configError: claudeMcp.error }),
    remediationKey: claudeExec.invalidPath
      ? '检查 Claude Code 可执行文件路径是否有效'
      : !claudeExec.found
        ? '安装 Claude Code CLI 或在“外部程序”中指定路径'
        : claudeMcp.error
          ? '检查 ~/.claude.json 格式是否有效'
          : '程序已检测到（不自动触发账号登录检查）',
    mcpCount: claudeMcp.resources.length,
  };

  // 4. External Pi (read-only executable check)
  const extPiCmd = desktopSettings.piCommand?.trim() || 'pi';
  const extPiExec = resolveExecutable(extPiCmd, env);
  const extPiBackend: BackendReadiness = {
    backend: 'pi-rpc',
    level: extPiExec.invalidPath ? 'invalid_path' : extPiExec.found ? 'ready' : 'missing_executable',
    executable: extPiExec.resolved ?? extPiCmd,
    credentialState: piCredState,
    configPath: piAgentDir,
    remediationKey: extPiExec.invalidPath
      ? '检查外部 Pi 可执行文件路径是否有效'
      : extPiExec.found
        ? '已就绪'
        : '安装外部 Pi CLI 或在“外部程序”中指定路径',
    mcpCount: 0,
  };

  // 5. ACP profiles (read-only profile launch command verification)
  const profileSnapshot = await options.profiles.list();
  const acpProfiles = profileSnapshot.profiles.filter(p => p.driver === 'acp');
  let acpLevel: ReadinessLevel = 'unconfigured';
  let acpExecSample: string | undefined;
  let acpError: string | undefined;
  if (acpProfiles.length > 0) {
    let anyReady = false;
    let anyInvalid = false;
    let anyMissing = false;
    for (const profile of acpProfiles) {
      const currentRev = profile.revisions.find(r => r.revision === profile.currentRevision) ?? profile.revisions.at(-1);
      const cmd = currentRev?.launch?.command?.trim() ?? '';
      if (!acpExecSample && cmd) acpExecSample = cmd;
      const res = resolveExecutable(cmd, env);
      if (res.found) anyReady = true;
      else if (res.invalidPath) anyInvalid = true;
      else anyMissing = true;
    }
    if (anyReady && !anyInvalid && !anyMissing) acpLevel = 'ready';
    else if (anyInvalid) {
      acpLevel = 'invalid_path';
      acpError = '存在路径无效的 ACP 配置';
    } else if (anyMissing) {
      acpLevel = 'missing_executable';
      acpError = '部分 ACP 配置的命令未找到';
    } else {
      acpLevel = 'ready';
    }
  }
  const acpBackend: BackendReadiness = {
    backend: 'acp',
    level: acpLevel,
    ...(acpExecSample && { executable: acpExecSample }),
    credentialState: 'not_applicable',
    ...(acpError && { configError: acpError }),
    remediationKey: acpLevel === 'unconfigured'
      ? '可在“外部程序”中添加 ACP Agent 配置'
      : acpLevel === 'ready'
        ? '已就绪'
        : '检查 ACP 配置的命令路径是否可执行',
    mcpCount: 0,
  };

  // 6. Project-level MCP configurations (when a workspace directory is active)
  if (options.workspacePath?.trim()) {
    const ws = options.workspacePath.trim();
    const projectMcp = parseJsonMcpServers(join(ws, '.mcp.json'), 'project', 'project');
    const projectClaude = parseJsonMcpServers(join(ws, '.claude', 'settings.json'), 'claude', 'project');
    const projectClaudeLocal = parseJsonMcpServers(join(ws, '.claude', 'settings.local.json'), 'claude', 'project');
    const projectCodex = parseCodexMcpToml(join(ws, '.codex', 'config.toml'), 'project');
    mcpResources.push(
      ...projectMcp.resources,
      ...projectClaude.resources,
      ...projectClaudeLocal.resources,
      ...projectCodex.resources,
    );
    codexBackend.mcpCount += projectCodex.resources.length;
    claudeBackend.mcpCount += projectClaude.resources.length + projectClaudeLocal.resources.length;
  }

  return {
    checkedAt: Date.now(),
    backends: [piBackend, codexBackend, claudeBackend, extPiBackend, acpBackend],
    mcpResources,
  };
}
