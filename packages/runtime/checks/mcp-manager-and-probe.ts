import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  probeMcpServer,
  removeProjectMcpServer,
  resolveMcpServerTarget,
  toggleProjectMcpServer,
  upsertProjectMcpServer,
} from '../src/mcp-manager.js';

const root = mkdtempSync(join(tmpdir(), 'hive-mcp-manager-'));

try {
  // 1. Project .mcp.json CRUD preserving unrelated top-level keys
  const configPath = join(root, '.mcp.json');
  writeFileSync(configPath, JSON.stringify({ customKey: 'keep-me', mcpServers: {} }, null, 2), 'utf8');

  const stdioScript = join(root, 'mock-mcp-stdio.mjs');
  writeFileSync(
    stdioScript,
    `
import { createInterface } from 'node:readline';
const rl = createInterface({ input: process.stdin });
rl.on('line', line => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.method === 'initialize') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: msg.id,
      result: {
        protocolVersion: '2024-11-05',
        serverInfo: { name: 'mock-stdio-mcp', version: '1.2.3' },
        capabilities: { tools: {} },
      },
    }) + '\\n');
  } else if (msg.method === 'tools/list') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: msg.id,
      result: {
        tools: [
          { name: 'search_docs', description: 'Search local docs' },
          { name: 'run_lint', description: 'Run linter' },
        ],
      },
    }) + '\\n');
  }
});
`,
    'utf8',
  );

  upsertProjectMcpServer(root, {
    name: 'local-stdio',
    transport: 'stdio',
    command: process.execPath,
    args: [stdioScript],
  });

  const saved = JSON.parse(readFileSync(configPath, 'utf8')) as Record<string, unknown>;
  assert.equal(saved.customKey, 'keep-me');
  assert.ok((saved.mcpServers as Record<string, unknown>)['local-stdio']);

  // Reject invalid server names
  assert.throws(() =>
    upsertProjectMcpServer(root, { name: '../escape', transport: 'stdio', command: 'echo' }),
  );

  // Resolve and probe stdio MCP server
  const stdioTarget = resolveMcpServerTarget({ name: 'local-stdio', scope: 'project', workspacePath: root });
  assert.ok(stdioTarget);
  const stdioProbe = await probeMcpServer(stdioTarget, 4000);
  assert.equal(stdioProbe.ok, true);
  assert.equal(stdioProbe.status, 'ok');
  assert.equal(stdioProbe.serverName, 'mock-stdio-mcp');
  assert.equal(stdioProbe.serverVersion, '1.2.3');
  assert.deepEqual(stdioProbe.tools.map(t => t.name), ['search_docs', 'run_lint']);

  // Toggle disabled & re-enable
  toggleProjectMcpServer(root, 'local-stdio', true);
  const afterDisable = JSON.parse(readFileSync(configPath, 'utf8')) as { mcpServers: Record<string, { disabled?: boolean }> };
  assert.equal(afterDisable.mcpServers['local-stdio']?.disabled, true);
  toggleProjectMcpServer(root, 'local-stdio', false);
  const afterEnable = JSON.parse(readFileSync(configPath, 'utf8')) as { mcpServers: Record<string, { disabled?: boolean }> };
  assert.equal(afterEnable.mcpServers['local-stdio']?.disabled, undefined);

  // 2. Real local HTTP MCP server probe
  const server = createServer((req, res) => {
    let body = '';
    req.on('data', chunk => { body += chunk; });
    req.on('end', () => {
      const msg = JSON.parse(body) as { id: number; method: string };
      res.setHeader('content-type', 'application/json');
      if (msg.method === 'initialize') {
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          id: msg.id,
          result: {
            protocolVersion: '2024-11-05',
            serverInfo: { name: 'mock-http-mcp', version: '2.0.0' },
          },
        }));
      } else if (msg.method === 'tools/list') {
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          id: msg.id,
          result: { tools: [{ name: 'query_db', description: 'Query sqlite' }] },
        }));
      } else {
        res.end(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: {} }));
      }
    });
  });

  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  const addr = server.address();
  assert.ok(addr && typeof addr === 'object');
  const httpUrl = `http://127.0.0.1:${addr.port}/mcp`;

  try {
    upsertProjectMcpServer(root, { name: 'local-http', transport: 'http', url: httpUrl });
    const httpTarget = resolveMcpServerTarget({ name: 'local-http', scope: 'project', workspacePath: root });
    assert.ok(httpTarget);
    const httpProbe = await probeMcpServer(httpTarget, 4000);
    assert.equal(httpProbe.ok, true);
    assert.equal(httpProbe.serverName, 'mock-http-mcp');
    assert.deepEqual(httpProbe.tools.map(t => t.name), ['query_db']);
  } finally {
    await new Promise<void>(resolve => server.close(() => resolve()));
  }

  // 3. Hung stdio MCP server times out cleanly
  const hungScript = join(root, 'mock-mcp-hung.mjs');
  writeFileSync(hungScript, `setInterval(() => {}, 10000);`, 'utf8');
  const timeoutProbe = await probeMcpServer(
    { name: 'hung', transport: 'stdio', command: process.execPath, args: [hungScript] },
    1000,
  );
  assert.equal(timeoutProbe.ok, false);
  assert.equal(timeoutProbe.status, 'timeout');

  // 4. Remove server
  removeProjectMcpServer(root, 'local-http');
  const afterRemove = JSON.parse(readFileSync(configPath, 'utf8')) as { mcpServers: Record<string, unknown> };
  assert.equal(afterRemove.mcpServers['local-http'], undefined);
  assert.ok(afterRemove.mcpServers['local-stdio']);

  console.log('MCP_MANAGER_PROBE_OK: project .mcp.json CRUD, real stdio & HTTP initialize+tools/list handshake, and timeout cleanup verified.');
} finally {
  rmSync(root, { recursive: true, force: true });
}
