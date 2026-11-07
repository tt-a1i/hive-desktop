/** Real original Codex approvals; only openai/gpt-6.1-terra, one owned marker, no global config changes. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { lstat, mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { CodexInteractions } from '../src/agents/codex-interactions.js';
import { CodexTransport, type CodexServerRequest } from '../src/agents/codex-transport.js';
if (!process.argv.includes('--live') && !process.argv.includes('--prepare')) throw new Error('Use --prepare (no inference) or --live (one native Terra turn)');
// 0.166.0 rejects legacy untrusted config despite retaining it in generated protocol unions.
const live = process.argv.includes('--live');
const original = '/Users/tsk/.npm-global/bin/codex';
const version = execFileSync(original, ['--version'], { encoding: 'utf8' }).trim();
assert.equal(version, 'codex-cli 0.166.0');
const artifacts = resolve('../../.artifacts'); await mkdir(artifacts, { recursive: true });
const directory = await mkdtemp(join(artifacts, 'codex-native-interactions-'));
const workspace = join(directory, 'owned workspace'); await mkdir(workspace, { mode: 0o700 });
const wrapper = join(directory, 'codex-approval-fixture.sh');
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
await writeFile(wrapper, `#!/bin/sh\nexec ${quote(original)} --config 'approval_policy="on-request"' --config 'sandbox_mode="read-only"' --config 'approvals_reviewer="user"' "$@"\n`, { mode: 0o700 });
const marker = `HIVE_NATIVE_APPROVAL_${randomUUID()}`;
const name = `marker-${randomUUID()}.txt`;
const target = join(workspace, name);
const readCommand = `cat ${quote(target)}`;
const writeCommand = `printf '%s\\n' ${quote(marker)} > ${quote(target)}`;
const rawCommands = new Map<string, 'read' | 'write'>([[readCommand, 'read'], [`cat ${name}`, 'read'], [writeCommand, 'write'], [`${writeCommand} && ${readCommand}`, 'write']]);
// Only byte-exact shell presentations of the fixed command are eligible; parsed actions alone are insufficient.
for (const [command, kind] of [...rawCommands]) for (const shell of ['/bin/zsh', '/bin/bash', '/bin/sh', 'zsh', 'bash', 'sh']) for (const flag of ['-lc', '-c']) {
  rawCommands.set(`${shell} ${flag} ${quote(command)}`, kind);
  rawCommands.set(`${shell} ${flag} ${JSON.stringify(command)}`, kind);
}
const receipt: Record<string, unknown> = { pass: false, realNative: true, inference: live, requestedModel: 'openai/gpt-6.1-terra', version,
  directory, workspace, wrapper, target, marker, configuration: { approval_policy: 'on-request', sandbox_mode: 'read-only', approvals_reviewer: 'user' },
  uiEnvironment: { HIVE_CODEX_COMMAND: wrapper, HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop-state') }, approvals: [], notifications: [], events: [], syntheticEvidence: false };
let transport: CodexTransport | undefined;
let threadId = ''; let turnId = ''; let terminalComplete = false; let writeAccepted = false;
let terminalResolve!: (turn: Record<string, unknown>) => void; let terminalReject!: (error: Error) => void;
const terminal = new Promise<Record<string, unknown>>((yes, no) => { terminalResolve = yes; terminalReject = no; }); void terminal.catch(() => {});
let haltReject!: (error: Error) => void;
const halted = new Promise<never>((_, no) => { haltReject = no; }); void halted.catch(() => {});
const items = new Map<string, Record<string, unknown>>();
const waiters = new Map<string, (item: Record<string, unknown>) => void>();
const interactions = new CodexInteractions(() => threadId, event => {
  (receipt.events as unknown[]).push(event.type === 'interaction' ? { type: event.type, sessionId: event.sessionId, interaction: { id: event.interaction.id, method: event.interaction.method, state: event.interaction.state, threadId: event.interaction.threadId, turnId: event.interaction.turnId, itemId: event.interaction.itemId, nativeRequestId: event.interaction.nativeRequestId } } : event);
}, id => transport?.cancelServerRequest(id));
const exists = async () => { try { await lstat(target); return true; } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false; throw error; } };
async function persist() { await writeFile(join(directory, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n'); }
function object(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }
async function deadline<T>(operation: Promise<T>, label: string, timeoutMs = 120000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([operation, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error(`${label} timed out`)), timeoutMs); })]); }
  finally { clearTimeout(timer); }
}
async function approval(request: CodexServerRequest): Promise<unknown> {
  const params = request.params;
  if (!object(params) || params.threadId !== threadId || params.turnId !== turnId || typeof params.itemId !== 'string' || !interactions.supports(request.method)) {
    receipt.blockedRequest = { method: request.method, id: request.id, reason: 'Unexpected/unowned native request; parameters omitted' };
    await persist(); haltReject(new Error('Unexpected native request; no permission granted')); throw new Error('Unsupported test interaction');
  }
  const pendingReply = interactions.request(request); void pendingReply.catch(() => {});
  let item = items.get(params.itemId);
  if (!item && request.method === 'item/fileChange/requestApproval') {
    try { item = await deadline(new Promise(resolve => waiters.set(params.itemId as string, resolve)), 'native file diff', 5000); }
    finally { waiters.delete(params.itemId); }
  }
  const entry = interactions.snapshot().find(value => value.nativeRequestId === request.id)!;
  const absent = !await exists();
  let kind: 'read' | 'write' | undefined;
  let allowed = false;
  if (request.method === 'item/fileChange/requestApproval' && params.grantRoot == null && item?.type === 'fileChange' && Array.isArray(item.changes) && item.changes.length === 1) {
    const change = item.changes[0];
    allowed = absent && !writeAccepted && object(change) && resolve(workspace, String(change.path)) === target
      && object(change.kind) && change.kind.type === 'add' && change.diff === marker + '\n';
    if (allowed) kind = 'write';
  } else if (request.method === 'item/commandExecution/requestApproval' && (params.kind == null || params.kind === 'command')
    && params.cwd === workspace && params.networkApprovalContext == null && params.additionalPermissions == null && typeof params.command === 'string'
    && (!Array.isArray(params.availableDecisions) || params.availableDecisions.includes('accept'))) {
    kind = rawCommands.get(params.command);
    allowed = kind === 'read' ? !absent && writeAccepted : kind === 'write' && absent && !writeAccepted;
  }
  // Capture pending state and prove nonexistence BEFORE approving any write. No secret answers are collected.
  const observation = allowed ? { request, item: item ?? null, interaction: entry, markerAbsentBeforeApproval: absent, kind, decision: 'pending' }
    : { method: request.method, nativeRequestId: request.id, itemId: params.itemId, markerAbsentBeforeApproval: absent, decision: 'pending-unsafe-not-approved', reason: 'Does not exactly match the single-marker allowlist; raw parameters omitted' };
  (receipt.approvals as unknown[]).push(observation);
  await persist();
  console.log(JSON.stringify({ stage: 'native-approval-pending', method: request.method, markerAbsentBeforeApproval: absent, allowed, evidence: join(directory, 'receipt.json') }));
  if (!allowed) { haltReject(new Error('Unsafe or unrecognized pending approval; not accepted')); return pendingReply; }
  if (kind === 'write') { assert.equal(absent, true); writeAccepted = true; }
  interactions.respond(entry.id, { decision: 'accept' });
  Object.assign(observation, { decision: 'accept', singleActionOnly: true });
  await persist();
  return pendingReply;
}
try {
  receipt.targetAbsentBeforeTurn = !await exists(); assert.equal(receipt.targetAbsentBeforeTurn, true);
  transport = await CodexTransport.start({ command: wrapper, cwd: workspace, onRequest: approval,
    onNotification(notification) {
      const params = notification.params;
      if (!object(params)) return;
      if (notification.method === 'serverRequest/resolved' && (typeof params.requestId === 'string' || typeof params.requestId === 'number')) interactions.resolved(params.requestId);
      if (params.threadId !== threadId) return;
      if (notification.method === 'turn/started' && object(params.turn)) turnId = String(params.turn.id);
      if ((notification.method === 'item/started' || notification.method === 'item/completed') && object(params.item) && typeof params.item.id === 'string') {
        items.set(params.item.id, params.item); interactions.item(String(params.turnId), params.item); waiters.get(params.item.id)?.(params.item);
        (receipt.notifications as unknown[]).push({ method: notification.method, itemId: params.item.id, type: params.item.type, status: params.item.status ?? null });
      }
      if (notification.method === 'turn/completed' && object(params.turn)) { terminalComplete = true; interactions.endTurn(String(params.turn.id), params.turn.status === 'interrupted' ? 'aborted' : 'turn-ended'); terminalResolve(params.turn); }
      if (notification.method === 'error' && object(params.error)) receipt.nativeError = { message: params.error.message, willRetry: params.willRetry };
    }, onClose(info) { if (!terminalComplete && live) terminalReject(info.error); } });
  const started = await transport.sendRequest<{ thread: { id: string; path: string | null }; model: string; modelProvider: string; approvalPolicy: unknown; approvalsReviewer: string; sandbox: { type: string; networkAccess: boolean } }>('thread/start', {
    cwd: workspace, model: 'gpt-6.1-terra', modelProvider: 'openai', allowProviderModelFallback: false, historyMode: 'legacy', ephemeral: true,
    approvalPolicy: 'on-request', approvalsReviewer: 'user', sandbox: 'read-only' });
  threadId = started.thread.id;
  receipt.thread = { id: started.thread.id, model: started.model, modelProvider: started.modelProvider, approvalPolicy: started.approvalPolicy, approvalsReviewer: started.approvalsReviewer, sandbox: started.sandbox, ephemeral: true };
  assert.equal(started.model, 'gpt-6.1-terra'); assert.equal(started.modelProvider, 'openai');
  assert.equal(started.approvalPolicy, 'on-request'); assert.equal(started.approvalsReviewer, 'user'); assert.equal(started.sandbox.type, 'readOnly'); assert.equal(started.sandbox.networkAccess, false);
  await persist();
  console.log(JSON.stringify({ stage: 'prepared', inference: live, wrapper, uiEnvironment: receipt.uiEnvironment, workspace, evidence: join(directory, 'receipt.json') }));
  if (live) {
    const prompt = `Perform this single bounded verification in ${JSON.stringify(workspace)}. Create exactly the new file ${JSON.stringify(name)} containing exactly ${JSON.stringify(marker + '\n')}. Prefer apply_patch with an Add File patch; request native approval when necessary. Then read that same file with exactly this shell command: ${JSON.stringify(readCommand)}. Do not read or change any other files, inspect environment/auth/config, use network, run setup, or use additional tools. If apply_patch is unavailable, the only permitted write command is ${JSON.stringify(writeCommand)}. Do not ask for persistent/session permissions or approval-rule amendments. Finish with APPROVAL_VERIFIED after the native read returns the marker.`;
    receipt.prompt = prompt;
    const turn = await transport.sendRequest<{ turn: { id: string } }>('turn/start', { threadId, model: 'gpt-6.1-terra', approvalPolicy: 'on-request', approvalsReviewer: 'user',
      sandboxPolicy: { type: 'readOnly', networkAccess: false }, effort: 'low', input: [{ type: 'text', text: prompt, text_elements: [] }] });
    turnId = turn.turn.id; receipt.turnId = turnId;
    const completed = await deadline(Promise.race([terminal, halted]), 'native Terra approval turn');
    receipt.completedTurn = { id: completed.id, status: completed.status, error: completed.error ?? null };
    assert.equal(completed.status, 'completed'); assert.equal(writeAccepted, true, 'A real native approval must precede the write');
    assert.equal(await readFile(target, 'utf8'), marker + '\n');
    assert.deepEqual(await readdir(workspace), [name]);
    const nativeItems = [...items.values()];
    const read = nativeItems.find(item => item.type === 'commandExecution' && item.status === 'completed' && item.exitCode === 0 && typeof item.aggregatedOutput === 'string' && item.aggregatedOutput.trim() === marker);
    assert.ok(read, 'Native successful read item must contain the exact marker');
    receipt.nativeReadItem = read;
    receipt.nativeFileItems = nativeItems.filter(item => item.type === 'fileChange');
    receipt.finalContent = await readFile(target, 'utf8');
    receipt.verifiedApprovalMethods = [...new Set((receipt.approvals as { request?: { method: string }; decision: string }[]).filter(entry => entry.decision === 'accept').map(entry => entry.request!.method))];
  }
  receipt.pass = true;
} catch (error) { receipt.pass = false; receipt.error = String(error); }
finally {
  interactions.clear('aborted');
  if (transport && turnId && !terminalComplete) {
    try { await transport.sendRequest('turn/interrupt', { threadId, turnId }); await deadline(terminal.catch(() => {}), 'interrupted turn', 10000); }
    catch (error) { receipt.cleanupInterruptError = String(error); }
  }
  try { await transport?.close(); } catch (error) { receipt.cleanupError = String(error); receipt.pass = false; }
  await persist();
}
console.log(JSON.stringify({ pass: receipt.pass, inference: live, evidence: join(directory, 'receipt.json'), wrapper, workspace, verifiedApprovalMethods: receipt.verifiedApprovalMethods ?? [] }));
if (receipt.pass !== true) process.exitCode = 1;
