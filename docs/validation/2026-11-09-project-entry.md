# Project entry, draft transfer and program repair — 2026-11-09

UI slice built after `e91d042`; full desktop parity is still in progress.

## Behavior

The empty project/conversation surface now identifies its actual selected project name and directory, with a file-manager reveal action. Before creating a chat, an explicit explanation connects writing a draft to choosing a lead. The lead picker is visually primary and has a caret; its existing four-backend menu and session creation handlers are reused.

Native checking reproduced an unrelated but directly visible error-state defect: after `chat.open` failed, the footer still said “正在打开会话…”. The guarded request failure now settles that status; decoded snapshot failures get the same default, and a valid snapshot transitions to connected before restoring native interactions. The shared error banner exposes its complete existing diagnostic through a native accessibility label.

## Actual native flow

Evidence: `.artifacts/project-entry-e91d042/`.

1. Started owned empty desktop state. Clicked Open project folder, used the native macOS Go to Folder sheet and selected the owned `project` directory. Hive created exactly one real workspace and no chat automatically.
2. Observed actual project name/path and draft/Agent explanation. Entered Chinese text containing `/tmp/hive-entry`, clicked the project-understanding suggestion; it appended a second line without replacing the first.
3. Selected Codex through the four-backend native menu. Exactly one Codex record was created. Both draft lines transferred to the session.
4. PATH Codex `/opt/homebrew/bin/codex` reported `0.46.0` and rejected `app-server --listen`. Failure remained visible, Send disabled, draft intact. Reopened the same owned state with the fix: complete diagnostic appeared in AX, footer read “未能打开会话”. This is an observed compatibility boundary; no fallback to a different protocol or agent was added.
5. Original `/Users/tsk/.npm-global/bin/codex` reported `0.167.0`. It opened the same session; selected `openai/gpt-6.1-terra` via the native model picker. Normal and approximately 822 × 597 logical-pixel layouts retained the two-line draft, project identity and model/actions.
6. Repeated with no `HIVE_CODEX_COMMAND` override, allowing the old PATH CLI to fail. From the failure page opened External programs, entered the compatible CLI path, ran its real version check, saved settings, returned and clicked Reopen conversation. Same chat reopened in the same application without a restart, retaining draft and Terra. Configuration is scoped to the owned desktop database.
7. All native instances exited normally. The final configuration/repair instance was exec session 42495 (exit 0); earlier instances also returned 0. A full cold runtime loaded the application-saved command (source `app`) and restored the same chat/model with pending 0 and history length 0.

The artifact `check.mjs` asserts exact durable draft text, one chat/workspace, zero Hive messages, empty delivery spool, accessible original failure, saved executable source and actual cold Codex snapshot. `receipt.json` records the result. Its first attempt used the RPC response wrapper shape for a direct `chats.open` call (`cold.session.id`); after inspecting the actual return contract, corrected the artifact assertion to `cold.id`. No production behavior was changed for that check.

No prompt was sent and no inference was requested. Native initialization initially displayed the user's existing default before the explicit per-chat Terra choice; no other model was invoked. Global CLI installation/authentication/default configuration was not changed.

## Checks and limits

Passed macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, Windows target `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` with LLVM on PATH, and `git diff --check`.

This changes presentation and UI open-status reporting, with no backend protocol/schema change. No new source tests or full suite; actual native failure/recovery, real HTTP/SQLite and original Codex metadata/cold-open provide the targeted verification. New hint translations were added; English visual inspection of this second slice, full keyboard/VoiceOver coverage, file-manager button activation and Windows native flow remain pending. CLI version probing alone is not full compatibility verification. Candidate/current ZIPs remain unchanged.
