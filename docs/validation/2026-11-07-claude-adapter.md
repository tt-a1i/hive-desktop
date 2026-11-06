# Claude adapter: native controls verified, inference unverified

2026-11-07, macOS. This records native controls, shared runtime integration and desktop UI checks. It does not establish Claude model execution, team-live acceptance or account entitlement.

## Scope

`packages/runtime/src/agents/claude.ts` exports `createClaudeSession`. It implements `PiSessionHandle` with native Claude session identity plus `teamToolName`, `processId` and `catalog()`. `sessionFile` remains undefined because the official session API exposes session UUIDs, not the transcript's physical path. Resume uses `options.sessionId` and public `getSessionMessages`, which selects the native parent-UUID conversation chain.

The runtime pins official `@anthropic-ai/claude-agent-sdk` **0.3.317** and its schema peer `zod` **4.6.7**. On this machine it explicitly launches the user's original `/Users/tsk/.npm-global/bin/claude`, Claude Code **2.1.317**, through `pathToClaudeCodeExecutable`. It does not choose the SDK's bundled CLI, read or forward credentials, write global settings, set an authentication proxy, or change model providers. The external executable can be supplied via `command` or `HIVE_CLAUDE_COMMAND`.

Native configuration sources remain enabled. The host uses default permission mode and does not supply bypass/automatic permission grants. Native `canUseTool`, user-dialog and MCP-elicitation callbacks are exposed. Without a corresponding UI handler, requests fail visibly and are denied/cancelled; only explicitly supported dialog kinds are advertised. Existing native configuration rules can decide tools before a callback is needed, as documented by the SDK.

Model options use the `claude` catalog namespace, with the native model ID. An initial provider mismatch is rejected before spawning. `setModel` is a session control operation. It does not send a user prompt or use the SDK settings-writing APIs. The user's Terra-only model-test constraint was maintained: **no Claude inference requests were made**.

Hidden Hive context uses documented `SDKUserMessage.shouldQuery:false` and `client_composed:true`. The real native record remains a user record; Hive presentation metadata identifies its UUID and displays it as hidden context. The metadata file is presentation/delivery-acknowledgement bookkeeping, not another collaboration ledger. User text and team dispatch/report semantics still belong to the existing Hive CLI/store.

Streaming reconstruction groups native assistant envelopes sharing the same API `message.id`; SDK block envelopes can have different UUIDs. All native wrapper UUIDs and original records remain in source metadata. During a process lifetime an in-memory display-ID alias joins stream deltas with final updates; reload uses the first native wrapper UUID. Identical consecutive blocks from different UUIDs remain distinct.

A successful delivery requires a successful terminal result correlated to that exact input UUID, the durable native input and an assistant UUID observed in that run on the active native chain. Later real user inputs, absent/abandoned assistant records, failed results and queued continuation results reject completion. Abort waits for a terminal result and every owned tool promise; process failure also aborts and awaits owned tools before clearing the active run. Intentional close waits for the owned process; unexpected exits are separately reported.

## Verification

`pnpm --filter @hive-desktop/runtime check`, `pnpm --filter @hive-desktop/runtime build` and `pnpm --filter @hive-desktop/runtime check:claude` passed.

The targeted checks cover distinct evidence boundaries:

- **Actual original CLI and official SDK, no input:** initialization, 52 native commands, 5 native model catalog entries, idle interrupt and close. Account/authentication blobs are excluded from receipts.
- **Actual SDK session, no inference:** connected SDK MCP server, catalog model selection through the control API, transcript-only hidden context, public native history read, preserved UUIDs after resume, no assistant messages, untouched lazy binding callback and intentional close without an unexpected-exit callback.
- **Actual owned process death:** SIGKILL of the adapter's recorded child PID, one unexpected-exit callback, subsequent prompt rejected before submission and awaited disposal.
- **Synthetic native history/event fixtures:** block-stream/final display identity, native UUID provenance, equal text blocks, cumulative updates, delivery presentation, tool-result identity and exact terminal/input/assistant association. These are explicitly synthetic; they prove projection behavior, not model or team execution.
- **Official tool callback + actual Hive CLI + loopback HTTP fixture:** binding absent outside a run, lazy current binding per callback, changed workspace/authentication headers, and abort/cleanup of a real hanging CLI request. This invokes the host callback directly; it is not a model-selected MCP tool call or an actual Hive dispatch/report loop. The separate native-session check proves the SDK MCP server handshake.

Receipts: `.artifacts/claude/native-init.json`, `.artifacts/claude/session.json` and `.artifacts/claude/lifecycle.json`. The native hidden-context session and temporary project paths are retained in those receipts. No raw credentials or account responses are saved.

## Remaining boundaries

Claude assistant inference, streamed responses from a real Claude model, real permission/user-input UI flows, actual model-selected MCP calls, Orchestrator/Worker team-loop behavior, mid-inference abort and active-tool process-death behavior have not been model-tested. Their handling is implemented and checked through synthetic records, direct callback execution and source review, with the limits above.

The shared ChatManager and Hive executor now recognize `claude` / `hive:claude`, persist the native session UUID, provide catalog lookup, inherit explicit backend/model choices on spawn, and resolve the current team binding lazily. Each process receives its current MCP tool name in hidden context. App-local Claude executable settings follow environment → app setting → PATH; old settings and old two-field updates remain compatible. The native UI includes Claude in session creation, member backend selection, model selection and external-program settings.

Actual no-inference shared-runtime checks cover create/open/catalog/close, UUID persistence, full runtime restart, SIGKILL recovery with the same native UUID, invalid executable paths and legacy settings. No activation, member start, user prompt or model-selected tool was invoked; Hive run/token/dispatch counts remain zero. Receipts: `.artifacts/claude-runtime-gEhn3M/receipt.json` and independent `.artifacts/claude-runtime-dzO0Wg/receipt.json`.

Native CLI resume adds zero-token `<synthetic>` assistant placeholders for transcript-only context. The earlier raw “zero assistants after resume” assertion failed; that attempt remains at `.artifacts/claude-runtime-T1qSXE`. Actual CUA reproduced a visible `No response requested.` bubble after reopening an otherwise empty chat. The correction hides only the structured native placeholder immediately associated with a registered Hive hidden-context UUID, retains the raw record/native UUID/context association, and excludes synthetic assistants from delivery-completion evidence. Same-text real user/assistant messages, errors and tool results remain visible in focused fixtures. Two real runtime resumes had zero visible assistant bubbles; original UI data was also replayed. Evidence: `.artifacts/claude-placeholder-resume-VRW5ul/receipt.json` and `.artifacts/claude-placeholder-original-proof.json`.

Windows executable lookup recognizes native `.exe` names and rejects shell launchers, but no Windows Claude process was run. The workspace now excludes the SDK's eight precisely named optional native CLI packages; the adapter explicitly selects the user's original executable. The isolated Mac package and no-inference control/recovery checks passed without those bundled binaries, as recorded in the portable-packaging guide. Windows package smoke does not validate Claude account/model behavior. No full test suite was run: verification targeted new adapter and runtime boundaries, and Claude inference remains outside the permitted test model.

CUA additionally verified a frozen macOS bundle: saving the native executable in application settings, creating an empty Claude chat, loading the five native catalog entries, quitting and reopening. The initial reopen reproduced the placeholder defect; a newly packaged runtime reopened that same persisted chat and displayed the empty-conversation state with no assistant bubble. No prompt was entered or sent and no member was started. Proof bundles are `.artifacts/claude-ui-proof` (before) and `.artifacts/claude-ui-fixed` (after), sharing the former's dedicated state directory.
