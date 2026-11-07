import { createHash, randomUUID } from 'node:crypto';
import { accessSync, constants, lstatSync, mkdirSync, readFileSync, realpathSync, renameSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { applyEdits, getNodeValue, modify, parseTree, type Node, type ParseError } from 'jsonc-parser';
import { ModelRuntime } from '@earendil-works/pi-coding-agent';

export class PiModelConfigError extends Error {}
/** Config inspection must not read, create, refresh or mutate credentials. */
export function createConfigRuntime(modelsPath: string | null) {
  return ModelRuntime.create({ modelsPath, refreshOnCreate: false, credentials: {
    async read() { return undefined; },
    async list() { return []; },
    async modify() { throw new PiModelConfigError('配置校验不能修改凭据'); },
    async delete() { throw new PiModelConfigError('配置校验不能删除凭据'); },
  } });
}
export type ObjectValue = Record<string, unknown>;
export function object(value: unknown): ObjectValue {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new PiModelConfigError('配置应为 JSON 对象');
  return value as ObjectValue;
}
function uniqueKeys(node: Node) {
  if (node.type === 'object') {
    const keys = node.children!.map(property => property.children![0]!.value);
    if (new Set(keys).size !== keys.length) throw new PiModelConfigError('配置包含重复字段，请先在 Pi 配置文件中消除重复项');
  }
  for (const child of node.children ?? []) uniqueKeys(child);
}
export function parseConfig(text: string): ObjectValue {
  const errors: ParseError[] = [];
  const tree = parseTree(text.replace(/^\uFEFF/, ' '), errors, { allowTrailingComma: true });
  if (errors.length || !tree) throw new PiModelConfigError(`models.json 无法解析${errors[0] ? `（字符位置 ${errors[0].offset}）` : ''}，原文件未改动`);
  uniqueKeys(tree);
  const config = object(getNodeValue(tree));
  object(config.providers);
  return config;
}
function targetPath(path: string): string {
  try { return realpathSync(path); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    if (lstatSync(path, { throwIfNoEntry: false })?.isSymbolicLink()) throw new PiModelConfigError('models.json 的链接目标不存在，请先修复链接');
    return resolve(path);
  }
}
export function readConfig(path: string) {
  const target = targetPath(path);
  let content: string | null = null;
  let mode = 0o600;
  try {
    const info = statSync(target);
    if (!info.isFile()) throw new PiModelConfigError('models.json 不是普通文件');
    mode = info.mode & 0o777;
    content = readFileSync(target, 'utf8');
  } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  const revision = createHash('sha256').update(target + '\0' + (content === null ? 'missing' : content)).digest('hex');
  const text = content ?? '{\n  "providers": {}\n}\n';
  return { target, content, text, mode, revision, config: parseConfig(text) };
}
export function edit(text: string, path: (string | number)[], value: unknown) {
  const bom = text.startsWith('\uFEFF') ? '\uFEFF' : '';
  const body = bom ? text.slice(1) : text;
  return bom + applyEdits(body, modify(body, path, value, { formattingOptions: { insertSpaces: true, tabSize: 2, eol: text.includes('\r\n') ? '\r\n' : '\n' } }));
}
/** Stage and validate without credentials, availability checks, key commands or network refresh. */
export async function commitConfig(path: string, revision: string, text: string) {
  let current = readConfig(path);
  if (current.revision !== revision) return false;
  parseConfig(text);
  mkdirSync(dirname(current.target), { recursive: true, mode: 0o700 });
  const temporary = join(dirname(current.target), `.models-${randomUUID()}.tmp`);
  try {
    writeFileSync(temporary, text, { flag: 'wx', mode: current.mode, flush: true });
    const runtime = await createConfigRuntime(temporary);
    if (runtime.getError()) throw new PiModelConfigError('配置未通过 Pi 校验，未保存。请检查接口类型、地址、模型参数以及现有高级配置。');
    // Re-read after asynchronous SDK validation; commit has no JS interleaving gap.
    current = readConfig(path);
    if (current.revision !== revision) return false;
    if (current.content !== null) accessSync(current.target, constants.W_OK);
    renameSync(temporary, current.target);
    return true;
  } finally {
    try { unlinkSync(temporary); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  }
}
