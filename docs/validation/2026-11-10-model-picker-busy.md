# Close the model picker when a background turn starts — 2026-11-10

Source baseline `0129d05`; follows the [combined model/effort panel](2026-11-09-model-effort-panel.md). Evidence `.artifacts/model-busy-0129d05/`.

## Real failure and root cause

Opened the native combined picker in an idle, independently created Codex Terra thread. Through Hive's existing authenticated loopback UI endpoint, sent an owned user-input turn while leaving the picker open. The original Codex 0.167.0 actually ran a 12-second shell tool and returned `PICKER_BUSY_OK`.

Busy rendering replaced the Popover with its disabled trigger, but `choosing_model` stayed true. When the turn settled, the Popover remounted open and focused search again. `before.*`, `during.*`, `tool-running.*`, `reopened.*` show this transition. A subsequent coordinate intended for the composer hit the unexpectedly reopened High option instead; restored Medium immediately. This was an owned test thread, no user settings/global defaults affected. Initial AX/coordinate attempts did not change the draft and are not claimed as successful running input.

The shared `apply_activity` handler now clears the controlled model-picker state whenever the selected session is busy. This covers native state/delta/tool events from foreground and background delivery without timers, new protocol paths or backend changes.

## Real fixed-case verification

- Restarted the owned app with the new source build. Explicitly starting the existing Hive orchestrator emitted the application's existing recovery input and another `PICKER_BUSY_OK`; this is recorded separately, not counted as an extra user send or cold-open-only behavior. All inference in this run used `openai/gpt-6.1-terra`.
- Reopened the picker, then sent a second explicit input through the existing UI endpoint (`fixed-input-receipt.json`): shell waits 12 seconds and prints `PICKER_BUSY_FIXED_OK_TOOL`, then final exactly `PICKER_BUSY_FIXED_OK`.
- `fixed-before.*`: picker open. `fixed-during.*`: actual tool running, disabled combined selector, visible Stop/Queue, no picker surface. `fixed-settled.*`: final received, picker stays closed, same original unsent draft and Medium retained.
- Continued editing after completion through the native editable control; exact updated draft visible in `fixed-editable.*`. The attempts to append while busy did not change the value, so interactive typing/IME during a turn remains unverified by this case. Original draft preservation is verified.
- Normal exits: initial unbound setup app session 48165, original reproduction session 61586, fixed session 93907 all returned 0. Independent cold open/read found the same Terra thread, native Medium, pending 0, two explicit user deliveries, two completed shell tools, two original-marker responses (original + explicit restart recovery) and one fixed-marker response. Exact updated unsent draft persisted. `check-cold.mjs` / `cold-receipt.json` record pass; cold read requested no additional model turn.

The first preparation assumed an unbound empty record already had agentId; corrected the owned binding before the actual case. An initial test launch passed a combined model spec, and the existing structured launch validator rejected it for missing `--provider`; corrected the artifact to separate `--provider openai --model gpt-6.1-terra`. No production fallback or validator change. The preparation log was corrected to acknowledge that explicit restart can cause an existing recovery turn.

## Checks and limits

Passed macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, Windows target `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` with LLVM on PATH, and `git diff --check`. UI state fix: no new UI tests or unrelated full runtime suite. Actual HTTP/SQLite/original Codex/native window/cold read cover this case.

Other backends, Stop/Queue activation, incoming Worker-report-specific timing, disconnect/loading/hidden-navigation transitions, complete typing/focus/IME and Windows native remain separate acceptance. No permission/authentication changes. Candidate ZIP remains `b539c26`, not this source fix. Full product goal remains active.
