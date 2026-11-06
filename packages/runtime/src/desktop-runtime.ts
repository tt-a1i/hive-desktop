import { join } from 'node:path';
import { Chats, type ChatEvent } from './chats.js';
import { createHiveHost } from './hive-host.js';
import type { HiveAgentRuntime } from './hive-runtime-contract.js';
import { createStructuredRuntime } from './structured-runtime.js';

export async function createDesktopRuntime(dataDir: string, emit: (event: ChatEvent) => void) {
  let chats: Chats;
  let agents: HiveAgentRuntime | undefined;
  const hive = await createHiveHost({
    dataDir: join(dataDir, 'hive'),
    agentRuntimeFactory: context => {
      agents = createStructuredRuntime(context, { start: input => chats.start(input) });
      return agents;
    },
  });
  try {
    if (!agents) throw new Error('Hive runtime factory was not installed');
    chats = new Chats(hive, dataDir, emit, agents);
  } catch (error) { await hive.close(); throw error; }
  let closing: Promise<void> | undefined;
  return { hive, chats, agents,
    close: () => closing ??= (async () => {
      try { await hive.close(); } finally { await chats.close(); }
    })(),
  };
}
