import { execFile, spawn } from 'node:child_process';
import { existsSync, readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { randomBytes } from 'node:crypto';

export interface McpToolInfo {
  name: string;
  description?: string;
}

export interface McpProbeResult {
  name: string;
  ok: boolean;
  status: 'ok' | 'error' | 'timeout' | 'unsupported';
  latencyMs: number;
  serverName?: string;
  serverVersion?: string;
  protocolVersion?: string;
  tools: McpToolInfo[];
  error?: string;
}

export interface McpServerTarget {
  name: string;
  transport: 'stdio' | 'http' | 'sse' | 'unknown';
  command?: string;
  args?: string[];
  url?: string;
  env?: Record<string, string>;
  headers?: Record<string, string>;
  cwd?: string;
}

export interface ProjectMcpServerInput {
  name: string;
  transport: 'stdio' | 'http';
  command?: string;
  args?: string[];
  url?: string;
  env?: Record<string, string>;
  disabled?: boolean;
}

const VALID_SERVER_NAME = /^[A-Za-z0-9._-]{1,64}$/;
const DEFAULT_PROBE_TIMEOUT_MS = 5000;
const MAX_PROBE_TIMEOUT_MS = 15000;
const MAX_TOOLS_RETURNED = 100;

function assertValidServerName(name: string): string {
  const trimmed = name.trim();
  if (!VALID_SERVER_NAME.test(trimmed)) {
    throw new Error('MCP 服务名称仅支持 1–64 位字母、数字、点、横线或下划线');
  }
  return trimmed;
}

function readJsonObject(filePath: string): Record<string, unknown> {
  if (!existsSync(filePath)) return {};
  const parsed = JSON.parse(readFileSync(filePath, 'utf8')) as unknown;
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('MCP 配置文件根节点必须是 JSON 对象');
  }
  return parsed as Record<string, unknown>;
}

function atomicWriteJson(filePath: string, payload: Record<string, unknown>): void {
  const tmp = `${filePath}.${randomBytes(6).toString('hex')}.tmp`;
  try {
    writeFileSync(tmp, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
    renameSync(tmp, filePath);
  } catch (err) {
    try { unlinkSync(tmp); } catch {}
    throw err;
  }
}

export function projectMcpConfigPath(workspacePath: string): string {
  return join(workspacePath, '.mcp.json');
}

export function upsertProjectMcpServer(
  workspacePath: string,
  input: ProjectMcpServerInput,
): { ok: true; configPath: string; name: string } {
  const name = assertValidServerName(input.name);
  const configPath = projectMcpConfigPath(workspacePath);
  const root = readJsonObject(configPath);
  const existing = root.mcpServers && typeof root.mcpServers === 'object' && !Array.isArray(root.mcpServers)
    ? { ...(root.mcpServers as Record<string, unknown>) }
    : {};

  const envEntries = input.env && typeof input.env === 'object'
    ? Object.entries(input.env)
        .map(([k, v]) => [k.trim(), String(v)] as const)
        .filter(([k]) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(k))
    : [];
  const envObj = envEntries.length > 0 ? Object.fromEntries(envEntries) : undefined;

  if (input.transport === 'stdio') {
    const command = (input.command ?? '').trim();
    if (!command) throw new Error('stdio 类型 MCP 必须填写启动命令');
    const args = Array.isArray(input.args)
      ? input.args.map(a => String(a)).filter(a => a.length > 0)
      : [];
    existing[name] = {
      command,
      ...(args.length > 0 && { args }),
      ...(envObj && { env: envObj }),
      ...(input.disabled ? { disabled: true } : {}),
    };
  } else if (input.transport === 'http') {
    const url = (input.url ?? '').trim();
    if (!/^https?:\/\//i.test(url)) {
      throw new Error('http 类型 MCP 必须填写以 http:// 或 https:// 开头的地址');
    }
    existing[name] = {
      type: 'http',
      url,
      ...(envObj && { env: envObj }),
      ...(input.disabled ? { disabled: true } : {}),
    };
  } else {
    throw new Error(`不支持的 MCP 传输类型: ${String(input.transport)}`);
  }

  atomicWriteJson(configPath, { ...root, mcpServers: existing });
  return { ok: true, configPath, name };
}

export function toggleProjectMcpServer(
  workspacePath: string,
  name: string,
  disabled: boolean,
): { ok: true; configPath: string; name: string; disabled: boolean } {
  const validName = assertValidServerName(name);
  const configPath = projectMcpConfigPath(workspacePath);
  if (!existsSync(configPath)) {
    throw new Error('当前项目不存在 .mcp.json 配置文件');
  }
  const root = readJsonObject(configPath);
  const servers = root.mcpServers && typeof root.mcpServers === 'object' && !Array.isArray(root.mcpServers)
    ? { ...(root.mcpServers as Record<string, unknown>) }
    : {};
  const entry = servers[validName];
  if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
    throw new Error(`在 .mcp.json 中找不到 MCP 服务 "${validName}"`);
  }
  const next = { ...(entry as Record<string, unknown>) };
  if (disabled) {
    next.disabled = true;
  } else {
    delete next.disabled;
  }
  servers[validName] = next;
  atomicWriteJson(configPath, { ...root, mcpServers: servers });
  return { ok: true, configPath, name: validName, disabled };
}

export function removeProjectMcpServer(
  workspacePath: string,
  name: string,
): { ok: true; configPath: string; name: string } {
  const validName = assertValidServerName(name);
  const configPath = projectMcpConfigPath(workspacePath);
  if (!existsSync(configPath)) {
    throw new Error('当前项目不存在 .mcp.json 配置文件');
  }
  const root = readJsonObject(configPath);
  const servers = root.mcpServers && typeof root.mcpServers === 'object' && !Array.isArray(root.mcpServers)
    ? { ...(root.mcpServers as Record<string, unknown>) }
    : {};
  if (!(validName in servers)) {
    throw new Error(`在 .mcp.json 中找不到 MCP 服务 "${validName}"`);
  }
  delete servers[validName];
  atomicWriteJson(configPath, { ...root, mcpServers: servers });
  return { ok: true, configPath, name: validName };
}

function extractTargetFromJsonConfig(filePath: string, name: string, cwd?: string): McpServerTarget | undefined {
  if (!existsSync(filePath)) return undefined;
  try {
    const parsed = JSON.parse(readFileSync(filePath, 'utf8')) as Record<string, unknown>;
    const servers = parsed?.mcpServers;
    if (!servers || typeof servers !== 'object' || Array.isArray(servers)) return undefined;
    const entry = (servers as Record<string, unknown>)[name];
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return undefined;
    const cfg = entry as Record<string, unknown>;
    const url = typeof cfg.url === 'string' && cfg.url.trim() ? cfg.url.trim() : undefined;
    const command = typeof cfg.command === 'string' && cfg.command.trim() ? cfg.command.trim() : undefined;
    const args = Array.isArray(cfg.args) ? cfg.args.map(String) : undefined;
    const env = cfg.env && typeof cfg.env === 'object' && !Array.isArray(cfg.env)
      ? Object.fromEntries(Object.entries(cfg.env as Record<string, unknown>).map(([k, v]) => [k, String(v)]))
      : undefined;
    const headers = cfg.headers && typeof cfg.headers === 'object' && !Array.isArray(cfg.headers)
      ? Object.fromEntries(Object.entries(cfg.headers as Record<string, unknown>).map(([k, v]) => [k, String(v)]))
      : undefined;
    const transport: McpServerTarget['transport'] = url
      ? (cfg.type === 'sse' ? 'sse' : 'http')
      : command
        ? 'stdio'
        : 'unknown';
    return {
      name,
      transport,
      ...(command && { command }),
      ...(args && { args }),
      ...(url && { url }),
      ...(env && { env }),
      ...(headers && { headers }),
      ...(cwd && { cwd }),
    };
  } catch {
    return undefined;
  }
}

function extractTargetFromCodexToml(filePath: string, name: string, cwd?: string): McpServerTarget | undefined {
  if (!existsSync(filePath)) return undefined;
  try {
    const text = readFileSync(filePath, 'utf8');
    let inTarget = false;
    let command: string | undefined;
    let url: string | undefined;
    let args: string[] | undefined;
    for (const rawLine of text.split(/\r?\n/)) {
      const line = rawLine.trim();
      if (!line || line.startsWith('#')) continue;
      const section = line.match(/^\[mcp_servers\.([^\]]+)\]$/);
      if (section) {
        const secName = section[1]!.trim().replace(/^["']|["']$/g, '');
        inTarget = secName === name;
        continue;
      }
      if (line.startsWith('[')) {
        inTarget = false;
        continue;
      }
      if (!inTarget) continue;
      const eq = line.indexOf('=');
      if (eq < 0) continue;
      const key = line.slice(0, eq).trim();
      const val = line.slice(eq + 1).trim();
      if (key === 'command') {
        command = val.replace(/^["']|["']$/g, '');
      } else if (key === 'url') {
        url = val.replace(/^["']|["']$/g, '');
      } else if (key === 'args' && val.startsWith('[')) {
        try {
          const parsed = JSON.parse(val.replace(/'/g, '"')) as unknown;
          if (Array.isArray(parsed)) args = parsed.map(String);
        } catch {}
      }
    }
    if (!command && !url) return undefined;
    return {
      name,
      transport: url ? 'http' : 'stdio',
      ...(command && { command }),
      ...(args && { args }),
      ...(url && { url }),
      ...(cwd && { cwd }),
    };
  } catch {
    return undefined;
  }
}

export function resolveMcpServerTarget(options: {
  name: string;
  scope?: 'global' | 'project';
  configPath?: string;
  workspacePath?: string;
  homeDir?: string;
}): McpServerTarget | undefined {
  const home = options.homeDir ?? homedir();
  const cwd = options.workspacePath;

  if (options.configPath) {
    if (options.configPath.endsWith('.toml')) {
      return extractTargetFromCodexToml(options.configPath, options.name, cwd);
    }
    return extractTargetFromJsonConfig(options.configPath, options.name, cwd);
  }

  if (options.workspacePath && options.scope !== 'global') {
    for (const rel of ['.mcp.json', join('.claude', 'settings.json'), join('.claude', 'settings.local.json')]) {
      const found = extractTargetFromJsonConfig(join(options.workspacePath, rel), options.name, cwd);
      if (found) return found;
    }
    const toml = extractTargetFromCodexToml(join(options.workspacePath, '.codex', 'config.toml'), options.name, cwd);
    if (toml) return toml;
  }

  for (const globalJson of [join(home, '.claude.json'), join(home, '.claude', 'settings.json')]) {
    const found = extractTargetFromJsonConfig(globalJson, options.name, cwd);
    if (found) return found;
  }
  return extractTargetFromCodexToml(join(home, '.codex', 'config.toml'), options.name, cwd);
}

function parseToolsList(raw: unknown): McpToolInfo[] {
  if (!raw || typeof raw !== 'object') return [];
  const tools = (raw as { tools?: unknown }).tools;
  if (!Array.isArray(tools)) return [];
  const out: McpToolInfo[] = [];
  for (const item of tools.slice(0, MAX_TOOLS_RETURNED)) {
    if (!item || typeof item !== 'object') continue;
    const rec = item as Record<string, unknown>;
    if (typeof rec.name !== 'string' || !rec.name.trim()) continue;
    const desc = typeof rec.description === 'string' && rec.description.trim()
      ? rec.description.trim().slice(0, 200)
      : undefined;
    out.push({ name: rec.name.trim(), ...(desc && { description: desc }) });
  }
  return out;
}

function parseSseOrJsonRpc(contentType: string, bodyText: string): Record<string, unknown> | undefined {
  const trimmed = bodyText.trim();
  if (!trimmed) return undefined;
  if (contentType.includes('text/event-stream') || trimmed.startsWith('event:') || trimmed.startsWith('data:')) {
    for (const line of trimmed.split(/\r?\n/)) {
      if (line.startsWith('data:')) {
        const data = line.slice(5).trim();
        if (!data || data === '[DONE]') continue;
        try {
          const parsed = JSON.parse(data) as Record<string, unknown>;
          if (parsed && typeof parsed === 'object' && ('result' in parsed || 'error' in parsed)) {
            return parsed;
          }
        } catch {}
      }
    }
    return undefined;
  }
  try {
    return JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return undefined;
  }
}

async function probeHttpMcp(target: McpServerTarget, timeoutMs: number, startedAt: number): Promise<McpProbeResult> {
  if (!target.url) {
    return { name: target.name, ok: false, status: 'error', latencyMs: Date.now() - startedAt, tools: [], error: '缺少 MCP 服务 URL' };
  }
  try {
    const initHeaders: Record<string, string> = {
      'content-type': 'application/json',
      accept: 'application/json, text/event-stream',
      ...(target.headers ?? {}),
    };
    const initResp = await fetch(target.url, {
      method: 'POST',
      headers: initHeaders,
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          clientInfo: { name: 'hive-desktop', version: '0.1.0' },
          capabilities: {},
        },
      }),
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!initResp.ok) {
      return {
        name: target.name,
        ok: false,
        status: 'error',
        latencyMs: Date.now() - startedAt,
        tools: [],
        error: `HTTP ${initResp.status} ${initResp.statusText}`.trim(),
      };
    }
    const sessionId = initResp.headers.get('mcp-session-id');
    const initBody = parseSseOrJsonRpc(initResp.headers.get('content-type') ?? '', await initResp.text());
    if (!initBody || initBody.error) {
      const errMsg = initBody?.error && typeof initBody.error === 'object'
        ? String((initBody.error as { message?: unknown }).message ?? 'initialize 失败')
        : '无效的 MCP initialize 响应';
      return { name: target.name, ok: false, status: 'error', latencyMs: Date.now() - startedAt, tools: [], error: errMsg };
    }
    const initResult = (initBody.result ?? {}) as Record<string, unknown>;
    const serverInfo = (initResult.serverInfo ?? {}) as Record<string, unknown>;
    const serverName = typeof serverInfo.name === 'string' ? serverInfo.name : undefined;
    const serverVersion = typeof serverInfo.version === 'string' ? serverInfo.version : undefined;
    const protocolVersion = typeof initResult.protocolVersion === 'string' ? initResult.protocolVersion : undefined;

    const listHeaders: Record<string, string> = {
      ...initHeaders,
      ...(sessionId && { 'mcp-session-id': sessionId }),
    };
    let tools: McpToolInfo[] = [];
    try {
      const listResp = await fetch(target.url, {
        method: 'POST',
        headers: listHeaders,
        body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }),
        signal: AbortSignal.timeout(Math.max(1000, timeoutMs - (Date.now() - startedAt))),
      });
      if (listResp.ok) {
        const listBody = parseSseOrJsonRpc(listResp.headers.get('content-type') ?? '', await listResp.text());
        tools = parseToolsList(listBody?.result);
      }
    } catch {
      // initialize already succeeded; tools/list failure is non-fatal
    }

    return {
      name: target.name,
      ok: true,
      status: 'ok',
      latencyMs: Date.now() - startedAt,
      ...(serverName && { serverName }),
      ...(serverVersion && { serverVersion }),
      ...(protocolVersion && { protocolVersion }),
      tools,
    };
  } catch (err) {
    const isTimeout = err instanceof Error && (err.name === 'TimeoutError' || err.name === 'AbortError');
    return {
      name: target.name,
      ok: false,
      status: isTimeout ? 'timeout' : 'error',
      latencyMs: Date.now() - startedAt,
      tools: [],
      error: isTimeout ? `握手超时（>${timeoutMs}ms）` : (err instanceof Error ? err.message : String(err)),
    };
  }
}

async function probeStdioMcp(target: McpServerTarget, timeoutMs: number, startedAt: number): Promise<McpProbeResult> {
  if (!target.command) {
    return { name: target.name, ok: false, status: 'error', latencyMs: Date.now() - startedAt, tools: [], error: '缺少 MCP 启动命令' };
  }
  return new Promise<McpProbeResult>(resolve => {
    let settled = false;
    let stdoutBuf = '';
    let stderrBuf = '';
    let initDone = false;
    let serverName: string | undefined;
    let serverVersion: string | undefined;
    let protocolVersion: string | undefined;

    const child = spawn(target.command!, target.args ?? [], {
      cwd: target.cwd,
      env: { ...process.env, ...(target.env ?? {}) },
      detached: process.platform !== 'win32',
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    const killTree = () => {
      if (!child.pid) return;
      if (process.platform === 'win32') {
        execFile('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true }, () => {});
      } else {
        try { process.kill(-child.pid, 'SIGKILL'); } catch {
          try { child.kill('SIGKILL'); } catch {}
        }
      }
    };

    const finish = (result: McpProbeResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      killTree();
      resolve(result);
    };

    const timer = setTimeout(() => {
      finish({
        name: target.name,
        ok: false,
        status: 'timeout',
        latencyMs: Date.now() - startedAt,
        tools: [],
        error: `MCP 握手超时（>${timeoutMs}ms）`,
      });
    }, timeoutMs);
    timer.unref();

    child.once('error', err => {
      finish({
        name: target.name,
        ok: false,
        status: 'error',
        latencyMs: Date.now() - startedAt,
        tools: [],
        error: err.message,
      });
    });

    child.stderr.on('data', (chunk: Buffer) => {
      if (stderrBuf.length < 1024) {
        stderrBuf += chunk.toString('utf8');
      }
    });

    child.stdout.on('data', (chunk: Buffer) => {
      stdoutBuf += chunk.toString('utf8');
      let nl: number;
      while ((nl = stdoutBuf.indexOf('\n')) >= 0) {
        const line = stdoutBuf.slice(0, nl).trim();
        stdoutBuf = stdoutBuf.slice(nl + 1);
        if (!line) continue;
        let msg: Record<string, unknown>;
        try {
          msg = JSON.parse(line) as Record<string, unknown>;
        } catch {
          continue;
        }
        if (msg.id === 1 && !initDone) {
          if (msg.error) {
            const errMsg = typeof msg.error === 'object' && msg.error
              ? String((msg.error as { message?: unknown }).message ?? 'initialize 失败')
              : 'initialize 失败';
            finish({ name: target.name, ok: false, status: 'error', latencyMs: Date.now() - startedAt, tools: [], error: errMsg });
            return;
          }
          initDone = true;
          const res = (msg.result ?? {}) as Record<string, unknown>;
          const info = (res.serverInfo ?? {}) as Record<string, unknown>;
          serverName = typeof info.name === 'string' ? info.name : undefined;
          serverVersion = typeof info.version === 'string' ? info.version : undefined;
          protocolVersion = typeof res.protocolVersion === 'string' ? res.protocolVersion : undefined;
          try {
            child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);
            child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} })}\n`);
          } catch {
            finish({
              name: target.name,
              ok: true,
              status: 'ok',
              latencyMs: Date.now() - startedAt,
              ...(serverName && { serverName }),
              ...(serverVersion && { serverVersion }),
              ...(protocolVersion && { protocolVersion }),
              tools: [],
            });
          }
        } else if (msg.id === 2) {
          const tools = msg.error ? [] : parseToolsList(msg.result);
          finish({
            name: target.name,
            ok: true,
            status: 'ok',
            latencyMs: Date.now() - startedAt,
            ...(serverName && { serverName }),
            ...(serverVersion && { serverVersion }),
            ...(protocolVersion && { protocolVersion }),
            tools,
          });
          return;
        }
      }
    });

    child.once('close', code => {
      if (initDone) {
        finish({
          name: target.name,
          ok: true,
          status: 'ok',
          latencyMs: Date.now() - startedAt,
          ...(serverName && { serverName }),
          ...(serverVersion && { serverVersion }),
          ...(protocolVersion && { protocolVersion }),
          tools: [],
        });
        return;
      }
      const detail = stderrBuf.trim().slice(0, 300);
      finish({
        name: target.name,
        ok: false,
        status: 'error',
        latencyMs: Date.now() - startedAt,
        tools: [],
        error: detail || `进程提前退出（exit ${code ?? 'unknown'}）`,
      });
    });

    child.stdin.on('error', () => {});
    child.stdin.write(`${JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        clientInfo: { name: 'hive-desktop', version: '0.1.0' },
        capabilities: {},
      },
    })}\n`);
  });
}

export async function probeMcpServer(
  target: McpServerTarget,
  timeoutMs = DEFAULT_PROBE_TIMEOUT_MS,
): Promise<McpProbeResult> {
  const startedAt = Date.now();
  const clampedTimeout = Math.min(Math.max(1000, Math.floor(timeoutMs)), MAX_PROBE_TIMEOUT_MS);
  if (target.transport === 'stdio') {
    return probeStdioMcp(target, clampedTimeout, startedAt);
  }
  if (target.transport === 'http' || target.transport === 'sse') {
    return probeHttpMcp(target, clampedTimeout, startedAt);
  }
  return {
    name: target.name,
    ok: false,
    status: 'unsupported',
    latencyMs: Date.now() - startedAt,
    tools: [],
    error: `暂不支持对该传输类型 (${target.transport}) 执行握手探活`,
  };
}
