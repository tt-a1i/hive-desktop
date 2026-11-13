import { Terminals, type TerminalEvent } from './terminal.js';
import { AgentModels } from './agent-models.js';
import { join } from 'node:path';
import { Chats, type ChatEvent } from './chats.js';
import { createHiveHost } from './hive-host.js';
import type { HiveAgentRuntime } from './hive-runtime-contract.js';
import { DesktopSettings, type DesktopExecutableEnvironment } from './desktop-settings.js';
import { createStructuredRuntime } from './structured-runtime.js';
import { PiAuth } from './pi-auth.js';
import { PiModelConfig } from './pi-model-config.js';
import { isWorkerRole } from '@tt-a1i/hive/dist/src/shared/types.js';
import { structuredCommand } from './structured-runtime.js';

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
  const piModels = new PiModelConfig();
  try {
    if (!agents) throw new Error('Hive runtime factory was not installed');
    chats = new Chats(hive, dataDir, emit, agents, settings, agentModels);
  } catch (error) { await hive.close(); throw error; }
  const terminal = new Terminals(hive, workspaceId => chats.assertWorkspaceAvailable(workspaceId), options.onTerminalEvent ?? (() => {}));
  let closing: Promise<void> | undefined;
  return { hive, chats, agents, settings, agentModels, terminal, piAuth, piModels,
    createMember: (workspaceId: string, input: { name: unknown; role: unknown; backend: unknown; description?: unknown }) => {
      chats.assertWorkspaceAvailable(workspaceId);
      if (typeof input.name !== 'string' || !input.name.trim() || !isWorkerRole(input.role)
        || typeof input.backend !== 'string' || !structuredCommand(`hive:${input.backend}`)
        || (input.description !== undefined && typeof input.description !== 'string')) throw new TypeError('Invalid desktop member configuration');
      return hive.createMember(workspaceId, { name: input.name, role: input.role,
        ...(input.description !== undefined && { description: input.description as string }) }, `hive:${input.backend}`);
    },
    removeWorkspace: (workspaceId: string) => chats.removeWorkspace(workspaceId, () => terminal.detachWorkspace(workspaceId)),
    requestHive: (method: string, path: string, body?: unknown) => {
      if (!['GET', 'HEAD'].includes(method.toUpperCase())) {
        const match = /^\/api\/(?:ui\/)?workspaces\/([^/?#]+)(?:[/?#]|$)/.exec(path);
        if (match) chats.assertWorkspaceAvailable(decodeURIComponent(match[1]!));
      }
      return hive.request(method, path, body);
    },
    close: () => closing ??= (async () => {
      const stage1 = await Promise.allSettled([settings.close(), piModels.close(), piAuth.close(), terminal.close(), agentModels.close()]);
      const stage2 = await Promise.allSettled([hive.close()]);
      const stage3 = await Promise.allSettled([chats.close()]);
      const errors = [...stage1, ...stage2, ...stage3].flatMap(result => result.status === 'rejected' ? [result.reason] : []);
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) throw new AggregateError(errors, 'Desktop runtime close failed');
    })(),
  };
}
