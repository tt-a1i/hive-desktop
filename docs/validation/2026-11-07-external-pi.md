# External Pi RPC session verification

2026-11-07, macOS, installed `/Users/tsk/Library/pnpm/pi` 1.4.0. This receipt covers the external session adapter. Subsequent shared-runtime integration and real two-agent acceptance are recorded in [the team receipt](2026-11-07-external-pi-team.md).

## Behavior verified

`createExternalPiSession` in `packages/runtime/src/agents/external-pi.ts` implements the existing `PiSessionHandle` and exposes the child's unique `teamToolName`. External Pi owns its normal configuration, OAuth, extensions and durable session. Hive explicitly loads its bridge extension alongside those resources. The executable can be supplied through `command` or `HIVE_PI_COMMAND`.

The optional model is passed as native CLI provider/model arguments before extension initialization, then verified with RPC `set_model` before accepting prompts. Pi 1.4.0's public `session.setModel` defaults to session-local selection; this path does not write global model defaults. All actual model calls in this verification used native `openai/gpt-6.1-terra`.

The lazy `teamBinding` callback runs immediately before each prompt. The idle bridge `bind` action replaces an in-child closure; credentials are neither conversation messages nor session entries. The team tool reuses Hive's existing CLI and ledger. Native external extensions share the same OS-user trust; this is not secret isolation from other installed extensions.

Finalized history follows the active Pi branch and carries canonical session entry IDs in `source.entryId` and the original `rawBackendItem`. Live/final message IDs use an in-memory alias for UI continuity; reopening the process uses canonical persisted IDs. Tool-result presentation IDs use the durable native `toolCallId`, matching live tool events; canonical source entry IDs remain available separately. Completion requires the exact terminal assistant entry from the observed successful run and active ancestry, with no intervening delivery or user input.

Fresh custom-only sessions persist through the public extension header/entries API followed by `ctx.switchSession`, without fake user messages or private SDK fields. RPC `prompt` acceptance is separate from run completion; delivery promises await `agent_settled`, refresh entries, then append the completion marker. Abort clears queues and waits for the run's terminal boundary. Unexpected process death rejects work and invokes `onUnexpectedExit`; intentional disposal does not invoke it.

## Commands and evidence

- `pnpm --filter @hive-desktop/runtime check`
- `pnpm --filter @hive-desktop/runtime build`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session-lifecycle.ts`
- `pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session-projection.ts` (reopens the retained live receipt without inference)
- `pnpm --filter @hive-desktop/runtime exec tsx checks/external-pi-session-live.ts --live`
- Earlier real-RPC transport and exact completion checks: `checks/external-pi-transport.ts`, `checks/external-pi-completion.ts`.

All listed checks passed. No-inference checks cover fresh hidden context, source identity, process restart, callback laziness, missing activation binding and an actual SIGKILL of the owned Pi process. The exact completion checks reject failed/aborted delivery A followed by successful B and reject abandoned-branch assistants.

The live session wrote a random code supplied only through hidden context to a real file using Pi's real bash tool. It verified streamed text/thinking deltas, tool lifecycle, live/final ID continuity, exact completion, canonical history on restart, and absence of a synthetic binding token from both conversation snapshots and the raw transcript. A second delivery started a real `sleep 30` bash call, was aborted at tool start, awaited terminal settlement and produced no completion marker.

Receipts are retained under `.artifacts/external-pi/session.json` and `.artifacts/external-pi/session-live.json`; the latter includes messages, events and the underlying session path. Temporary project/session directories remain available at the paths in those receipts.

The first live adapter probe found that Pi RPC `message_update` omits the cumulative SDK `message` field. That probe executed the tool and completed the delivery but failed the delta assertion. The mapper was corrected using Pi 1.4.0's `docs/json.md` delta-only schema and the successful live check was rerun. The initial transcript remains at `/var/folders/v1/6vx82zzx32q7y_s1w4sx84j00000gp/T/hive-external-session-live-0ymQJe`.

## Remaining boundaries

Native desktop interaction, Windows executable discovery and packaging have separate evidence. The direct adapter live probe deliberately did not invoke its synthetic team binding against a real Hive server; the subsequent team receipt does. Native extension dialogs without a desktop responder are cancelled with a visible adapter error; fire-and-forget notifications follow RPC's documented limitations. Reopening a conversation does not resume an interrupted tool process. The live abort check verifies terminal settlement and no completion marker, but does not establish shell PID or descendant-process cleanup.

No full test suite was run: verification targeted the new RPC, persistence, presentation and process-lifecycle boundaries. No existing tests or shared runtime/UI files were changed for this adapter.
