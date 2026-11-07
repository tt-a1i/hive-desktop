export type WorkspaceRemovalStage = 'prepare' | 'stop' | 'delete' | 'index';
export interface WorkspaceRemovalDetails {
  workspaceId: string;
  workspaceRemoved: boolean | null;
  removedSessionIds: string[];
  stage: WorkspaceRemovalStage;
  localIndexCleaned: false;
}
export class WorkspaceRemovalError extends Error {
  constructor(readonly code: 'WORKSPACE_REMOVAL_BLOCKED' | 'WORKSPACE_REMOVAL_FAILED' | 'WORKSPACE_REMOVAL_PARTIAL',
    message: string, readonly details: WorkspaceRemovalDetails, options?: ErrorOptions) {
    super(message, options); this.name = 'WorkspaceRemovalError';
  }
}
export function workspaceRemovalBlocked(workspaceId: string): WorkspaceRemovalError {
  return new WorkspaceRemovalError('WORKSPACE_REMOVAL_BLOCKED', '工作区正在移除，暂时不能执行此操作。',
    { workspaceId, workspaceRemoved: false, removedSessionIds: [], stage: 'prepare', localIndexCleaned: false });
}
