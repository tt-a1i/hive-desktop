import { Terminals, type TerminalEvent } from './terminal.js';
import { AgentModels } from './agent-models.js';
import { join } from 'node:path';
import { Chats, type ChatEvent } from './chats.js';
import { createHiveHost } from './hive-host.js';
import type { HiveAgentRuntime } from './hive-runtime-contract.js';
import { DesktopSettings, type DesktopExecutableEnvironment } from './desktop-settings.js';
import { createStructuredRuntime } from './structured-runtime.js';
import { PiAuth } from './pi-auth.js';

export async function createDesktopRuntime(dataDir: string, emit: (event: ChatEvent) => void,
  options: { executableEnvironment?: DesktopExecutableEnvironment; onTerminalEvent?: (event: TerminalEvent) => void } = {}) {
  let chats: Chats;
  let agents: HiveAgentRuntime | undefined;
  const hive = await createHiveHost({
    dataDir: join(dataDir, 'hive'),
    agentRuntimeFactory: context => {
      agents = createStructuredRuntime(context, { start: input => chats.start(input) });
      return agents;
    },
  });
  const settings = new DesktopSettings(hive, options.executableEnvironment);
  const agentModels = new AgentModels(hive, settings);
  const piAuth = new PiAuth();
  try {
    if (!agents) throw new Error('Hive runtime factory was not installed');
    chats = new Chats(hive, dataDir, emit, agents, settings, agentModels);
  } catch (error) { await hive.close(); throw error; }
  const terminal = new Terminals(hive, workspaceId => chats.assertWorkspaceAvailable(workspaceId), options.onTerminalEvent ?? (() => {}));
  let closing: Promise<void> | undefined;
  return { hive, chats, agents, settings, agentModels, terminal, piAuth,
    removeWorkspace: (workspaceId: string) => chats.removeWorkspace(workspaceId, () => terminal.detachWorkspace(workspaceId)),
    requestHive: (method: string, path: string, body?: unknown) => {
      if (!['GET', 'HEAD'].includes(method.toUpperCase())) {
        const match = /^\/api\/workspaces\/([^/?#]+)(?:[/?#]|$)/.exec(path);
        if (match) chats.assertWorkspaceAvailable(decodeURIComponent(match[1]!));
      }
      return hive.request(method, path, body);
    },
    close: () => closing ??= (async () => {
      try { await piAuth.close(); await terminal.close(); await agentModels.close(); } finally { try { await hive.close(); } finally { await chats.close(); } }
    })(),
  };
}
