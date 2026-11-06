import { execFile, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { join } from 'node:path';
import crossSpawn from 'cross-spawn';

/** A spawned CLI and the shutdown promise its caller must await. */
export class OwnedCliProcess {
  readonly child: ChildProcessWithoutNullStreams;
  private readonly exited: Promise<void>;
  private closed = false;
  private rootExited = false;
  private closeTask: Promise<void> | undefined;

  constructor(command: string, args: string[], options: { cwd: string; env?: NodeJS.ProcessEnv }) {
    // cross-spawn resolves Windows shims/PATHEXT and escapes argv; no shell command assembly here.
    this.child = crossSpawn(command, args, { ...options, stdio: ['pipe', 'pipe', 'pipe'], shell: false, windowsHide: true }) as ChildProcessWithoutNullStreams;
    this.exited = new Promise(resolve => this.child.once('close', () => { this.closed = true; resolve(); }));
    this.child.once('exit', () => { this.rootExited = true; });
  }

  close(graceMs = 1500): Promise<void> {
    if (this.closeTask) return this.closeTask;
    this.closeTask = this.shutdown(graceMs);
    return this.closeTask;
  }

  private async shutdown(graceMs: number): Promise<void> {
    this.child.stdin.end();
    if (await this.waitForExit(graceMs)) return;
    if (process.platform === 'win32') {
      // Killing cmd.exe alone leaves the real npm/pnpm CLI running. /T includes its descendants.
      // Never target a PID after observing its root exit: it may have been recycled.
      if (this.rootExited || !this.child.pid) throw new Error('Owned Windows CLI root exited before its inherited streams closed; descendant cleanup cannot be verified');
      const windows = process.env.SystemRoot ?? process.env.WINDIR;
      if (!windows) throw new Error('Windows SystemRoot is required for owned CLI tree termination');
      try {
        await new Promise<void>((resolve, reject) => execFile(join(windows, 'System32', 'taskkill.exe'), ['/PID', String(this.child.pid), '/T', '/F'],
          { windowsHide: true, timeout: 5000, maxBuffer: 16384 }, error => error ? reject(error) : resolve()));
      } catch (error) { if (!this.closed) throw new Error('Failed to terminate the owned Windows CLI process tree', { cause: error }); }
      if (!await this.waitForExit(2000)) throw new Error('Owned Windows CLI streams remained open after process tree termination');
      return;
    }
    this.child.kill('SIGTERM');
    if (await this.waitForExit(1000)) return;
    this.child.kill('SIGKILL');
    if (!await this.waitForExit(2000)) throw new Error('Owned CLI did not close after SIGKILL');
  }

  private async waitForExit(ms: number): Promise<boolean> {
    if (this.closed) return true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try { return await Promise.race([this.exited.then(() => true), new Promise<false>(resolve => { timer = setTimeout(() => resolve(false), ms); })]); }
    finally { clearTimeout(timer); }
  }
}
