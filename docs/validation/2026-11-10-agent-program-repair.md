# External Agent program repair — 2026-11-10

Source baseline `54a82ce` plus the verified Codex version metadata correction. Evidence `.artifacts/agent-repair-54a82ce/`. This step checks repair through existing settings, original Codex and real persisted history, rather than adding a second readiness protocol or presenting executable detection as account/model validation.

## Actual native failure → repair → reopen

Created an isolated copy of a completed Terra conversation and its exact unsent Chinese/English draft. Through the real desktop settings service/HTTP/SQLite, set only this copy's Codex command to the deliberately missing `/hive-desktop-validation/missing-codex` (`prepare.mjs`, `prepare-receipt.json`). No user/global settings or auth changes.

1. Native launch failed to open the selected conversation with actual ENOENT. The page retained the complete draft, disabled send/model controls and exposed External program settings / Reopen conversation (`missing.*`).
2. Actual External program settings opened the existing page with the missing saved path. Check Codex version returned Program not found with the actual command/source (`probe-missing.*`).
3. Clicked the path input, Cmd+A and ordinary keyboard input entered `/Users/tsk/.npm-global/bin/codex`. Previous result became stale, the page requested a recheck, and Unsaved changes appeared. Actual version check returned 0.167.0 and source Entered path while Currently using still named the missing saved command (`probe-success.*`). This confirms checking does not silently save or change launch selection.
4. Direct indexed click on the offscreen Save button was rejected by computer-use tooling. From the path input, five actual Tabs brought Save into view with focus (`save-focus.*`); Return saved through the real service. Currently using changed to the new app path, saved notice appeared, Save disabled and Unsaved changes cleared (`saved.*`). No source change was inferred from the indexed-click failure.
5. Actual Back returned to the failed conversation. Reopen conversation then restored the same original Codex thread/history, actual completed command and ordered replies, enabled composer controls and exact original unsent draft (`reopened.*`). No new conversation, send or recovery model turn requested.
6. Cmd+Q exited normally (exec 9065, exit 0). Independent cold runtime read (`check-cold.mjs`, `cold-receipt.json`) confirmed the new app path/source, same thread, Terra/Medium, pending 0, only the prior two user inputs, one actual command result, ordered marker replies and exact draft. No model turn requested.

## Correction found during use

The original version result incorrectly warned that Codex 0.167.0 was not verified because `DesktopSettings.probe` still advertised 0.166.0. Existing real 0.167.0 Terra evidence includes [live input/queue](2026-11-10-composer-queue-input.md), [busy lifecycle](2026-11-10-model-picker-busy.md), model/effort changes and this repair. Updated the advertised version to 0.167.0 and added an assertion to the existing executable-probe integration check. The synthetic probe continues returning 0.166.0, so detected version and verification metadata remain independently observable; it does not replace native evidence.

After rebuilding the runtime, launched the owned app again, opened External programs and actually checked the saved Codex path. Native readout returned Detected version 0.167.0 without the outdated compatibility warning; the separate statement that version detection does not verify sign-in or model availability remained (`fixed-version.*`). Second normal quit exited 0 (exec 76536). No new inference in this step. This is current Mac evidence, not Windows native verification or universal compatibility with other versions. Prior version-specific native test fixtures are historical baselines and were not rewritten.

## Checks and boundary

Passed `HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime check:programs` with Node 22 on PATH: TypeScript build, real HTTP/SQLite/probe processes, original Codex --version, unsaved input, environment precedence, literal argv, missing/failed/unrecognized output, output limit, timeout termination, shutdown cancellation and compiled JSONL wire process exit. Receipts copied to `program-core-receipt.json` and `program-wire-receipt.json` within this evidence directory. `git diff --check` passed.

Production change is one runtime metadata value; no Rust/protocol/schema/permissions changes and no dependency added. No unrelated full test suite or repeat Mac/Windows Rust compilation. Authentication renewal, a missing executable on Windows and other backend repair paths remain separate native acceptance. Candidate ZIP remains `b539c26` and excludes current source changes. Full product goal remains active.
