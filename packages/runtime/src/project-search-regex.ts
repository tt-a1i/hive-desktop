import { Worker } from 'node:worker_threads';
import type { RegexRequest, RegexResult, RegexScan } from './project-search-regex-worker.js';

export const REGEX_INVALID = 'Invalid regular expression.';
export const REGEX_TIMEOUT = 'Regular expression search timed out.';
export const REGEX_UNAVAILABLE = 'Regular expression search is temporarily unavailable.';
const COMPILE_MS = 1000;
const FILE_MS = 300;
const TOTAL_MS = 4000;

type WorkerReply = {kind: 'ready' | 'invalid' | 'failed'} | RegexResult;
type Slot = {resolve: (result: RegexResult) => void; reject: (reason: unknown) => void; timer: NodeJS.Timeout};

let live: ProjectRegexSearch | undefined;
let latest = 0;
const cancelled = () => new DOMException('Project file search cancelled', 'AbortError');

export async function startProjectRegexSearch(
  query: string, caseSensitive: boolean, wordClass: string | undefined, rangeLimit: number, signal?: AbortSignal,
  fileBudgetMs = FILE_MS,
) {
  signal?.throwIfAborted();
  const generation = ++latest;
  if (live) await live.close(cancelled());
  signal?.throwIfAborted();
  if (generation !== latest) throw cancelled();

  let search: ProjectRegexSearch;
  try {
    search = new ProjectRegexSearch(query, caseSensitive, wordClass, rangeLimit, signal, Math.min(fileBudgetMs, FILE_MS));
  } catch {
    throw new Error(REGEX_UNAVAILABLE);
  }
  live = search;
  try {
    await search.ready;
    return search;
  } catch (error) {
    await search.close(error);
    throw error;
  }
}

class ProjectRegexSearch {
  readonly ready: Promise<void>;
  readonly #worker: Worker;
  readonly #exit: Promise<number>;
  readonly #fileBudgetMs: number;
  readonly #cancel = new AbortController();
  readonly #removeAbort: () => void;
  readonly #totalTimer: NodeJS.Timeout;
  #compileTimer: NodeJS.Timeout | undefined;
  #pending: Slot | undefined;
  #closing: Promise<number> | undefined;

  get signal(): AbortSignal { return this.#cancel.signal; }

  constructor(query: string, caseSensitive: boolean, wordClass: string | undefined, rangeLimit: number, signal: AbortSignal | undefined, fileBudgetMs: number) {
    // Node 22 can load this self-contained TS entry in source-mode checks;
    // production loads the compiled sibling from the bundled dist directory.
    const workerUrl = new URL(import.meta.url.endsWith('.ts')
      ? './project-search-regex-worker.ts' : './project-search-regex-worker.js', import.meta.url);
    this.#worker = new Worker(workerUrl, {
      execArgv: [], workerData: {query, caseSensitive, wordClass, rangeLimit},
      resourceLimits: {maxOldGenerationSizeMb: 32, maxYoungGenerationSizeMb: 8, stackSizeMb: 2},
    });
    this.#fileBudgetMs = fileBudgetMs;
    this.#exit = new Promise(resolve => this.#worker.once('exit', resolve));
    const onAbort = () => { void this.close(signal?.reason ?? cancelled()); };
    signal?.addEventListener('abort', onAbort, {once: true});
    this.#removeAbort = () => signal?.removeEventListener('abort', onAbort);
    this.#totalTimer = setTimeout(() => { void this.close(new Error(REGEX_TIMEOUT)); }, TOTAL_MS);
    this.ready = new Promise((resolve, reject) => {
      this.#compileTimer = setTimeout(() => { void this.close(new Error(REGEX_TIMEOUT)); }, COMPILE_MS);
      const fail = () => reject(this.signal.reason);
      this.signal.addEventListener('abort', fail, {once: true});
      this.#worker.on('message', (reply: WorkerReply) => {
        if (this.signal.aborted) return;
        if (reply.kind === 'ready') {
          clearTimeout(this.#compileTimer);
          this.signal.removeEventListener('abort', fail);
          resolve();
        } else if (reply.kind === 'invalid') {
          void this.close(new Error(REGEX_INVALID));
        } else if (reply.kind === 'failed') {
          void this.close(new Error(REGEX_UNAVAILABLE));
        } else if ((reply.kind === 'scanned' || reply.kind === 'matchedPaths') && this.#pending) {
          const pending = this.#pending;
          this.#pending = undefined;
          clearTimeout(pending.timer);
          pending.resolve(reply);
        }
      });
      const unavailable = () => { void this.close(new Error(REGEX_UNAVAILABLE)); };
      this.#worker.on('error', unavailable);
      this.#worker.on('exit', unavailable);
    });
  }

  async scan(text: string, offset: number, limit: number): Promise<RegexScan> {
    const result = await this.#request({kind: 'scan', text, offset, limit});
    if (result.kind !== 'scanned') throw new Error(REGEX_UNAVAILABLE);
    return result;
  }

  async matchPaths(paths: readonly string[]): Promise<number[]> {
    const result = await this.#request({kind: 'matchPaths', paths});
    if (result.kind !== 'matchedPaths') throw new Error(REGEX_UNAVAILABLE);
    return result.indices;
  }

  async #request(request: RegexRequest): Promise<RegexResult> {
    this.signal.throwIfAborted();
    if (this.#pending) throw new Error(REGEX_UNAVAILABLE);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { void this.close(new Error(REGEX_TIMEOUT)); }, this.#fileBudgetMs);
      this.#pending = {resolve, reject, timer};
      try {
        this.#worker.postMessage(request);
      } catch {
        void this.close(new Error(REGEX_UNAVAILABLE));
      }
    });
  }

  close(reason: unknown = cancelled()): Promise<number> {
    if (this.#closing) return this.#closing;
    // Keep the slot occupied until Node reports that the worker exited.
    this.#closing = this.#exit.finally(() => {
      if (live === this) live = undefined;
    });
    void this.#worker.terminate().catch(() => {});
    this.#cancel.abort(reason);
    this.#removeAbort();
    clearTimeout(this.#totalTimer);
    clearTimeout(this.#compileTimer);
    if (this.#pending) {
      clearTimeout(this.#pending.timer);
      this.#pending.reject(reason);
      this.#pending = undefined;
    }
    return this.#closing;
  }
}
