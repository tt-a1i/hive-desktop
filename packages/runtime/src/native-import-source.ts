import { constants } from 'node:fs';
import { lstat, open, opendir, realpath, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, dirname, isAbsolute, join, relative, sep } from 'node:path';
import { getAgentDir } from '@earendil-works/pi-coding-agent';
import type { AgentProfileRef, ResolvedAgentProfile } from './agent-profiles.js';
import { resolveClaudeTranscript } from './agents/claude-transcript-source.js';
import { importedCodexMessage } from './agents/imported-codex-history.js';

export type ImportBackend = 'codex' | 'claude' | 'pi-rpc';
export interface NativeImportSource {
  home: string;
  path: string;
  cwd: string;
}
export interface NativeImportSession extends NativeImportSource {
  backend: ImportBackend;
  id: string;
  profile: AgentProfileRef;
  title: string;
  updatedAt: number;
}

const recentMillis = 90 * 24 * 60 * 60 * 1000;
const maxDirectoryEntries = 12_000;
const maxTranscriptCandidates = 512;
const maxPrefixBytes = 128 * 1024;
const maxCodexStateBytes = 4 * 1024 * 1024;
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i;
const safePiId = /^[a-zA-Z0-9_-]{8,128}$/;

function object(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}
function missing(error: unknown): boolean {
  return ['ENOENT', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '');
}
export function inside(parent: string, child: string): boolean {
  const name = relative(parent, child);
  return name !== '' && name !== '..' && !name.startsWith(`..${sep}`) && !isAbsolute(name);
}
export function isImportBackend(backend: string): backend is ImportBackend {
  return backend === 'codex' || backend === 'claude' || backend === 'pi-rpc';
}
export function nativeImportHome(profile: ResolvedAgentProfile): string {
  if (profile.launch.configDir) return profile.launch.configDir;
  if (profile.driver === 'codex') return process.env.CODEX_HOME || join(homedir(), '.codex');
  if (profile.driver === 'claude') return process.env.CLAUDE_CONFIG_DIR || join(homedir(), '.claude');
  return getAgentDir();
}
function transcriptsRoot(home: string, backend: ImportBackend): string {
  return join(home, backend === 'claude' ? 'projects' : 'sessions');
}

export async function importProjectRoot(cwd: string): Promise<string> {
  let current = cwd;
  for (let depth = 0; depth < 32; depth++) {
    try {
      const entry = await lstat(join(current, '.git'));
      if (entry.isFile() || entry.isDirectory()) return current;
    } catch (error) { if (!missing(error)) throw error; }
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return cwd;
}

export async function readRecentCodexProjects(home: string, ownDataDir: string, signal: AbortSignal) {
  const projects: {path:string; updatedAt:number}[] = [];
  const path = join(home, '.codex-global-state.json');
  let limited = false;
  try {
    if (await realpath(path) !== path) return {projects, limited};
    const file = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
    let state: Record<string, unknown> | undefined;
    try {
      const info = await file.stat();
      if (!info.isFile()) return {projects, limited};
      if (info.size > maxCodexStateBytes) return {projects, limited:true};
      const buffer = Buffer.alloc(info.size);
      const {bytesRead} = await file.read(buffer);
      if (bytesRead !== info.size) return {projects, limited:true};
      state = object(JSON.parse(buffer.toString('utf8')));
    } finally { await file.close(); }
    const local = Object.values(object(state?.['local-projects']) ?? {});
    limited = local.length > 512;
    for (const entry of local.slice(0, 512)) {
      signal.throwIfAborted();
      const project = object(entry);
      if (!Array.isArray(project?.rootPaths)) continue;
      limited ||= project.rootPaths.length > 16;
      const updatedAt = typeof project.updatedAt === 'number' && Number.isFinite(project.updatedAt) ? project.updatedAt : 0;
      for (const requested of project.rootPaths.slice(0, 16)) {
        signal.throwIfAborted();
        if (typeof requested !== 'string' || !isAbsolute(requested)) continue;
        try {
          const cwd = await realpath(requested);
          if (cwd === ownDataDir || inside(ownDataDir, cwd) || !(await stat(cwd)).isDirectory()) continue;
          projects.push({path:await importProjectRoot(cwd), updatedAt});
        } catch (error) { if (!missing(error)) limited = true; }
      }
    }
  } catch (error) {
    signal.throwIfAborted();
    if (!missing(error)) limited = true;
  }
  return {projects, limited};
}

async function* transcriptRows(path: string, bytes: number) {
  // Pin the open descriptor, read only complete JSONL records, and never
  // acquire a write handle. A native process can still append to the source.
  if (await realpath(path) !== path) throw new Error('The native transcript path changed');
  const descriptor = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
  try {
    const info = await descriptor.stat();
    if (!info.isFile()) throw new Error('The native transcript is not a regular file');
    const buffer = Buffer.alloc(Math.min(info.size, bytes));
    const { bytesRead } = await descriptor.read(buffer, 0, buffer.length, 0);
    const raw = buffer.toString('utf8', 0, bytesRead);
    const lastNewline = raw.lastIndexOf('\n');
    const prefix = bytesRead === info.size ? raw : raw.slice(0, lastNewline + 1);
    let index = 0;
    for (const line of prefix.split('\n')) {
      index++;
      if (!line.trim()) continue;
      try {
        const row = object(JSON.parse(line));
        if (row) yield { row, index };
      } catch { /* A malformed record cannot provide native identity or content. */ }
    }
  } finally {
    await descriptor.close();
  }
}

function messageText(content: unknown): string {
  if (typeof content === 'string') return content.trim();
  if (!Array.isArray(content)) return '';
  return content.map(object).filter(entry => entry?.type === 'text' || entry?.type === 'input_text' || entry?.type === 'output_text')
    .flatMap(entry => typeof entry?.text === 'string' ? [entry.text.trim()] : []).filter(Boolean).join('\n');
}
function title(text: string, backend: ImportBackend): string {
  const clean = text.replace(/[\p{Cc}\p{Cs}]/gu, ' ').replace(/\s+/g, ' ').trim();
  return Array.from(clean).slice(0, 60).join('') || ({codex:'Codex', claude:'Claude Code', 'pi-rpc':'Pi'}[backend]);
}
async function readIdentity(backend: ImportBackend, home: string, path: string) {
  const root = transcriptsRoot(home, backend);
  if (!inside(root, path)) return undefined;
  const fileName = basename(path);
  let id: string | undefined, cwd: string | undefined, firstUser = '', hasConversation = false;
  for await (const {row, index} of transcriptRows(path, maxPrefixBytes)) {
    if (backend === 'codex') {
      const payload = object(row.payload);
      if (index === 1 && row.type === 'session_meta' && typeof payload?.id === 'string' && uuid.test(payload.id)
        && typeof payload.cwd === 'string' && fileName.startsWith('rollout-') && fileName.endsWith(`-${payload.id}.jsonl`)
        && !object(payload.source)?.subagent) {
        id = payload.id; cwd = payload.cwd;
      }
      if (!id) return undefined;
      const message = importedCodexMessage(row);
      if (message) {
        hasConversation = true;
        if (message.role === 'user') firstUser = message.content;
      }
    } else if (backend === 'claude') {
      if ((row.type !== 'user' && row.type !== 'assistant') || row.isSidechain === true) continue;
      if (typeof row.sessionId !== 'string' || !uuid.test(row.sessionId) || fileName !== `${row.sessionId}.jsonl` || typeof row.cwd !== 'string'
        || typeof row.uuid !== 'string' || !uuid.test(row.uuid)) continue;
      id = row.sessionId; cwd = row.cwd; hasConversation = true;
      if (row.type === 'user') firstUser = messageText(object(row.message)?.content);
    } else {
      if (index === 1 && row.type === 'session' && typeof row.id === 'string' && safePiId.test(row.id)
        && typeof row.cwd === 'string' && fileName.endsWith(`_${row.id}.jsonl`)) {
        id = row.id; cwd = row.cwd;
      }
      if (!id) return undefined;
      const message = object(row.message);
      if (row.type === 'message' && (message?.role === 'user' || message?.role === 'assistant')) {
        hasConversation = true;
        if (message.role === 'user') firstUser = messageText(message.content);
      }
    }
    if (id && cwd && firstUser) break;
  }
  if (!id || !cwd || !isAbsolute(cwd) || !hasConversation) return undefined;
  const project = await realpath(cwd);
  if (backend === 'claude' && await resolveClaudeTranscript(id, project, {...process.env, CLAUDE_CONFIG_DIR:home}) !== path) return undefined;
  if (!(await stat(project)).isDirectory()) return undefined;
  return { id, cwd: project, title: title(firstUser, backend) };
}

interface Candidate {path: string; updatedAt: number; size: number}
export async function scanImportSource(profile: ResolvedAgentProfile, home: string, ownDataDir: string, signal: AbortSignal, now = Date.now()) {
  const backend = profile.driver;
  if (!isImportBackend(backend)) return {sessions: [], limited: false};
  const root = transcriptsRoot(home, backend);
  const cutoff = now - recentMillis;
  let remaining = maxDirectoryEntries, limited = false;
  const candidates: Candidate[] = [];
  const visit = async (directory: string, depth: number): Promise<void> => {
    signal.throwIfAborted();
    if (!remaining || inside(ownDataDir, directory) || directory === ownDataDir) return;
    try {
      if (await realpath(directory) !== directory) return;
      // Visit only the native layout. In particular, do not walk Claude
      // subagent directories or follow links out of a configured session home.
      for await (const entry of await opendir(directory)) {
        signal.throwIfAborted();
        if (remaining === 0) { limited = true; break; }
        remaining--;
        const path = join(directory, entry.name);
        if (entry.isDirectory() && depth > 0) {
          const part = depth === 3 ? /^\d{4}$/ : /^\d{2}$/;
          if (backend !== 'codex' || part.test(entry.name)) await visit(path, depth - 1);
        } else if (entry.isFile() && (depth === 0 || backend === 'pi-rpc') && entry.name.endsWith('.jsonl')) {
          try {
            const {mtimeMs, size} = await stat(path);
            if (mtimeMs >= cutoff) candidates.push({path, updatedAt: Math.trunc(mtimeMs), size});
          } catch (error) { if (!missing(error)) limited = true; }
        }
      }
    } catch (error) {
      signal.throwIfAborted();
      if (!missing(error)) limited = true;
    }
  };
  await visit(root, backend === 'codex' ? 3 : 1);
  candidates.sort((left, right) => right.updatedAt - left.updatedAt || left.path.localeCompare(right.path));
  if (candidates.length > maxTranscriptCandidates) limited = true;
  const sessions: NativeImportSession[] = [];
  for (const candidate of candidates.slice(0, maxTranscriptCandidates)) {
    signal.throwIfAborted();
    try {
      const identity = await readIdentity(backend, home, candidate.path);
      if (identity && identity.cwd !== ownDataDir && !inside(ownDataDir, identity.cwd))
        sessions.push({...identity, path:candidate.path, updatedAt:candidate.updatedAt, backend, profile:profile.ref, home});
      else if (!identity && candidate.size > maxPrefixBytes) limited = true;
    } catch (error) { if (!missing(error)) limited = true; }
  }
  return {sessions, limited};
}

export async function verifyImportSource(source: NativeImportSource, backend: ImportBackend, id: string, home: string, cwd: string) {
  if (await realpath(home) !== source.home || await realpath(cwd) !== source.cwd)
    throw new Error('The native conversation configuration or project directory changed');
  const identity = await readIdentity(backend, source.home, source.path);
  if (identity?.id !== id || identity.cwd !== source.cwd)
    throw new Error('The native transcript no longer matches its original conversation and project');
}
