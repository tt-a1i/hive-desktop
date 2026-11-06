import crossSpawn from 'cross-spawn';
import { Job, supported } from '@seydx/win-job';

if (process.platform !== 'win32' || !supported) throw new Error('Windows CLI Job ownership is unavailable');
const command = process.argv[2];
const args: unknown = JSON.parse(process.argv[3] ?? 'null');
if (!command || !Array.isArray(args) || !args.every(value => typeof value === 'string')) throw new Error('Invalid owned Windows CLI arguments');

// Assign this launcher BEFORE spawning: assigning an already-running CLI would miss early descendants.
// The native handle is non-inheritable and stays open until this process exits, including crashes.
const job = new Job({ killOnClose: true, allowBreakaway: false });
job.assign(process.pid);
const child = crossSpawn(command, args, { stdio: 'inherit', shell: false, windowsHide: true });
const result = await new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve, reject) => {
  child.once('error', reject);
  child.once('close', (code, signal) => resolve({ code, signal }));
});
// Natural event-loop exit closes our sole Job handle; Windows then terminates any leftover descendants.
// Calling job.close()/terminate() here would also kill this launcher and lose the native CLI's exit code.
process.exitCode = result.code ?? (result.signal ? 1 : 0);
