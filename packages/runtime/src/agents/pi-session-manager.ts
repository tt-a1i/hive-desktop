import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { SessionManager } from '@earendil-works/pi-coding-agent';

/** Desktop sessions contain custom deliveries before their first assistant reply. */
export function createDesktopSessionManager(cwd: string, sessionDir: string, sessionFile?: string): SessionManager {
  const manager = sessionFile ? SessionManager.open(sessionFile, sessionDir) : SessionManager.create(cwd, sessionDir);
  const path = manager.getSessionFile();
  if (path && !existsSync(path)) {
    // Pi normally delays creating this file until a regular user/assistant entry.
    // Load the real header through its public API so custom entries persist too.
    mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
    try {
      writeFileSync(path, `${JSON.stringify(manager.getHeader())}\n`, { flag: 'wx', mode: 0o600, flush: true });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
    }
    manager.setSessionFile(path);
  }
  return manager;
}
