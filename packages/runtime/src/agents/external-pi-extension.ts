import { PI_USER_INPUT, PI_USER_INPUT_RESULT, piUserInput, piUserPrompt, assertPiUserInputHandled } from './pi-user-input.js';
import { promptImages, imageMessage } from '../prompt-images.js';
import { existsSync, writeFileSync } from 'node:fs';
import type { ExtensionAPI } from '@earendil-works/pi-coding-agent';
import { createTeamTool } from '../team-tool.js';
import { assertExternalPiCompletion } from './external-pi-completion.js';

/** Loaded explicitly by external Pi, alongside its native user/project extensions. */
export default function hiveDesktopBridge(pi: ExtensionAPI): void {
  const command = process.env.HIVE_PI_BRIDGE_COMMAND;
  const toolName = process.env.HIVE_PI_TEAM_TOOL;
  if (!command || !toolName) throw new Error('Missing Hive external Pi bridge identity');
  let binding: { workspaceId: string; agentId: string; token: string; hivePort: string; cwd: string } | undefined;
  const team = createTeamTool(() => {
    if (!binding) throw new Error('Hive team binding is not active');
    return binding;
  });
  // Per-child name prevents silently replacing a user's existing team tool.
  pi.registerTool({ ...team, name: toolName });
  pi.registerCommand(command, {
    description: 'Hive Desktop private session bridge',
    async handler(args, ctx) {
      const value: unknown = JSON.parse(args);
      if (!value || typeof value !== 'object' || !('action' in value)) throw new Error('Invalid Hive bridge payload');
      if (!ctx.isIdle()) throw new Error('Hive bridge requires an idle Pi session');
      if (value.action === 'bind') {
        const candidate = 'binding' in value ? value.binding : undefined;
        if (candidate === undefined || candidate === null) { binding = undefined; return; }
        if (typeof candidate !== 'object') throw new Error('Invalid Hive team binding');
        for (const key of ['workspaceId', 'agentId', 'token', 'hivePort', 'cwd']) {
          if (!(key in candidate) || typeof candidate[key as keyof typeof candidate] !== 'string' || !candidate[key as keyof typeof candidate]) throw new Error('Invalid Hive team binding');
        }
        binding = candidate as typeof binding;
      } else if (value.action === 'begin-user-input') {
        const input = piUserInput('input' in value ? value.input : undefined);
        if (!input) throw new Error('Invalid native user input metadata');
        const native = piUserPrompt(input);
        pi.appendEntry(PI_USER_INPUT, input);
        if (native.context.trim()) pi.sendMessage({ customType: 'hive-user-input-context', content: native.context,
          display: false, details: { deliveryId: input.deliveryId } }, { triggerTurn: false });
      } else if (value.action === 'end-user-input') {
        if (!('deliveryId' in value) || typeof value.deliveryId !== 'string' || !('disposition' in value)
          || !['started', 'queued', 'handled', 'failed'].includes(String(value.disposition))) throw new Error('Invalid native input result');
        pi.appendEntry(PI_USER_INPUT_RESULT, { deliveryId: value.deliveryId, disposition: value.disposition });
      } else if (value.action === 'complete-user-input') {
        if (!('deliveryId' in value) || typeof value.deliveryId !== 'string') throw new Error('Missing native input ID');
        assertPiUserInputHandled(ctx.sessionManager.getBranch(), value.deliveryId);
        pi.appendEntry('hive-delivery-completed', { id: value.deliveryId });
      } else if (value.action === 'user') {
        if (!('text' in value) || typeof value.text !== 'string') throw new Error('Hive user message must be a string');
        pi.sendUserMessage(imageMessage(value.text,promptImages('images' in value?value.images:undefined)), { expandPromptTemplates: false });
      } else if (value.action === 'initialize') {
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
        pi.sendMessage({ customType, content: imageMessage(value.text,promptImages('images' in value?value.images:undefined)), display: false, details }, { triggerTurn: value.action === 'delivery' && (!('triggerTurn' in value) || value.triggerTurn !== false) });
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
