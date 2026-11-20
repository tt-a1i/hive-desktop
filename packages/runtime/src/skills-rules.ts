import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, join } from 'node:path';

export interface SkillOrRuleEntry {
  name: string;
  kind: 'rule' | 'skill';
  scope: 'project' | 'global';
  source: string;
  path: string;
  description?: string;
  sizeBytes: number;
  excerpt: string;
}

export interface SkillsAndRulesReport {
  checkedAt: number;
  rules: SkillOrRuleEntry[];
  skills: SkillOrRuleEntry[];
}

const MAX_READ_BYTES = 16_384;
const MAX_EXCERPT_CHARS = 360;

function readBoundedPrefix(filePath: string): { text: string; sizeBytes: number } | undefined {
  try {
    const st = statSync(filePath);
    if (!st.isFile()) return undefined;
    const raw = readFileSync(filePath, 'utf8');
    return {
      text: raw.length > MAX_READ_BYTES ? raw.slice(0, MAX_READ_BYTES) : raw,
      sizeBytes: st.size,
    };
  } catch {
    return undefined;
  }
}

function parseFrontmatter(text: string): { name?: string; description?: string; body: string } {
  const normalized = text.replace(/\r\n/g, '\n');
  if (!normalized.startsWith('---\n')) {
    return { body: normalized };
  }
  const end = normalized.indexOf('\n---', 4);
  if (end < 0) {
    return { body: normalized };
  }
  const header = normalized.slice(4, end);
  const body = normalized.slice(end + 4).trimStart();
  let name: string | undefined;
  let description: string | undefined;
  for (const rawLine of header.split('\n')) {
    const idx = rawLine.indexOf(':');
    if (idx < 0) continue;
    const key = rawLine.slice(0, idx).trim().toLowerCase();
    const val = rawLine.slice(idx + 1).trim().replace(/^["']|["']$/g, '');
    if (!val) continue;
    if (key === 'name' && !name) name = val;
    else if (key === 'description' && !description) description = val;
  }
  return {
    ...(name && { name }),
    ...(description && { description }),
    body,
  };
}

function buildExcerpt(body: string): string {
  const compact = body
    .split('\n')
    .map(l => l.trim())
    .filter(Boolean)
    .join(' ')
    .replace(/\s+/g, ' ');
  return compact.length > MAX_EXCERPT_CHARS
    ? `${compact.slice(0, MAX_EXCERPT_CHARS)}…`
    : compact;
}

function scanRuleFile(
  filePath: string,
  scope: 'project' | 'global',
  source: string,
  seen: Set<string>,
  out: SkillOrRuleEntry[],
): void {
  if (seen.has(filePath) || !existsSync(filePath)) return;
  const data = readBoundedPrefix(filePath);
  if (!data) return;
  seen.add(filePath);
  const { body } = parseFrontmatter(data.text);
  out.push({
    name: basename(filePath),
    kind: 'rule',
    scope,
    source,
    path: filePath,
    sizeBytes: data.sizeBytes,
    excerpt: buildExcerpt(body),
  });
}

function scanSkillsDirectory(
  dirPath: string,
  scope: 'project' | 'global',
  source: string,
  seen: Set<string>,
  out: SkillOrRuleEntry[],
): void {
  if (!existsSync(dirPath)) return;
  let entries;
  try {
    entries = readdirSync(dirPath, { withFileTypes: true });
  } catch {
    return;
  }
  for (const dirent of entries) {
    if (!dirent.isDirectory() && !dirent.isSymbolicLink()) continue;
    const skillFile = join(dirPath, dirent.name, 'SKILL.md');
    if (seen.has(skillFile) || !existsSync(skillFile)) continue;
    const data = readBoundedPrefix(skillFile);
    if (!data) continue;
    seen.add(skillFile);
    const parsed = parseFrontmatter(data.text);
    out.push({
      name: parsed.name ?? dirent.name,
      kind: 'skill',
      scope,
      source,
      path: skillFile,
      ...(parsed.description && { description: parsed.description }),
      sizeBytes: data.sizeBytes,
      excerpt: buildExcerpt(parsed.body),
    });
  }
}

export function collectSkillsAndRules(options: {
  workspacePath?: string;
  homeDir?: string;
} = {}): SkillsAndRulesReport {
  const home = options.homeDir ?? homedir();
  const rules: SkillOrRuleEntry[] = [];
  const skills: SkillOrRuleEntry[] = [];
  const seenRules = new Set<string>();
  const seenSkills = new Set<string>();

  if (options.workspacePath) {
    const ws = options.workspacePath;
    scanRuleFile(join(ws, 'AGENTS.md'), 'project', 'project', seenRules, rules);
    scanRuleFile(join(ws, 'CLAUDE.md'), 'project', 'claude', seenRules, rules);
    scanRuleFile(join(ws, '.cursorrules'), 'project', 'cursor', seenRules, rules);
    scanSkillsDirectory(join(ws, '.claude', 'skills'), 'project', 'claude', seenSkills, skills);
    scanSkillsDirectory(join(ws, '.agents', 'skills'), 'project', 'agents', seenSkills, skills);
    scanSkillsDirectory(join(ws, '.codex', 'skills'), 'project', 'codex', seenSkills, skills);
  }

  scanRuleFile(join(home, '.claude', 'CLAUDE.md'), 'global', 'claude', seenRules, rules);
  scanRuleFile(join(home, '.codex', 'AGENTS.md'), 'global', 'codex', seenRules, rules);
  scanSkillsDirectory(join(home, '.claude', 'skills'), 'global', 'claude', seenSkills, skills);
  scanSkillsDirectory(join(home, '.agents', 'skills'), 'global', 'agents', seenSkills, skills);
  scanSkillsDirectory(join(home, '.codex', 'skills'), 'global', 'codex', seenSkills, skills);
  scanSkillsDirectory(join(home, '.gemini', 'config', 'skills'), 'global', 'gemini', seenSkills, skills);

  return {
    checkedAt: Date.now(),
    rules,
    skills: skills.slice(0, 100),
  };
}
