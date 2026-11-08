/** Read native reports without synthesizing totals or treating missing values as zero. */
export interface UsageCounter { key:string; label:string; tokens:number }
export interface UsageReport { label:string; counters:UsageCounter[] }
export interface UsageSnapshot {
 source:string;
 reports:UsageReport[];
 context?:{tokens:number|null;window:number;percent:number|null;estimated:boolean};
 notice?:string;
}
function object(value:unknown):Record<string,unknown>{
 if(!value||typeof value!=='object'||Array.isArray(value))throw new TypeError('Agent 未返回有效的用量对象。');
 return value as Record<string,unknown>;
}
function count(value:unknown):number{
 if(typeof value!=='number'||!Number.isSafeInteger(value)||value<0)throw new TypeError('Agent 返回的 Token 计数无效。');
 return value;
}
function counters(value:unknown,fields:readonly (readonly [string,string])[]):UsageCounter[]{
 const raw=object(value);
 return fields.flatMap(([key,label])=>raw[key]===undefined?[]:[{key,label,tokens:count(raw[key])}]);
}
const piFields=[['input','输入（不含缓存）'],['output','输出'],['cacheRead','缓存读取'],['cacheWrite','缓存写入'],['total','原生总计']]as const;
const codexFields=[['inputTokens','输入（含缓存）'],['outputTokens','输出'],['cachedInputTokens','缓存输入（输入子集）'],['cacheWriteInputTokens','缓存写入'],['reasoningOutputTokens','推理输出（输出子集）'],['totalTokens','原生总计']]as const;
const claudeFields=[['input_tokens','输入（不含缓存）'],['output_tokens','输出'],['cache_read_input_tokens','缓存读取'],['cache_creation_input_tokens','缓存写入']]as const;
const claudeModelFields=[['inputTokens','输入（不含缓存）'],['outputTokens','输出'],['cacheReadInputTokens','缓存读取'],['cacheCreationInputTokens','缓存写入'],['thinkingTokens','思考（输出子集）'],['contextWindow','上下文窗口容量'],['maxOutputTokens','最大输出容量']]as const;
export function piUsage(value:unknown,source:string):UsageSnapshot{
 const raw=object(value);const reports=[{label:'原生会话累计',counters:counters(raw.tokens,piFields)}];
 const result:UsageSnapshot={source,reports};
 if(raw.contextUsage!==undefined){
  const context=object(raw.contextUsage);const window=count(context.contextWindow);
  if(window===0)throw new TypeError('Agent 返回的上下文窗口无效。');
  const tokens=context.tokens===null?null:count(context.tokens);
  const percent=context.percent;
  if(percent!==null&&(typeof percent!=='number'||!Number.isFinite(percent)||percent<0))throw new TypeError('Agent 返回的上下文比例无效。');
  result.context={tokens,window,percent:percent as number|null,estimated:true};
 }
 result.notice='上下文为 Pi 原生估算；压缩后尚无新回复时可能暂不可用。累计用量不等于当前上下文大小。';
 return result;
}
export function codexUsage(value:unknown):UsageSnapshot{
 const source='Codex 原生线程';
 if(value===undefined)return{source,reports:[],notice:'本次原生连接尚未返回线程用量；未提供的数据不会显示为零。'};
 const raw=object(value);const reports=[{label:'原生线程累计',counters:counters(raw.total,codexFields)},{label:'原生最近一次用量',counters:counters(raw.last,codexFields)}];
 if(raw.modelContextWindow!==null&&raw.modelContextWindow!==undefined)reports.push({label:'最近报告的模型容量',counters:[{key:'modelContextWindow',label:'窗口容量（最近报告）',tokens:count(raw.modelContextWindow)}]});
 return{source,reports,notice:'最近一次用量不是实时上下文占用；缓存输入和推理输出分别属于输入、输出的子集。重新连接后以原生通知为准。'};
}
export function claudeUsage(value:unknown):UsageSnapshot{
 const source='Claude 官方 SDK';
 if(value===undefined)return{source,reports:[],notice:'本次连接尚未收到原生回合统计；历史不会被重新计为零。'};
 const raw=object(value);const reports:UsageReport[]=[];
 if(raw.usage!==undefined)reports.push({label:'最近一轮主循环',counters:counters(raw.usage,claudeFields)});
 if(raw.modelUsage!==undefined)for(const[model,usage]of Object.entries(object(raw.modelUsage)))reports.push({label:`模型累计 · ${model}`,counters:counters(usage,claudeModelFields)});
 return{source,reports,notice:'主循环统计不含子 Agent 和辅助调用；模型累计取最新原生结果，不逐轮相加。原生报告有自身的恢复及重置边界，不是订阅额度或账单。'};
}
