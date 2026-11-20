import { createHash } from 'node:crypto';
import { realpath, stat } from 'node:fs/promises';
import { basename } from 'node:path';
import { defaultAgentProfileId, type AgentProfiles, type AgentProfileRef } from './agent-profiles.js';
import {
  importProjectRoot, isImportBackend, nativeImportHome, readRecentCodexProjects, scanImportSource, verifyImportSource,
  type ImportBackend, type NativeImportSession,
} from './native-import-source.js';

type Workspace = {id: string; name: string; path: string; controller_mode?: 'codex_app'};
type Project = {id: string; path: string; name: string; sessionCount: number; backends: ImportBackend[]; workspaceId?: string};
type PlannedProject = {project: Project; sessions: NativeImportSession[]};
type ImportResult = {id: string; path: string; workspace?: Workspace; imported: number; alreadyImported: number; skipped: number; error?: string};
type ImportCounts = Pick<ImportResult, 'imported' | 'alreadyImported' | 'skipped'>;

const maxProjects = 100;
const maxConfiguredHomes = 24;

function sameRevision(first: AgentProfileRef, second: AgentProfileRef): boolean {
  return first.id === second.id && first.revision === second.revision;
}

export class DesktopImport {
  private latest = new Map<string, PlannedProject>();
  private readonly aborted = new AbortController();
  private pending: Promise<unknown> = Promise.resolve();

  constructor(private readonly dataDir: string, private readonly profiles: AgentProfiles,
    private readonly requestHive: (method: string, path: string, body?: unknown) => Promise<unknown>,
    private readonly addSessions: (workspace: Workspace, sources: NativeImportSession[]) => Promise<ImportCounts>) {}

  close() {
    this.aborted.abort();
    return this.pending;
  }

  private serial<T>(operation: () => Promise<T>): Promise<T> {
    this.aborted.signal.throwIfAborted();
    const next = this.pending.then(() => {
      this.aborted.signal.throwIfAborted();
      return operation();
    });
    this.pending = next.catch(() => {});
    return next;
  }

  private async workspaces(): Promise<Map<string, Workspace>> {
    const workspaces = await this.requestHive('GET', '/api/workspaces') as Workspace[];
    const canonical = new Map<string, Workspace>();
    for (const workspace of workspaces) {
      try { canonical.set(await realpath(workspace.path), workspace); }
      catch (error) {
        if (!['ENOENT', 'ENOTDIR', 'EACCES', 'EPERM'].includes((error as NodeJS.ErrnoException).code ?? '')) throw error;
      }
    }
    return canonical;
  }

  discover() {
    return this.serial(async () => {
      this.latest.clear();
      const signal = this.aborted.signal;
      const dataDir = await realpath(this.dataDir);
      const workspaces = await this.workspaces();
      const {profiles} = await this.profiles.list();
      const enabled = profiles.filter(profile => profile.enabled && isImportBackend(profile.driver)).sort((left, right) => {
        const priority = (profile: typeof left) => Number(profile.id !== defaultAgentProfileId(profile.driver));
        return priority(left) - priority(right) || left.id.localeCompare(right.id);
      });
      const seenHomes = new Set<string>();
      const conversations: NativeImportSession[] = [];
      const recentFolders = new Map<string, number>();
      let limited = false;
      const now = Date.now();
      for (const entry of enabled) {
        signal.throwIfAborted();
        const profile = await this.profiles.getRef({id:entry.id, revision:entry.currentRevision}, entry.driver);
        let home: string;
        try { home = await realpath(nativeImportHome(profile)); }
        catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'ENOENT') limited = true;
          continue;
        }
        const key = JSON.stringify([entry.driver, home]);
        if (seenHomes.has(key)) continue;
        if (seenHomes.size === maxConfiguredHomes) { limited = true; break; }
        seenHomes.add(key);
        const scan = await scanImportSource(profile, home, dataDir, signal, now);
        limited ||= scan.limited;
        conversations.push(...scan.sessions);
        if (entry.driver === 'codex') {
          const recent = await readRecentCodexProjects(home, dataDir, signal);
          limited ||= recent.limited;
          for (const project of recent.projects)
            recentFolders.set(project.path, Math.max(project.updatedAt, recentFolders.get(project.path) ?? 0));
        }
      }
      const byPath = new Map<string, NativeImportSession[]>();
      const counts = new Map<string, number>();
      const identity = (source: NativeImportSession) => JSON.stringify([source.backend, source.id]);
      for (const source of conversations) counts.set(identity(source), (counts.get(identity(source)) ?? 0) + 1);
      conversations.sort((left, right) => right.updatedAt - left.updatedAt || left.path.localeCompare(right.path));
      const projectPaths = new Map<string, string>();
      for (const source of conversations) {
        signal.throwIfAborted();
        // Multiple native files asserting the same identity are ambiguous.
        if (counts.get(identity(source)) !== 1) { limited = true; continue; }
        try {
          if (!projectPaths.has(source.cwd)) projectPaths.set(source.cwd,
            workspaces.has(source.cwd) ? source.cwd : await importProjectRoot(source.cwd));
        } catch { limited = true; continue; }
        const path = projectPaths.get(source.cwd)!;
        const group = byPath.get(path);
        if (group) group.push(source);
        else byPath.set(path, [source]);
        recentFolders.set(path, Math.max(source.updatedAt, recentFolders.get(path) ?? 0));
      }
      if (recentFolders.size > maxProjects) limited = true;
      const recent = [...recentFolders].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
      for (const [path] of recent.slice(0, maxProjects)) {
        const sessions = byPath.get(path) ?? [];
        const workspace = workspaces.get(path);
        const id = createHash('sha256').update(JSON.stringify([path,
          sessions.map(source => JSON.stringify([source.backend, source.id, source.path, source.profile])).sort(),
        ])).digest('hex');
        this.latest.set(id, {project:{id, path, name:workspace?.name ?? (basename(path) || path),
          sessionCount:sessions.length, backends:[...new Set(sessions.map(source => source.backend))],
          ...(workspace && {workspaceId:workspace.id})}, sessions});
      }
      return {projects:[...this.latest.values()].map(option => option.project), limited};
    });
  }

  apply(projectIds: unknown) {
    if (!Array.isArray(projectIds) || projectIds.length === 0 || projectIds.length > maxProjects
      || !projectIds.every(id => typeof id === 'string' && /^[a-f0-9]{64}$/.test(id))
      || new Set(projectIds).size !== projectIds.length)
      throw new TypeError('Select distinct projects from the latest discovery');
    const ids = [...projectIds] as string[];
    return this.serial(async () => {
      const selected = ids.map(id => {
        const option = this.latest.get(id);
        if (!option) throw new Error('Import selection expired. Scan for projects again.');
        return option;
      });
      const results: ImportResult[] = [];
      const workspaces = await this.workspaces();
      for (const {project, sessions} of selected) {
        this.aborted.signal.throwIfAborted();
        const result: ImportResult = {id:project.id, path:project.path, imported:0, alreadyImported:0, skipped:0};
        let workspace = workspaces.get(project.path);
        if (workspace) result.workspace = workspace;
        try {
          if (await realpath(project.path) !== project.path || !(await stat(project.path)).isDirectory())
            throw new Error('The discovered project directory is unavailable or changed');
          const usable: NativeImportSession[] = [];
          const profileErrors = new Map<string, string>();
          const nativeHomes = new Map<string, string>();
          for (const source of sessions) {
            this.aborted.signal.throwIfAborted();
            const key = JSON.stringify(source.profile);
            if (!nativeHomes.has(key) && !profileErrors.has(key)) {
              try {
                const current = await this.profiles.currentRef(source.profile.id);
                if (!sameRevision(current, source.profile)) throw new Error('A native profile changed after discovery');
                const profile = await this.profiles.resolveForExecution(source.profile, source.backend);
                nativeHomes.set(key, nativeImportHome(profile));
              } catch { profileErrors.set(key, 'A native profile changed or became unavailable'); }
            }
            try {
              if (profileErrors.has(key)) throw new Error(profileErrors.get(key));
              await verifyImportSource(source, source.backend, source.id, nativeHomes.get(key)!, source.cwd);
              if (source.cwd !== project.path && await importProjectRoot(source.cwd) !== project.path)
                throw new Error('The native conversation project changed');
              usable.push(source);
            } catch { result.skipped++; }
          }
          if (result.skipped) result.error = 'Some discovered conversations or profiles changed or became unavailable. Scan again to retry them.';
          if (usable.length || sessions.length === 0) {
            this.aborted.signal.throwIfAborted();
            if (!workspace) {
              try {
                workspace = await this.requestHive('POST', '/api/workspaces', {
                  path:project.path, name:project.name, autostart_orchestrator:false,
                }) as Workspace;
              } catch (error) {
                // Another workflow can create this workspace, or its creation
                // may have committed before the response was interrupted.
                workspace = (await this.workspaces()).get(project.path);
                if (!workspace) throw error;
              }
              workspaces.set(project.path, workspace);
              result.workspace = workspace;
            }
            const counts = await this.addSessions(workspace, usable);
            result.imported = counts.imported;
            result.alreadyImported = counts.alreadyImported;
            result.skipped += counts.skipped;
            if (counts.skipped) result.error ??= 'Some conversations cannot be added to this workspace.';
          }
        } catch (error) {
          this.aborted.signal.throwIfAborted();
          result.error = error instanceof Error ? error.message : String(error);
          result.skipped = sessions.length - result.imported - result.alreadyImported;
        }
        results.push(result);
      }
      return {results};
    });
  }
}
