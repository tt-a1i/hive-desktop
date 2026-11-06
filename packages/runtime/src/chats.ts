import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createPiSession } from './agents/pi.js';
import type { PiEvent, PiSessionHandle } from './agents/types.js';
import { ChatIndex, type ChatRecord } from './chat-index.js';
import type { HiveHost } from './hive-host.js';

interface LiveChat { handle: PiSessionHandle; running?: Promise<void> }
interface Workspace { id: string; path: string }

export class Chats {
  private readonly index: ChatIndex;
  private readonly live = new Map<string, LiveChat>();
  private readonly opening = new Map<string, Promise<LiveChat>>();
  private readonly sequences = new Map<string, number>();
  private readonly sessionDir: string;
  private closing = false;

  constructor(private readonly hive: HiveHost, dataDir: string, private readonly emit: (event: PiEvent & { sequence: number }) => void) {
    const directory = join(dataDir, 'chats');
    this.sessionDir = join(directory, 'pi');
    mkdirSync(this.sessionDir, { recursive: true });
    this.index = new ChatIndex(directory);
  }
  list(workspaceId: string) { return this.index.list(workspaceId); }

  private async workspace(id: string): Promise<Workspace> {
    const workspaces = await this.hive.request('GET', '/api/workspaces') as Workspace[];
    const workspace = workspaces.find(workspace => workspace.id === id);
    if (!workspace) throw new Error('Workspace not found');
    return workspace;
  }
  private async load(record: ChatRecord): Promise<LiveChat> {
    const workspace = await this.workspace(record.workspaceId);
    const handle = await createPiSession({
      cwd: workspace.path, sessionDir: this.sessionDir,
      ...(record.sessionFile && existsSync(record.sessionFile) ? { sessionFile: record.sessionFile } : {}),
      ...(record.model ? { model: { provider: record.model.provider, id: record.model.id } } : {}),
      onEvent: event => {
        const sequence = (this.sequences.get(record.id) ?? 0) + 1;
        this.sequences.set(record.id, sequence);
        this.emit({ ...event, sessionId: record.id, sequence });
      },
    });
    try {
      const model = handle.currentModel();
      this.index.save({ ...record, sessionFile: handle.sessionFile ?? null, ...(model ? { model } : {}) });
      const live = { handle };
      this.live.set(record.id, live);
      return live;
    } catch (error) { handle.dispose(); throw error; }
  }
  private snapshot(id: string, chat: LiveChat) {
    return {
      ...this.index.get(id), ...chat.handle.snapshot(), models: chat.handle.listModels(),
      busy: !!chat.running, eventSequence: this.sequences.get(id) ?? 0,
    };
  }
  private async require(id: string): Promise<LiveChat> {
    if (this.closing) throw new Error('Chat runtime is closing');
    const existing = this.live.get(id);
    if (existing) return existing;
    const pending = this.opening.get(id);
    if (pending) return pending;
    const opening = this.load(this.index.get(id)).finally(() => this.opening.delete(id));
    this.opening.set(id, opening);
    return opening;
  }
  async create(workspaceId: string) {
    if (this.closing) throw new Error('Chat runtime is closing');
    await this.workspace(workspaceId);
    if (this.closing) throw new Error('Chat runtime is closing');
    const record: ChatRecord = {
      id: randomUUID(), workspaceId, title: '新会话', sessionFile: null, updatedAt: Date.now(),
    };
    // Register the pending creation so shutdown waits for SDK initialization.
    const opening = this.load(record).finally(() => this.opening.delete(record.id));
    this.opening.set(record.id, opening);
    const chat = await opening;
    return this.snapshot(record.id, chat);
  }
  async open(id: string) {
    const chat = await this.require(id);
    return this.snapshot(id, chat);
  }
  async prompt(id: string, text: string) {
    if (!text.trim()) throw new Error('Message cannot be empty');
    const chat = await this.require(id);
    if (chat.running) throw new Error('This conversation is already running');
    const record = this.index.get(id);
    this.index.save({ ...record, title: record.title === '新会话' ? text.trim().slice(0, 60) : record.title, updatedAt: Date.now() });
    // Acceptance and completion are separate: RPC returns immediately; Pi events
    // deliver text/tools and settled. A settled turn does not settle a Hive task.
    chat.running = chat.handle.prompt(text).catch(() => {
      // The adapter emits the original error into this conversation.
    }).finally(() => { delete chat.running; });
    return { accepted: true, session: this.index.get(id) };
  }
  async abort(id: string) {
    const chat = await this.require(id);
    await chat.handle.abort();
    await chat.running;
    return { ok: true };
  }
  async setModel(id: string, provider: string, modelId: string) {
    const chat = await this.require(id);
    if (chat.running) throw new Error('Stop the current response before changing model');
    await chat.handle.setModel(provider, modelId);
    const model = chat.handle.currentModel();
    this.index.save({ ...this.index.get(id), ...(model ? { model } : {}), updatedAt: Date.now() });
    return { ok: true };
  }
  async close() {
    this.closing = true;
    await Promise.allSettled(this.opening.values());
    await Promise.all([...this.live.values()].map(async chat => {
      try { await chat.handle.abort(); await chat.running; }
      finally { chat.handle.dispose(); }
    }));
    this.live.clear();
  }
}
