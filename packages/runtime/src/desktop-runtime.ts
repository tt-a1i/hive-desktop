import { AgentModels } from './agent-models.js';
import { join } from 'node:path';
import { Chats, type ChatEvent } from './chats.js';
import { createHiveHost } from './hive-host.js';
import type { HiveAgentRuntime } from './hive-runtime-contract.js';
import { DesktopSettings, type DesktopExecutableEnvironment } from './desktop-settings.js';
import { createStructuredRuntime } from './structured-runtime.js';

export async function createDesktopRuntime(dataDir: string, emit: (event: ChatEvent) => void,
  options: { executableEnvironment?: DesktopExecutableEnvironment } = {}) {
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
  try {
    if (!agents) throw new Error('Hive runtime factory was not installed');
    chats = new Chats(hive, dataDir, emit, agents, settings);
  } catch (error) { await hive.close(); throw error; }
  const agentModels = new AgentModels(hive, settings);
  let closing: Promise<void> | undefined;
  return { hive, chats, agents, settings, agentModels,
    close: () => closing ??= (async () => {
      try { await agentModels.close(); await hive.close(); } finally { await chats.close(); }
    })(),
  };
}
