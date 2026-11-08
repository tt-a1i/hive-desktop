import { fileURLToPath } from 'node:url';

// Copyable terminal commands, without mutating the caller's shell environment.
export function remoteCommands(dataDir: string, platform = process.platform) {
  const quote = platform === 'win32'
    ? (value: string) => `'${value.replaceAll("'", "''")}'`
    : (value: string) => `'${value.replaceAll("'", "'\"'\"'")}'`;
  const prefix = platform === 'win32' ? '& ' : '';
  const command = (action: string) => prefix + [
    process.execPath, fileURLToPath(new URL('./remote-cli.js', import.meta.url)), dataDir, action,
  ].map(quote).join(' ');
  return { login: command('login'), logout: command('logout'), status: command('status') };
}
