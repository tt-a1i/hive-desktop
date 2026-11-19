import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import crossSpawn from 'cross-spawn';

export function ownedWindowsCliLaunch(command: string, args: string[]) {
  const host = import.meta.url.endsWith('.ts') ? './owned-windows-cli-host.ts' : './owned-windows-cli-host.js';
  return { command: process.execPath, args: [fileURLToPath(new URL(host, import.meta.url)), command, JSON.stringify(args)] };
}

/** A spawned CLI and the shutdown promise its caller must await. */
export class OwnedCliProcess {
  readonly child: ChildProcessWithoutNullStreams;
  private readonly nativeProbeOwner: boolean;
  private readonly exited: Promise<void>;
  private closed = false;
  private rootExited = false;
  private closeTask: Promise<void> | undefined;
  private readonly startedAt = performance.now();
  private readonly events: { atMs: number; type: string; code?: number | string | null; signal?: NodeJS.Signals | null }[] = [];

  constructor(command: string, args: string[], options: { cwd: string; env?: NodeJS.ProcessEnv; readOnlyProbe?: boolean }) {
    // cross-spawn resolves Windows shims/PATHEXT and escapes argv; no shell command assembly here.
    const { readOnlyProbe, ...spawnOptions } = options;
    const probeOwner = readOnlyProbe && process.platform === 'darwin'
      ? process.env.HIVE_DESKTOP_SHELL_OWNER : undefined;
    this.nativeProbeOwner = !!probeOwner;
    const launch = probeOwner
      ? { command: probeOwner, args: ['--hive-owned-cli', String(process.pid), command, ...args] }
      : process.platform === 'win32'
      ? ownedWindowsCliLaunch(command, args)
      : { command, args };
    this.child = crossSpawn(launch.command, launch.args, { ...spawnOptions, detached: !!probeOwner, stdio: ['pipe', 'pipe', 'pipe'], shell: false, windowsHide: true }) as ChildProcessWithoutNullStreams;
    this.child.once('spawn', () => this.record('spawn'));
    this.child.once('error', error => this.record('error', (error as NodeJS.ErrnoException).code));
    this.exited = new Promise(resolve => this.child.once('close', (code, signal) => { this.closed = true; this.record('close', code, signal); resolve(); }));
    this.child.once('exit', (code, signal) => { this.rootExited = true; this.record('exit', code, signal); });
    this.child.stdin.once('finish', () => this.record('stdin.finish'));
    this.child.stdin.once('close', () => this.record('stdin.close'));
    this.child.stdout.once('end', () => this.record('stdout.end'));
    this.child.stdout.once('close', () => this.record('stdout.close'));
    this.child.stderr.once('end', () => this.record('stderr.end'));
    this.child.stderr.once('close', () => this.record('stderr.close'));
  }

  /** Lifecycle facts only: never contains argv, stdin, output content or environment. */
  get diagnostics() {
    return { pid: this.child.pid, spawnfile: this.child.spawnfile, rootExited: this.rootExited, closed: this.closed,
      exitCode: this.child.exitCode, signalCode: this.child.signalCode,
      stdin: { finished: this.child.stdin.writableFinished, destroyed: this.child.stdin.destroyed, closed: this.child.stdin.closed, bufferedBytes: this.child.stdin.writableLength },
      stdout: { ended: this.child.stdout.readableEnded, destroyed: this.child.stdout.destroyed, closed: this.child.stdout.closed },
      stderr: { ended: this.child.stderr.readableEnded, destroyed: this.child.stderr.destroyed, closed: this.child.stderr.closed },
      events: this.events.map(event => ({ ...event })) };
  }
  private record(type: string, code?: number | string | null, signal?: NodeJS.Signals | null): void {
    this.events.push({ atMs: Math.round(performance.now() - this.startedAt), type, ...(code !== undefined && { code }), ...(signal !== undefined && { signal }) });
  }

  close(graceMs = 1500): Promise<void> {
    if (this.closeTask) return this.closeTask;
    this.closeTask = this.shutdown(graceMs);
    return this.closeTask;
  }

  private async shutdown(graceMs: number): Promise<void> {
    this.record('stdin.end.requested');
    this.child.stdin.end();
    if (await this.waitForExit(graceMs)) return;
    this.record('grace.expired');
    if (process.platform === 'win32') {
      // Exiting the launcher closes its sole Job handle and terminates its descendants.
      if (!this.rootExited) this.child.kill('SIGKILL');
      if (!await this.waitForExit(2000)) throw new Error('Owned Windows CLI streams remained open after its Job host exited');
      return;
    }
    this.child.kill('SIGTERM');
    if (this.nativeProbeOwner) {
      // TERM asks the host to reap its CLI group. Keep the host alive while it
      // confirms exit, including when a descendant is slow to release a pipe.
      if (!await this.waitForExit(4000)) throw new Error('Native probe process cleanup could not be confirmed');
      return;
    }
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
