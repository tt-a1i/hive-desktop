import { fileURLToPath } from 'node:url';
import { terminalCommand } from './terminal-command.js';

// Copyable terminal commands, without mutating the caller's shell environment.
export function remoteCommands(dataDir: string, platform = process.platform) {
  const command = (action: string) => terminalCommand([
    process.execPath, fileURLToPath(new URL('./remote-cli.js', import.meta.url)), dataDir, action,
  ], platform);
  return { login: command('login'), logout: command('logout'), status: command('status') };
}
