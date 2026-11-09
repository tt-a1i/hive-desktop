# Actual running input, queue and Unicode paste — 2026-11-10

Production source `59c1fe9`, no product code change in this step. Follow-up to the [busy picker fix](2026-11-10-model-picker-busy.md). Evidence `.artifacts/queued-composer-59c1fe9/`.

## Actual native sequence

Fresh owned Codex chat/native thread, app-local data and Pi directory. Started the bound orchestrator through existing Hive UI HTTP routes with `hive:codex --provider openai --model gpt-6.1-terra`. All inference used original Codex 0.167.0 / Terra, Medium. No global auth/default changes.

1. Idle baseline: native input click, Cmd+A and `typeText` replaced the fixture draft with `IDLE_TYPED` (`idle-typed.*`).
2. Opened the combined model/effort picker, then used existing authenticated loopback `/api/workspaces/:id/user-input` to request a shell waiting 40 seconds and printing `COMPOSER_LIVE_OK_TOOL`, followed by final `COMPOSER_LIVE_OK`. Picker closed on busy and original draft remained (`before.*`, `running.*`).
3. After actual command execution appeared In progress, clicked the current native input, Cmd+A and typed `After the current task, reply exactly COMPOSER_QUEUED_OK.`. Complete exact text appeared while the tool was still running (`typed-running.*`). This verifies ordinary keyboard input during a live turn, not just AX value assignment.
4. Clicked the actual Queue button. Composer cleared, one received-message card exposed the full queued input and waiting status, and empty Queue disabled (`queued.*`). No Stop activated.
5. Original tool completed and the queued request ran next. Independent cold history contains exactly ordered marker replies `COMPOSER_LIVE_OK`, then `COMPOSER_QUEUED_OK`; only two user deliveries and one actual command result. Actual command `aggregatedOutput` equals `COMPOSER_LIVE_OK_TOOL`, not merely a matching command string. Pending 0; no extra model menu appeared.
6. A Chinese `typeText` attempt on the empty editor did not change its value (`chinese-attempt.*`); this is not counted as successful Unicode keyboard/IME input. The following AX value operation was observed after replies had completed, so `settled.*` is not claimed as an in-flight unsent-draft test.
7. After completion, clicked the native editor, Cmd+A and used actual plain-text clipboard paste. Complete two-line Chinese/English draft with `C:\work\hive` and `/tmp/hive` appeared (`paste-draft.*`). No send. This proves clipboard paste, not composition through a Chinese input method.
8. Cmd+Q exited normally, exec session 80919 returned 0. Independent cold runtime open/native reasoning read retained same thread, Terra/Medium, pending 0, ordered replies and exact unsent multiline draft. `check-cold.mjs` / `cold-receipt.json` record pass and request no model turn.

## Scope and checks

Actual native UI, original Codex, HTTP/SQLite, real shell output and cold restore checked. No synthetic model peer in this case. No product/test code changed, so no repeated compilation/full runtime suite; `git diff --check` passed. Existing macOS/Windows compilation evidence remains its original scope.

Keyboard input and native Queue activation with the merged selector are now covered for this Terra/English/short-window sequence. Chinese IME and empty-editor pointer focus through all automation methods remain unverified; no production bug/fix is inferred from a failed automation attempt alone. Stop activation, other backends, all layouts/fonts/themes, Windows native and candidate-package inclusion remain separate acceptance. Candidate ZIP is still `b539c26`. Full product goal remains active.
