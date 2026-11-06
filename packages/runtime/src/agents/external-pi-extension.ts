import { existsSync, writeFileSync } from 'node:fs';
import type { ExtensionAPI } from '@earendil-works/pi-coding-agent';
import { createTeamTool } from '../team-tool.js';
import { assertExternalPiCompletion } from './external-pi-completion.js';

/** Loaded explicitly by external Pi, alongside its native user/project extensions. */
export default function hiveDesktopBridge(pi: ExtensionAPI): void {
  const command = process.env.HIVE_PI_BRIDGE_COMMAND;
  const toolName = process.env.HIVE_PI_TEAM_TOOL;
  if (!command || !toolName) throw new Error('Missing Hive external Pi bridge identity');
  const team = createTeamTool(() => {
    const required = (name: string): string => {
      const value = process.env[name];
      if (!value) throw new Error(`Missing Hive team environment: ${name}`);
      return value;
    };
    return { workspaceId: required('HIVE_PROJECT_ID'), agentId: required('HIVE_AGENT_ID'),
      token: required('HIVE_AGENT_TOKEN'), hivePort: required('HIVE_PORT'), cwd: process.cwd() };
  });
  // Per-child name prevents silently replacing a user's existing team tool.
  pi.registerTool({ ...team, name: toolName });
  pi.registerCommand(command, {
    description: 'Hive Desktop private session bridge',
    async handler(args, ctx) {
      const value: unknown = JSON.parse(args);
      if (!value || typeof value !== 'object' || !('action' in value)) throw new Error('Invalid Hive bridge payload');
      if (!ctx.isIdle()) throw new Error('Hive bridge requires an idle Pi session');
      if (value.action === 'initialize') {
        const tools = pi.getAllTools().filter(tool => tool.name === toolName);
        if (tools.length !== 1) throw new Error('Hive external Pi team tool collision');
        const path = ctx.sessionManager.getSessionFile();
        if (!path) throw new Error('Hive external Pi requires a persistent session');
        pi.appendEntry('hive-bridge-ready', { version: 1, command, teamToolName: toolName });
        if (!existsSync(path)) {
          // Public external-extension equivalent of the built-in SessionManager bootstrap.
          // Pi defers custom-only sessions; write its real header/entries then reload publicly.
          const entries = [ctx.sessionManager.getHeader(), ...ctx.sessionManager.getEntries()];
          writeFileSync(path, entries.map(entry => JSON.stringify(entry)).join('\n') + '\n', { flag: 'wx', mode: 0o600, flush: true });
          const result = await ctx.switchSession(path);
          if (result.cancelled) throw new Error('External Pi extension cancelled persistent session initialization');
        }
      } else if (value.action === 'context' || value.action === 'delivery') {
        if (!('text' in value) || typeof value.text !== 'string') throw new Error('Hive bridge text must be a string');
        const customType = value.action === 'delivery' ? 'hive-delivery'
          : 'customType' in value && typeof value.customType === 'string' ? value.customType : 'hive-context';
        const details = 'details' in value ? value.details : undefined;
        pi.sendMessage({ customType, content: value.text, display: false, details }, { triggerTurn: value.action === 'delivery' && (!('triggerTurn' in value) || value.triggerTurn !== false) });
      } else if (value.action === 'complete') {
        if (!('deliveryId' in value) || typeof value.deliveryId !== 'string') throw new Error('Missing Hive delivery ID');
        const id = value.deliveryId;
        if (!('assistantEntryId' in value) || typeof value.assistantEntryId !== 'string') throw new Error('Missing Hive terminal assistant entry ID');
        assertExternalPiCompletion(ctx.sessionManager.getBranch(), id, value.assistantEntryId);
        pi.appendEntry('hive-delivery-completed', { id });
      } else throw new Error('Unsupported Hive bridge action');
    },
  });
}
