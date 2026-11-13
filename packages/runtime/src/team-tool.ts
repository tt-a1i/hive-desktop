import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import type { ToolDefinition } from '@earendil-works/pi-coding-agent';
import { Type } from 'typebox';

const require = createRequire(import.meta.url);
const teamCli = require.resolve('@tt-a1i/hive/dist/src/cli/team.js');
export const teamParameters = Type.Object({
  args: Type.Array(Type.String(), { description: 'CLI arguments after team, one argument per array item.' }),
  stdin: Type.Optional(Type.String({ description: 'Input for commands using --stdin.' })),
});

type TeamToolOptions = {
  workspaceId: string;
  agentId: string;
  token: string;
  hivePort: string;
  cwd: string;
};

export function createTeamCommand(source: TeamToolOptions | (() => TeamToolOptions)) {
  return {
    name: 'team',
    label: 'Team',
    description: 'Run the native Hive team CLI (list, send, spawn, report, messages, guide, and other commands). Pass each CLI argument separately. For send, pass task text as a positional argument; it does not accept --stdin. Use stdin only for commands that support --stdin, such as report. The CLI handles parsing, authentication, and the dispatch ledger.',
    parameters: teamParameters,
    async execute(_id: string, { args, stdin }: { args: string[]; stdin?: string }, signal?: AbortSignal) {
      const options = typeof source === 'function' ? source() : source;
      if (signal?.aborted) throw new Error('Team command aborted');
      const output = await new Promise<string>((resolve, reject) => {
        const child = spawn(process.execPath, [teamCli, ...args], {
          cwd: options.cwd,
          env: {
            ...process.env,
            HIVE_PORT: options.hivePort,
            HIVE_PROJECT_ID: options.workspaceId,
            HIVE_AGENT_ID: options.agentId,
            HIVE_AGENT_TOKEN: options.token,
          },
          stdio: ['pipe', 'pipe', 'pipe'],
        });
        let stdout = '';
        let stderr = '';
        child.stdout.setEncoding('utf8').on('data', (chunk: string) => { stdout += chunk; });
        child.stderr.setEncoding('utf8').on('data', (chunk: string) => { stderr += chunk; });
        // A CLI that exits before reading stdin can close the pipe early.
        child.stdin.on('error', (error: NodeJS.ErrnoException) => {
          if (error.code !== 'EPIPE') reject(error);
        });
        const abort = () => child.kill();
        signal?.addEventListener('abort', abort, { once: true });
        child.once('error', (error) => {
          signal?.removeEventListener('abort', abort);
          reject(error);
        });
        child.on('close', (code) => {
          signal?.removeEventListener('abort', abort);
          if (signal?.aborted) return reject(new Error('Team command aborted'));
          const text = [stdout.trimEnd(), stderr.trimEnd()].filter(Boolean).join('\n');
          if (code !== 0) return reject(new Error(text || `Team command exited with code ${code}`));
          resolve(text);
        });
        child.stdin.end(stdin);
      });
      return { content: [{ type: 'text' as const, text: output }], details: undefined };
    },
  };
}

export function createTeamTool(source: TeamToolOptions | (() => TeamToolOptions)): ToolDefinition<typeof teamParameters> {
  return createTeamCommand(source);
}
