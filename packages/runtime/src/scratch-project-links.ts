import { randomUUID } from 'node:crypto';
import { lstat, open, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';
import type { WorkspaceSummary } from './hive-runtime-contract.js';
import { isScratchId, scratchConversationPath } from './scratch-workspace.js';

export interface ScratchProjectLink {
  sourceWorkspaceId: string;
  sourceSessionId: string;
  targetWorkspaceId: string;
  path: string;
}

type Promotion = Omit<ScratchProjectLink, 'targetWorkspaceId'> & {name: string; targetWorkspaceId?: string};
export type ScratchProjectRequest = (method: string, path: string, body?: unknown) => Promise<unknown>;

function validPromotion(value: unknown): value is Promotion {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return isScratchId(record.sourceWorkspaceId) && isScratchId(record.sourceSessionId)
    && typeof record.name === 'string' && record.name.trim().length > 0
    && typeof record.path === 'string' && isAbsolute(record.path)
    && (record.targetWorkspaceId === undefined || isScratchId(record.targetWorkspaceId) && record.targetWorkspaceId !== record.sourceWorkspaceId);
}

/** The link does not move the original conversation or change its Hive workspace ownership. */
export class ScratchProjectLinks {
  private pending: Promise<void> = Promise.resolve();
  constructor(private readonly dataDir: string) {}

  serialize<T>(action: () => Promise<T>): Promise<T> {
    const task = this.pending.then(action, action);
    this.pending = task.then(() => {}, () => {});
    return task;
  }

  idle(): Promise<void> { return this.pending; }

  private async read(): Promise<Promotion[]> {
    const file = join(await realpath(this.dataDir), 'desktop-scratch-projects.json');
    try {
      const info = await lstat(file);
      if (!info.isFile()) throw new Error('Scratch project links must be stored in a regular file');
      const value: unknown = JSON.parse(await readFile(file, 'utf8'));
      if (!value || typeof value !== 'object' || !('version' in value) || value.version !== 1
        || !('promotions' in value) || !Array.isArray(value.promotions) || !value.promotions.every(validPromotion)) {
        throw new Error('Invalid saved Scratch project links');
      }
      const unique = new Set(value.promotions.map(item => item.sourceSessionId));
      if (unique.size !== value.promotions.length) throw new Error('Duplicate saved Scratch project links');
      return value.promotions;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [];
      throw new Error('无法读取保存的 Scratch 项目链接。', {cause: error});
    }
  }

  private async save(promotions: Promotion[]): Promise<void> {
    const root = await realpath(this.dataDir);
    const temporary = join(root, `desktop-scratch-projects-${randomUUID()}.tmp`);
    try {
      await writeFile(temporary, JSON.stringify({version: 1, promotions}), {flag: 'wx', mode: 0o600, flush: true});
      await rename(temporary, join(root, 'desktop-scratch-projects.json'));
      if (process.platform !== 'win32') {
        const directory = await open(root, 'r');
        try { await directory.sync(); } finally { await directory.close(); }
      }
    } finally { await rm(temporary, {force: true}); }
  }

  private findTarget(promotion: Promotion, workspaces: readonly WorkspaceSummary[]): WorkspaceSummary | undefined {
    return workspaces.find(item => item.path === promotion.path && isScratchId(item.id) && item.id !== promotion.sourceWorkspaceId
      && (promotion.targetWorkspaceId === undefined || item.id === promotion.targetWorkspaceId));
  }

  private matchesSource(promotion: Promotion, workspaces: readonly WorkspaceSummary[], hasSource: (workspaceId: string, sessionId: string) => boolean): boolean {
    const source = workspaces.find(item => item.id === promotion.sourceWorkspaceId);
    return !!source && hasSource(promotion.sourceWorkspaceId, promotion.sourceSessionId)
      && scratchConversationPath(source.path, promotion.sourceWorkspaceId, promotion.sourceSessionId) === promotion.path;
  }

  private link(promotion: Promotion, targetWorkspaceId: string): ScratchProjectLink {
    const {sourceWorkspaceId, sourceSessionId, path} = promotion;
    return {sourceWorkspaceId, sourceSessionId, targetWorkspaceId, path};
  }

  async list(workspaces: readonly WorkspaceSummary[], hasSource: (workspaceId: string, sessionId: string) => boolean,
    canRecover: (promotion: Promotion) => Promise<boolean>): Promise<ScratchProjectLink[]> {
    const promotions = await this.read();
    const links: ScratchProjectLink[] = [];
    let recovered = false;
    for (const promotion of promotions) {
      const target = this.findTarget(promotion, workspaces);
      if (!target || !this.matchesSource(promotion, workspaces, hasSource)) continue;
      if (promotion.targetWorkspaceId === undefined) {
        // An explicitly requested creation may already have committed before the sidecar exited.
        if (!await canRecover(promotion)) continue;
        promotion.targetWorkspaceId = target.id;
        recovered = true;
      }
      links.push(this.link(promotion, target.id));
    }
    if (recovered) await this.save(promotions);
    return links;
  }

  async hasTarget(sourceWorkspaceId: string, workspaces: readonly WorkspaceSummary[], hasSource: (workspaceId: string, sessionId: string) => boolean): Promise<boolean> {
    return (await this.read()).some(item => item.sourceWorkspaceId === sourceWorkspaceId
      && this.findTarget(item, workspaces) && this.matchesSource(item, workspaces, hasSource));
  }

  async forget(workspaceId: string, removedPath: string): Promise<void> {
    const promotions = await this.read();
    const remaining = promotions.filter(item => item.sourceWorkspaceId !== workspaceId
      && item.targetWorkspaceId !== workspaceId && !(item.targetWorkspaceId === undefined && item.path === removedPath));
    if (remaining.length !== promotions.length) await this.save(remaining);
  }

  async promote(source: Omit<ScratchProjectLink, 'targetWorkspaceId'>, name: string, request: ScratchProjectRequest,
    verifySource: () => Promise<void>, assertTargetAvailable: (workspaceId: string) => void) {
    const workspaces = () => request('GET', '/api/workspaces') as Promise<WorkspaceSummary[]>;
    const promotions = await this.read();
    let promotion = promotions.find(item => item.sourceSessionId === source.sourceSessionId);
    const existingWorkspaces = await workspaces();
    if (promotion && (promotion.sourceWorkspaceId !== source.sourceWorkspaceId || promotion.path !== source.path)) {
      throw new Error('已保存的项目链接属于其他 Scratch 会话。');
    }
    if (promotion?.targetWorkspaceId !== undefined) {
      const workspace = this.findTarget(promotion, existingWorkspaces);
      if (workspace) {
        assertTargetAvailable(workspace.id);
        return {workspace, link: this.link(promotion, workspace.id)};
      }
      // A new user action may register a project after the previous one was removed.
      promotions.splice(promotions.indexOf(promotion), 1);
      promotion = undefined;
    }
    if (!promotion) {
      promotion = {...source, name};
      promotions.push(promotion);
      await this.save(promotions);
    }
    let workspace = this.findTarget(promotion, existingWorkspaces);
    if (!workspace) {
      try {
        workspace = await request('POST', '/api/workspaces', {path: promotion.path, name: promotion.name, autostart_orchestrator: false}) as WorkspaceSummary;
      } catch (error) {
        // Retry only if Hive confirms that this exact folder was registered.
        workspace = this.findTarget(promotion, await workspaces());
        if (!workspace) throw error;
      }
      if (workspace.path !== promotion.path || !isScratchId(workspace.id) || workspace.id === promotion.sourceWorkspaceId)
        throw new Error('Hive 返回的项目与 Scratch 目录不匹配。');
    }
    await verifySource();
    assertTargetAvailable(workspace.id);
    promotion.targetWorkspaceId = workspace.id;
    await this.save(promotions);
    return {workspace, link: this.link(promotion, workspace.id)};
  }
}
