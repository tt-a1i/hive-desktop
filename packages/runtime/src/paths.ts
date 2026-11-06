import { homedir } from 'node:os';
import { isAbsolute, join, resolve } from 'node:path';

export function desktopDataDir(argv = process.argv.slice(2)): string {
  const at = argv.indexOf('--data-dir');
  if (at !== -1) {
    const value = argv[at + 1];
    if (!value || value.startsWith('--')) throw new Error('--data-dir requires a path');
    return resolve(value);
  }
  if (process.env.HIVE_DESKTOP_DATA_DIR) return resolve(process.env.HIVE_DESKTOP_DATA_DIR);
  if (process.platform === 'darwin') return join(homedir(), 'Library', 'Application Support', 'Hive Desktop');
  if (process.platform === 'win32') return join(process.env.LOCALAPPDATA ?? join(homedir(), 'AppData', 'Local'), 'Hive Desktop');
  const configured = process.env.XDG_DATA_HOME;
  return join(configured && isAbsolute(configured) ? configured : join(homedir(), '.local', 'share'), 'hive-desktop');
}
