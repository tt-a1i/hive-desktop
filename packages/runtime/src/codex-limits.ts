import { CodexTransport } from './agents/codex-transport.js';

export type CodexLimitWindow = { position: 'primary' | 'secondary'; usedPercent: number; windowDurationMins: number | null; resetsAt: string | null };
export type CodexLimits = { status: 'ok' | 'unavailable' | 'not_applicable' | 'signed_out' | 'failed' | 'cancelled'; checkedAt: string; ordinaryUsageAllowed: boolean | null; windows: CodexLimitWindow[] };
const object = (value: unknown): Record<string, unknown> | undefined => value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
/** The returned value is a strict whitelist; account identifiers, credit data and native errors never cross this boundary. */
export function parseCodexLimits(value: unknown, checkedAt: string): CodexLimits {
  const response = object(value);
  if (!response) return {status:'failed',checkedAt,ordinaryUsageAllowed:null,windows:[]};
  const ordinaryUsageAllowed = typeof response.ordinaryUsageAllowed === 'boolean' ? response.ordinaryUsageAllowed : null;
  const buckets = object(response.rateLimitsByLimitId);
  const fallback = object(response.rateLimits);
  const bucket = buckets && 'codex' in buckets ? object(buckets.codex) : fallback && (fallback.limitId == null || fallback.limitId === 'codex') ? fallback : undefined;
  if (!bucket || (bucket.limitId != null && bucket.limitId !== 'codex')) return {status:'unavailable',checkedAt,ordinaryUsageAllowed,windows:[]};
  const windows: CodexLimitWindow[] = [];
  for (const position of ['primary','secondary'] as const) {
    const item = object(bucket[position]);
    if (!item || typeof item.usedPercent !== 'number' || !Number.isFinite(item.usedPercent) || item.usedPercent < 0 || item.usedPercent > 100) continue;
    const duration = typeof item.windowDurationMins === 'number' && Number.isFinite(item.windowDurationMins) && item.windowDurationMins > 0 ? item.windowDurationMins : null;
    const reset = typeof item.resetsAt === 'number' && Number.isFinite(item.resetsAt) && item.resetsAt > 0 && item.resetsAt <= 253402300799 ? item.resetsAt : null;
    windows.push({position,usedPercent:item.usedPercent,windowDurationMins:duration,resetsAt:reset === null ? null : new Date(reset * 1000).toISOString()});
  }
  return {status:windows.length || ordinaryUsageAllowed !== null ? 'ok' : 'unavailable',checkedAt,ordinaryUsageAllowed,windows};
}
export async function probeCodexLimits(command:string,signal:AbortSignal, launch: { args?: readonly string[]; env?: NodeJS.ProcessEnv } = {}):Promise<CodexLimits> {
  const empty = (status:CodexLimits['status']):CodexLimits => ({status,checkedAt:new Date().toISOString(),ordinaryUsageAllowed:null,windows:[]});
  if(signal.aborted)return empty('cancelled');
  let transport:CodexTransport|undefined;
  const starting=CodexTransport.start({...launch,readOnlyProbe:true,command,cwd:process.cwd(),requestTimeoutMs:5000,shutdownGraceMs:200,signal});
  const abort=()=>{void starting.then(value=>value.close(),()=>{}).catch(()=>{});};
  signal.addEventListener('abort',abort,{once:true});
  try {
    transport=await starting; signal.throwIfAborted();
    const account=object(await transport.sendRequest('account/read',{refreshToken:false})); signal.throwIfAborted();
    if(!account || typeof account.requiresOpenaiAuth !== 'boolean') return empty('failed');
    if(account.account == null) return empty(account.requiresOpenaiAuth ? 'signed_out' : 'not_applicable');
    const kind = object(account.account)?.type;
    if(kind !== 'chatgpt') return empty(kind === 'apiKey' || kind === 'amazonBedrock' || kind === 'gcp' ? 'not_applicable' : 'unavailable');
    const limits=await transport.sendRequest('account/rateLimits/read',{excludeResetCreditDetails:true}); signal.throwIfAborted();
    return parseCodexLimits(limits,new Date().toISOString());
  } catch { return empty(signal.aborted?'cancelled':'failed'); }
  finally { signal.removeEventListener('abort',abort); if(transport) { try { await transport.close(); } catch { return empty('failed'); } } }
}
