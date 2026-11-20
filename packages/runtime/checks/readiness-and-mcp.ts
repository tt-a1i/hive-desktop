import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { collectReadinessReport } from '../src/readiness.js';

const root = mkdtempSync(join(tmpdir(), 'hive-readiness-'));
try {
  const home = join(root, 'home');
  const piDir = join(home, '.pi', 'agent');
  const codexDir = join(home, '.codex');
  const binDir = join(root, 'bin');
  mkdirSync(join(piDir, 'extensions'), { recursive: true });
  mkdirSync(codexDir, { recursive: true });
  mkdirSync(binDir, { recursive: true });

  // Fake executable for codex
  const fakeCodex = join(binDir, process.platform === 'win32' ? 'codex.cmd' : 'codex');
  writeFileSync(fakeCodex, '#!/bin/sh\necho codex-cli 0.169.0\n');
  if (process.platform !== 'win32') chmodSync(fakeCodex, 0o755);

  // Built-in Pi auth + extension
  writeFileSync(join(piDir, 'auth.json'), JSON.stringify({ openai: { type: 'oauth' } }));
  writeFileSync(join(piDir, 'extensions', 'custom-tool.ts'), 'export default {};');

  // Codex config.toml with 2 MCP servers (1 enabled with # inside URL, 1 disabled) and auth.json
  writeFileSync(join(codexDir, 'auth.json'), JSON.stringify({ tokens: {} }));
  writeFileSync(join(codexDir, 'config.toml'), [
    '[mcp_servers.context7]',
    'url = "https://mcp.context7.com/mcp#v2" # trailing comment',
    '',
    '[mcp_servers.local_db]',
    'command = "/usr/local/bin/db-mcp"',
    'enabled = false',
  ].join('\n'));

  // Project-level MCP configs
  const workspaceDir = join(root, 'project');
  mkdirSync(join(workspaceDir, '.claude'), { recursive: true });
  mkdirSync(join(workspaceDir, '.codex'), { recursive: true });
  writeFileSync(join(workspaceDir, '.mcp.json'), JSON.stringify({
    mcpServers: { proj_figma: { url: 'https://api.figma.com/mcp' } },
  }));
  writeFileSync(join(workspaceDir, '.codex', 'config.toml'), [
    '[mcp_servers.proj_docs]',
    'command = "/usr/local/bin/docs#tool" # inline comment',
  ].join('\n'));

  // Malformed Claude config should report error without crashing
  writeFileSync(join(home, '.claude.json'), '{ invalid json');

  const fakeSettings = {
    get: async () => ({
      codexCommand: fakeCodex,
      claudeCommand: '/nonexistent/path/to/claude',
      piCommand: 'definitely-missing-pi-binary-xyz',
    }),
  } as unknown as import('../src/desktop-settings.js').DesktopSettings;
  const fakeProfiles = {
    list: async () => ({ profiles: [] }),
  } as unknown as import('../src/agent-profiles.js').AgentProfiles;

  const report = await collectReadinessReport({
    settings: fakeSettings,
    profiles: fakeProfiles,
    workspacePath: workspaceDir,
    homeDir: home,
    env: { PATH: binDir, PI_CODING_AGENT_DIR: piDir, CODEX_HOME: codexDir },
  });

  const byId = Object.fromEntries(report.backends.map(b => [b.backend, b]));
  assert.equal(byId.pi?.level, 'ready');
  assert.equal(byId.codex?.level, 'ready');
  assert.equal(byId.codex?.mcpCount, 3);
  assert.equal(byId.claude?.level, 'invalid_path');
  assert.equal(byId['pi-rpc']?.level, 'missing_executable');
  assert.equal(byId.acp?.level, 'unconfigured');

  const c7 = report.mcpResources.find(r => r.name === 'context7');
  assert.equal(c7?.source, 'codex');
  assert.equal(c7?.scope, 'global');
  assert.equal(c7?.transport, 'http');
  assert.equal(c7?.detail, 'mcp.context7.com');
  assert.equal(c7?.status, 'configured');
  assert.ok(c7?.configPath?.endsWith('config.toml'));

  const projFigma = report.mcpResources.find(r => r.name === 'proj_figma');
  assert.equal(projFigma?.source, 'project');
  assert.equal(projFigma?.scope, 'project');
  assert.equal(projFigma?.detail, 'api.figma.com');

  const projDocs = report.mcpResources.find(r => r.name === 'proj_docs');
  assert.equal(projDocs?.source, 'codex');
  assert.equal(projDocs?.scope, 'project');
  assert.equal(projDocs?.detail, 'docs#tool');

  const localDb = report.mcpResources.find(r => r.name === 'local_db');
  assert.equal(localDb?.status, 'disabled');
  assert.equal(localDb?.detail, 'db-mcp');

  const claudeErr = report.mcpResources.find(r => r.source === 'claude' && r.status === 'error');
  assert.ok(claudeErr?.error);

  const piExt = report.mcpResources.find(r => r.source === 'pi' && r.transport === 'extension');
  assert.equal(piExt?.name, 'custom-tool.ts');

  console.log('READINESS_MCP_OK: all 5 backends readiness levels, Codex TOML MCP, Pi extensions, and malformed config diagnostics verified read-only.');
} finally {
  rmSync(root, { recursive: true, force: true });
}
