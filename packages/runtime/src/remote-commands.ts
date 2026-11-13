import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { terminalCommand } from './terminal-command.js';

function resolveRemoteCli(): string {
  if (!import.meta.url.endsWith('.ts')) return fileURLToPath(new URL('./remote-cli.js', import.meta.url));
  const built = fileURLToPath(new URL('../dist/remote-cli.js', import.meta.url));
  return existsSync(built) ? built : fileURLToPath(new URL('./remote-cli.ts', import.meta.url));
}

// Copyable terminal commands, without mutating the caller's shell environment.
export function remoteCommands(dataDir: string, platform = process.platform) {
  const script = resolveRemoteCli();
  const command = (action: string) => terminalCommand([process.execPath, script, dataDir, action], platform);
  return { login: command('login'), logout: command('logout'), status: command('status') };
}
