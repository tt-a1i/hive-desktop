# Directed program repair from an existing project — 2026-11-10

Baseline `92179bb`. Source-only UI change; canonical optimized candidate remains 252c19c. Evidence: `.artifacts/project-agent-repair-92179bb/`.

## Change

An external conversation that fails to open already offered program settings and retry, but its settings action opened a generic page. It now reuses the welcome setup helper to target the selected conversation's Codex, Claude or external Pi field. Built-in Pi remains outside this external-program action.

During actual Codex repair, opening settings then selecting/typing before the initial read completed concatenated the saved missing path with the new path. The previous edit-preservation guard protects changed text, but cannot preserve a selection made before saved text arrives. The setup helper now records the intended field, waits for the existing settings read to finish, then focuses and selects that field's entire current value. Already-loaded settings perform the same handoff immediately. Close, read failure, invalid response and navigation to another settings section discard the pending intent. No timer, input replay or new configuration store was added.

The earlier guard that protects edits made during an asynchronous read remains unchanged. This handoff is about focusing a loaded control; it does not queue keystrokes entered before settings controls are enabled.

## Native verification

Copied the prior ff86889 owned state after its processes exited. `prepare.mjs` used the real runtime settings service/HTTP/SQLite to set only this copy's Codex path to an owned nonexistent file. The original completed Terra thread and exact Chinese/English draft were preserved. Explicit source runtime and bundled Node overrides identify this as a debug source check, not an extracted-package claim.

1. Native open failed with real ENOENT, retained the draft, disabled send/model controls, and exposed repair/retry (`missing.ax.txt`).
2. The first attempt captured the selection race: the field contained `<missing-path>/Users/tsk/.npm-global/bin/codex` (`early-selection.ax.txt`). It was never saved or executed.
3. After the fix, repair loaded the actual missing saved path (`target-loaded.ax.txt`). Typed the original executable directly, without clicking the field or selecting again: the whole path was replaced exactly. Probe returned real `0.167.0`; currently-used still named the missing saved path, proving probe had not saved (`probe.ax.txt`).
4. Keyboard Tab revealed Save with a visible ring; Return saved the path through the existing service. Back returned to the same failed conversation. Reopen restored the actual completed command, `PACKAGED_FF_OK 41/3`, and exact unsent draft (`reopened.ax.txt`).
5. Normal quit/cold launch restored the same history/draft without ENOENT (`cold.ax.txt`). Both fixed-run native exits were 0; the earlier diagnostic run also exited 0.

`check-cold.mjs` passed: same session/native thread as preparation, one user input, one tool result with exit0, one exact original reply, pending0, idle, Terra/Medium, exact draft and only Codex's saved path repaired. No new model turn, user delivery, authentication, member or workspace was created by repair/recovery. Fixture and native global account/default configuration were not intentionally changed; this check does not claim a byte audit of all global CLI files.

## Checks and limits

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` passed without warnings.
- LLVM-assisted `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop` passed.
- `git diff --check` passed; no new/removed source tests or full suite. This is native UI focus/loading behavior, verified through real settings, original CLI control and cold runtime assertions.

Actual repair was Codex on Mac. Claude/external Pi use the same targeted helper, but this record does not establish their native missing-program recovery or Windows native behavior. New focus handoff, welcome cards and prior input guard are not yet in the canonical candidate. Complete onboarding/readiness, original picker409 reproduction and full product acceptance remain open.
