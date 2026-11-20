import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { AgentProfileRef } from './agent-profiles.js';
import type { ChatBackend, ChatRecord } from './chat-index.js';

type Model = NonNullable<ChatRecord['model']>;

/** Desktop-local preferences, keyed independently so projects can update in parallel. */
export class ProjectModelDefaults {
  private readonly directory: string;
  constructor(dataDir: string) {
    this.directory = join(dataDir, 'project-model-defaults');
  }

  private file(workspaceId: string, backend: ChatBackend, profile: AgentProfileRef) {
    const key = JSON.stringify([workspaceId, backend, profile.id, profile.revision]);
    return join(this.directory, `${createHash('sha256').update(key).digest('hex')}.json`);
  }

  get(workspaceId: string, backend: ChatBackend, profile: AgentProfileRef): Model | null {
    let raw: string;
    try { raw = readFileSync(this.file(workspaceId, backend, profile), 'utf8'); }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
      throw error;
    }
    const model = JSON.parse(raw) as Partial<Model> | null;
    if (!model || typeof model.provider !== 'string' || !model.provider
        || typeof model.id !== 'string' || !model.id || typeof model.name !== 'string') {
      throw new Error('项目默认模型设置无法读取。');
    }
    return {provider: model.provider, id: model.id, name: model.name};
  }

  set(workspaceId: string, backend: ChatBackend, profile: AgentProfileRef, model: Model | null) {
    const file = this.file(workspaceId, backend, profile);
    if (!model) {
      rmSync(file, {force: true});
      return;
    }
    mkdirSync(this.directory, {recursive: true, mode: 0o700});
    const temporary = join(this.directory, `${randomUUID()}.tmp`);
    try {
      writeFileSync(temporary, JSON.stringify(model), {flag: 'wx', mode: 0o600, flush: true});
      renameSync(temporary, file);
    } finally {
      rmSync(temporary, {force: true});
    }
  }
}
