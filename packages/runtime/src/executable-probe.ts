import { OwnedCliProcess } from './agents/owned-cli-process.js';

export type ExecutableProbeResult = {
  status: 'available' | 'not_found' | 'failed' | 'timeout' | 'cancelled';
  version?: string;
  message: string;
};
const VERSION = /^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)$/;
function versionOutput(stdout: string, stderr: string): string | undefined {
  const first = (stdout.trim() || stderr.trim()).split(/\r?\n/, 1)[0]?.trim() ?? '';
  const tokens = first.split(/\s+/);
  const token = tokens.length === 2 && ['pi', 'codex-cli'].includes(tokens[0]!) ? tokens[1] : tokens[0];
  return token?.match(VERSION)?.[1];
}

/** Only --version; retain the same Windows shim/Job ownership as actual CLI sessions. */
export async function probeExecutable(command: string, signal: AbortSignal): Promise<ExecutableProbeResult> {
  if (signal.aborted) return { status: 'cancelled', message: '检查已取消。' };
  const owned = new OwnedCliProcess(command, ['--version'], { cwd: process.cwd() });
  const child = owned.child;
  let stdout = '', stderr = '', bytes = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let onAbort: () => void = () => {};
  try {
    return await new Promise<ExecutableProbeResult>(resolve => {
      let settled = false;
      const finish = (result: ExecutableProbeResult) => { if (!settled) { settled = true; resolve(result); } };
      onAbort = () => finish({ status: 'cancelled', message: '检查已取消。' });
      signal.addEventListener('abort', onAbort, {once:true});
      timer = setTimeout(() => finish({status:'timeout',message:'程序没有在 5 秒内返回版本，已停止检查。'}), 5_000);
      const collect = (target: 'stdout' | 'stderr', chunk: Buffer) => {
        bytes += chunk.length;
        if (bytes > 64 * 1024) { finish({status:'failed',message:'程序输出过长，已停止检查。'}); return; }
        if (target === 'stdout') stdout += chunk.toString('utf8'); else stderr += chunk.toString('utf8');
      };
      child.stdout.on('data', chunk => collect('stdout', chunk));
      child.stderr.on('data', chunk => collect('stderr', chunk));
      child.stdin.on('error', error => {
        // A successful --version may close stdin before EOF; observe its exit below.
        if ((error as NodeJS.ErrnoException).code !== 'EPIPE') finish({status:'failed',message:'程序输入通道失败，已停止检查。'});
      });
      child.once('error', error => finish((error as NodeJS.ErrnoException).code === 'ENOENT'
        ? {status:'not_found',message:'找不到程序，请检查路径或安装对应 CLI。'}
        : {status:'failed',message:'无法运行程序，请检查文件权限和路径。'}));
      child.once('close', code => {
        if (code !== 0) { finish({status:'failed',message:'程序未能正常返回版本，请在终端检查该 CLI。'}); return; }
        const version = versionOutput(stdout, stderr);
        finish(version ? {status:'available',version,message:`已检测到版本 ${version}。`} : {status:'failed',message:'程序已运行，但没有返回可识别的版本。'});
      });
      child.stdin.end();
    });
  } finally {
    clearTimeout(timer);
    signal.removeEventListener('abort', onAbort);
    // Do not report a completed/cancelled probe until its owned process has closed.
    await owned.close(0);
  }
}
