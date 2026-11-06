/** Real SDK, native Codex/Pi RPC, Hive HTTP/SQLite. No prompts or Hive agent activation. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import { homedir } from 'node:os';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { AgentModelsInputError, type AgentModelCatalog } from '../src/agent-models.js';

await mkdir(resolve('../../.artifacts'), { recursive: true });
const directory = await mkdtemp(resolve('../../.artifacts/agent-models-'));
const dataDir = join(directory, 'state');
const runtime = await createDesktopRuntime(dataDir, () => {}, { executableEnvironment: {} });
const globalSettings = join(homedir(), '.pi', 'agent', 'settings.json');
const settingsHash = async () => existsSync(globalSettings) ? createHash('sha256').update(await readFile(globalSettings)).digest('hex') : null;
const beforeHash = await settingsHash();
function ownedNativeChildren(): number[] {
  return execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).trim().split('\n').flatMap(line => {
    const match = /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line)!;
    const command = match[3]!.trim();
    return Number(match[2]) === process.pid && (command === 'pi' || /\/codex app-server --listen/.test(command)) ? [Number(match[1])] : [];
  });
}
try {
  await runtime.settings.update({ piCommand: '/Users/tsk/Library/pnpm/pi', codexCommand: '/Users/tsk/.npm-global/bin/codex' });
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Native model catalogs', autostart_orchestrator: false,
  }) as { id: string };
  const agentId = `${workspace.id}:orchestrator`;
  const beforeChildren = ownedNativeChildren();
  const catalogs: Record<string, AgentModelCatalog> = {};
  for (const backend of ['pi', 'codex', 'pi-rpc'] as const) {
    catalogs[backend] = await runtime.agentModels.list(workspace.id, backend);
    assert.ok(catalogs[backend].models.some(model => model.provider === 'openai' && model.id === 'gpt-6.1-terra' && model.available), `${backend} must expose native Terra availability`);
    for (const model of catalogs[backend].models) {
      assert.deepEqual(Object.keys(model).sort(), ['available', 'id', 'name', 'provider']);
    }
    assert.deepEqual(runtime.chats.list(workspace.id), []);
    assert.equal(runtime.agents.getActiveRunByAgentId(workspace.id, agentId), undefined);
    assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
    assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/runs`), []);
    assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`), []);
    assert.deepEqual(ownedNativeChildren(), beforeChildren, 'Owned temporary native processes must be reaped before catalog resolves');
  }
  await assert.rejects(runtime.agentModels.list(workspace.id, 'terminal'), AgentModelsInputError);
  await assert.rejects(runtime.agentModels.list('missing-workspace', 'pi'), AgentModelsInputError);
  await assert.rejects(runtime.agentModels.list('', 'codex'), AgentModelsInputError);
  await runtime.settings.update({ piCommand: join(directory, 'missing pi'), codexCommand: join(directory, 'missing codex') });
  for (const backend of ['codex', 'pi-rpc']) {
    await assert.rejects(runtime.agentModels.list(workspace.id, backend), (error: NodeJS.ErrnoException) => error.code === 'ENOENT');
    assert.deepEqual(ownedNativeChildren(), beforeChildren);
  }
  assert.equal(await settingsHash(), beforeHash, 'Native catalogs must not rewrite the global Pi model/provider defaults');
  assert.equal(existsSync(join(dataDir, 'chats', 'index.json')), false);
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ workspaceId: workspace.id, catalogs,
    noChatsOrRunsOrDispatches: true, noPromptsInvoked: true, temporaryNativeChildrenReaped: true,
    invalidInputsAndRealExecutableFailuresRejected: true, globalSettingsUnchanged: true }, null, 2));
  console.log(`AGENT_MODELS_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined }, null, 2));
  throw error;
} finally {
  await runtime.close();
  console.log(`Evidence retained: ${directory}`);
}
