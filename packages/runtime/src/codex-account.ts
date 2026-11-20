import {CodexTransport} from './agents/codex-transport.js';
import {posix, win32} from 'node:path';
import {terminalArgument, terminalCommand} from './terminal-command.js';

export type CodexAccountStatus =
  | {status:'account_detected';accountType:'chatgpt'|'apiKey'|'amazonBedrock'|'gcp'}
  | {status:'signed_out'|'not_required'|'unknown'|'failed'|'cancelled'};

/** Use the same inherited Codex home when pasting into a separate terminal. */
export function codexLoginCommand(command: string, codexHome: string | undefined, platform = process.platform): string | undefined {
  const path = platform === 'win32' ? win32 : posix;
  const hasPath = (platform === 'win32' ? /[\\/]/ : /\//).test(command);
  if (platform === 'win32' && /^[\\/](?![\\/])|^[a-z]:(?![\\/])/i.test(command)) return undefined;
  const executable = hasPath && !path.isAbsolute(command)
    ? `${process.cwd()}${path.sep}${command}`
    : command;
  const argv = [executable, 'login'];
  const invocation = terminalCommand(argv, platform);
  if (!codexHome) return invocation;
  if (!path.isAbsolute(codexHome) || /[\x00-\x1f\x7f]/.test(codexHome)) return undefined;
  if (platform === 'win32' && /^[\\/](?![\\/])/.test(codexHome)) return undefined;
  const home = terminalArgument(codexHome, platform);
  if (platform !== 'win32') return `CODEX_HOME=${home} ${invocation}`;
  return `& { $previous = $env:CODEX_HOME; try { $env:CODEX_HOME = ${home}; ${invocation} } finally { $env:CODEX_HOME = $previous } }`;
}

/** Read native account metadata without a token refresh, login, thread or model turn.
 * Return only the closed status/type set; never expose account IDs, email or tokens.
 */
export async function probeCodexAccount(command:string, signal:AbortSignal, launch: { args?: readonly string[]; env?: NodeJS.ProcessEnv } = {}):Promise<CodexAccountStatus> {
  if(signal.aborted) return {status:'cancelled'};
  let transport:CodexTransport|undefined;
  const starting=CodexTransport.start({...launch,readOnlyProbe:true,command,cwd:process.cwd(),requestTimeoutMs:5_000,shutdownGraceMs:200,signal});
  const abort=()=>{void starting.then(value=>value.close(),()=>{}).catch(()=>{});};
  signal.addEventListener('abort',abort,{once:true});
  try {
    transport=await starting;
    signal.throwIfAborted();
    const value=await transport.sendRequest<unknown>('account/read',{refreshToken:false});
    signal.throwIfAborted();
    if(!value || typeof value!=='object' || Array.isArray(value)) return {status:'failed'};
    const response=value as {requiresOpenaiAuth?:unknown;account?:unknown};
    if(typeof response.requiresOpenaiAuth!=='boolean') return {status:'failed'};
    if(response.account==null) return {status:response.requiresOpenaiAuth?'signed_out':'not_required'};
    if(typeof response.account!=='object' || Array.isArray(response.account)) return {status:'failed'};
    const type=(response.account as {type?:unknown}).type;
    if(type==='chatgpt'||type==='apiKey'||type==='amazonBedrock'||type==='gcp') return {status:'account_detected',accountType:type};
    return {status:'unknown'};
  } catch {
    // Native transport errors may contain diagnostics with account data. The UI
    // receives only a status and can suggest checking the configured CLI locally.
    return {status:signal.aborted?'cancelled':'failed'};
  } finally {
    signal.removeEventListener('abort',abort);
    if(transport) await transport.close();
  }
}
