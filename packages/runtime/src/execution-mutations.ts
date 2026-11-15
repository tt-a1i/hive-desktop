/** Process-local admission for application-owned writers. The conservative global
 * conflict domain also covers workspaces whose canonical Git identity is not yet
 * known, including linked worktrees. External processes require separate checks. */
export class ExecutionMutations {
  private writers = 0;
  private leased = false;
  registerWorkspace(_workspaceId: string, _commonDir?: string): void {}
  isLeased(_workspaceId: string): boolean { return this.leased; }
  begin(_workspaceId: string): () => void {
    if (this.leased) throw new Error('Execution is temporarily unavailable during a history operation');
    this.writers++;
    let released = false;
    return () => { if (!released) { released = true; this.writers--; } };
  }
  acquire(_workspaceId: string, _commonDir?: string): () => void {
    if (this.leased || this.writers) throw new Error('Execution is busy; finish active work before the history operation');
    this.leased = true;
    let released = false;
    return () => { if (!released) { released = true; this.leased = false; } };
  }
}
