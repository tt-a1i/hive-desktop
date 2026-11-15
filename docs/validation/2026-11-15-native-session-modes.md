# Per-session native modes

Date: 2026-11-15. Source implementation and controlled protocol verification only; this record does not claim new package, native graphical, Windows interaction, or real-model acceptance.

## Behavior

The model settings panel now exposes the current backend's native session modes. Codex collaboration mode and access permissions are separate controls because they are independent native settings. Existing records omit both overrides and keep their existing native configuration.

- Codex collaboration choices are native `plan` and `default`, and appear only when the current owned app-server advertises them through `collaborationMode/list`. No plan prompt is synthesized. Missing methods report unsupported; failures report unknown. Successful and definitively unsupported capability results are cached separately for each control; unknown failures can recover on re-read. The selected mode is pending until an accepted `turn/start` carries the complete native collaboration setting and current model/reasoning value.
- Codex permission choices are read-only, workspace-write and full access, narrowed against `configRequirements/read`. Read-only/workspace-write use `on-request`; full access explicitly selects `never` and `danger-full-access`. The native thread start/resume receives the sandbox mode, and each subsequent turn receives the full sandbox policy. No default is changed. The control explains that this affects the native session only, and that full access disables Codex sandboxing and approvals.
- No “restore inherited configuration” option is offered after selecting an explicit Codex policy. Omitting later turn fields would retain Codex's sticky override, and an ID alone cannot reconstruct the original native configuration after a cold restart.
- Claude exposes the official SDK's `default`, `acceptEdits` and `plan` permission modes. The setter must acknowledge before Hive saves or reports the choice. A saved mode is passed at startup/resume and acknowledged again before any delivery. Dangerous bypass is omitted. No Claude account or native model validation was performed.
- Built-in Pi and generic external Pi explicitly report unsupported. They receive no new permission flow, prompt simulation or extension capability claim.

The backend saves provider-specific selections in each ChatIndex record. Reads distinguish selected, applied and pending-next-turn values. Mutation rejects active turns, queued deliveries and interactions. During an acknowledged change, the delivery queue pauses; reports accepted during that interval wait until the change completes. Native setter failure, an unknown result or index write failure retires the changed live session through the existing pause/dispose path. Cold restore with an unavailable saved mode fails before deliveries, rather than falling back and sending under a different policy. Existing session origin, worker identity, dispatch, report and outbox ownership are preserved.

## Evidence

One bounded zero-inference query used the local Codex 0.169.0 app-server. It performed only initialize, collaborationMode/list and configRequirements/read. The whitelist result was `plan/default` and no managed requirements. Owned child PID 71760 exited with status 0 after stdin closed. No account, model turn, login, refresh or raw configuration operation was requested.

`pnpm --filter @hive-desktop/runtime check:session-modes` builds the production runtime and runs four controlled checks:

1. `codex-modes.ts` uses a real local JSONL peer subprocess to verify independent turn fields, full sandbox policy, start/resume, busy rejection, restricted/unsupported capabilities and unknown responses.
2. `codex-mode-persistence.ts` uses real Hive/SQLite and the local peer to verify both saved axes across cold start, exact thread permission fields, disk-write retirement and fail-closed unavailable saved modes.
3. `session-modes.ts` uses the official Claude SDK and a synthetic peer to verify start/resume arguments, acknowledged restore/set/reject, persisted restart values, malformed records, write failure and unavailable saved modes. Busy, queued and interaction guards reject without a setter or disk change. Injected unsupported Pi handles prove no new approvals/native calls. A controlled report race waits for native acknowledgement and executes exactly once afterward.
4. `session-modes-wire.ts` exercises production JSONL read/set RPCs, independent Codex permissions, invalid choices and cold restart with both synthetic peers. It asserts no Codex turn or Claude query is submitted.

The runtime strict TypeScript check and the existing Codex check suite passed. The macOS UI compiled offline using its own target directory; targeted rustfmt and English locale closure/duplicate checks passed. UI state includes generation/session guards against stale responses. An offline MSVC target check was attempted from an APFS clone of the existing Windows cache. Cargo began rebuilding foundational dependencies, so it was stopped to respect the bounded cache/build budget; MSVC compilation remains unverified. The dedicated graphical acceptance task still owns interactive verification. Native Terra mode acceptance remains separate; no vendor inference was performed here.
