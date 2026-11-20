import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { collectSkillsAndRules } from '../src/skills-rules.js';
import { collectLocalUsageSummary } from '../src/usage-summary.js';
import type { ChatRecord } from '../src/chat-index.js';

const root = mkdtempSync(join(tmpdir(), 'hive-skills-usage-'));
const workspace = join(root, 'workspace');
const home = join(root, 'home');

try {
  // 1. Skills & rules discovery
  mkdirSync(join(workspace, '.claude', 'skills', 'code-review'), { recursive: true });
  mkdirSync(join(home, '.agents', 'skills', 'perf-audit'), { recursive: true });
  mkdirSync(join(home, '.claude'), { recursive: true });

  writeFileSync(join(workspace, 'AGENTS.md'), '# Project Rules\nAlways run zero-warning builds.\n', 'utf8');
  writeFileSync(join(home, '.claude', 'CLAUDE.md'), '# Global Claude Rule\nPrefer concise responses.\n', 'utf8');
  writeFileSync(
    join(workspace, '.claude', 'skills', 'code-review', 'SKILL.md'),
    '---\nname: code-review-local\ndescription: Review diffs carefully\n---\nCheck edge cases and tests.\n',
    'utf8',
  );
  writeFileSync(
    join(home, '.agents', 'skills', 'perf-audit', 'SKILL.md'),
    '---\nname: perf-audit\ndescription: Measure RSS and latency\n---\nProfile memory before optimizing.\n',
    'utf8',
  );

  const report = collectSkillsAndRules({ workspacePath: workspace, homeDir: home });
  assert.ok(report.rules.some(r => r.name === 'AGENTS.md' && r.scope === 'project' && r.excerpt.includes('zero-warning')));
  assert.ok(report.rules.some(r => r.name === 'CLAUDE.md' && r.scope === 'global'));
  const localSkill = report.skills.find(s => s.name === 'code-review-local');
  assert.ok(localSkill);
  assert.equal(localSkill.scope, 'project');
  assert.equal(localSkill.description, 'Review diffs carefully');
  assert.ok(report.skills.some(s => s.name === 'perf-audit' && s.scope === 'global'));

  // 2. Local multi-backend usage summary (zero model calls)
  const piFile = join(root, 'pi-session.jsonl');
  writeFileSync(
    piFile,
    [
      JSON.stringify({ type: 'session', id: 'pi-u1', timestamp: new Date().toISOString() }),
      JSON.stringify({
        type: 'message',
        id: 'm1',
        parentId: null,
        message: { role: 'assistant', content: [], usage: { input: 1200, output: 450, cacheRead: 800 } },
      }),
      JSON.stringify({
        type: 'message',
        id: 'm2',
        parentId: 'm1',
        message: { role: 'assistant', content: [], usage: { input: 300, output: 150, cacheRead: 200 } },
      }),
    ].join('\n') + '\n',
    'utf8',
  );

  const now = Date.now();
  const sessions: ChatRecord[] = [
    {
      id: 's-pi',
      workspaceId: 'w1',
      backend: 'pi',
      title: 'Pi chat',
      sessionFile: piFile,
      updatedAt: now - 1000,
    },
    {
      id: 's-codex',
      workspaceId: 'w1',
      backend: 'codex',
      title: 'Codex worktree',
      sessionFile: null,
      updatedAt: now - 2 * 24 * 60 * 60 * 1000,
      execution: { kind: 'worktree', path: workspace, branch: 'feat/wt', createdAt: now },
    },
    {
      id: 's-acp',
      workspaceId: 'w1',
      backend: 'acp',
      title: 'Old ACP',
      sessionFile: null,
      updatedAt: now - 20 * 24 * 60 * 60 * 1000,
      archived: true,
    },
  ];

  const summary = await collectLocalUsageSummary(sessions, now);
  assert.equal(summary.totalSessions, 3);
  assert.equal(summary.activeLast7Days, 2);
  assert.equal(summary.activeLast30Days, 3);
  assert.equal(summary.worktreeSessions, 1);
  assert.equal(summary.archivedSessions, 1);
  assert.equal(summary.sampledTranscripts, 1);
  assert.equal(summary.totalInputTokens, 1500);
  assert.equal(summary.totalOutputTokens, 600);
  assert.equal(summary.totalCacheReadTokens, 1000);
  assert.equal(summary.backends.find(b => b.backend === 'pi')?.totalSessions, 1);
  assert.equal(summary.backends.find(b => b.backend === 'codex')?.worktreeSessions, 1);

  console.log('SKILLS_AND_USAGE_OK: project/global rules & SKILL.md discovery and read-only multi-backend token/activity aggregation verified.');
} finally {
  rmSync(root, { recursive: true, force: true });
}
