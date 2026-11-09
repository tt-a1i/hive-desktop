# Error repair routes and bounded composer — 2026-11-10

Baseline `e6eaefc`. Source/debug UI slice; current/candidate development ZIPs remain `7ae27ca`. Evidence: `.artifacts/repair-actions-e6eaefc/`.

## Actual problem and observable change

1. After a built-in Pi failure, the user could inspect/retry the preserved input but had no direct account/model repair destination in that error panel. External program repair existed only on the open-failure surface. At Dark20 in a compact window, expanded failure content could push editing/footer controls beyond the window; a multiline conversation title consumed extra header height.
2. Shared backend-specific repair actions now open the existing Pi account/model pages or the selected external program field. Pending input/attachments remain pending, and opening settings never retries them. Error details are accessible/selectable. The composer has a bounded, shrinkable content viewport with its action footer outside it; error content has its own bounded focus-scroll region. The title's visible header normalizes whitespace while preserving the complete stored title, AX label and tooltip. Drafts/long content scroll rather than pushing the footer away.
3. Without these changes, users must leave the error context to find configuration manually, and compact windows can hide the controls needed to recover. A version probe remains explicitly distinct from authentication/model readiness; no new readiness service, backend inference probe, store or permission layer was introduced.

The repair action helper is reused by open failures, transient diagnostics and durable delivery failures. It selects by the session's actual backend, without matching error strings. Existing settings, program loading/selection handoff, explicit retry and queue semantics are reused. Failure scroll resets when the error/session changes; ordinary updates retain reading position. Composer focus reveal uses the existing nested Viewport/Reveal helpers, including its input and error region. Native command popovers remain deferred overlays.

## Real Pi SDK/Hive failure fixture

The owned fixture uses `openai/gpt-6.1-terra` with an explicitly synthetic credential and a localhost HTTP Responses endpoint. The actual Pi SDK received one controlled401 containing `Owned loopback expired credential · 原始失败，请修复配置`; no vendor/model inference occurred. Actual Hive/SQLite persisted one pending original multiline input and one uploaded text attachment.

Preparation first incorrectly assumed the delivery-spool filename was the chat ID; it failed after the real rejection had already settled. `finish-prepare.mjs` read the actual hashed spool filename and completed the fixture without resending or creating another chat. A hand-written draft initially omitted the real `key`/`failed` fields and produced the product's draft-read error; the invalid file/log were retained, the owned fixture was corrected to the actual schema before meaningful navigation checks, and no production fallback was added.

Native checks in the final source build:

- Pi account and model repair destinations opened the actual existing configuration pages, then returned to the same failed conversation. Models showed the actual owned `openai/gpt-6.1-terra` configuration. No login, credential save, configuration mutation or retry was performed.
- Error401 details appeared in AX. Keyboard Copy followed by paste into the owned unsent composer reproduced the complete original error. The temporary copied draft was restored through actual clipboard paste; automated `typeText` had not inserted its Chinese prefix. This validates clipboard input, not Chinese IME.
- Narrower-window checks exposed that bounding only the error card still left the whole input card oversized. The final change bounds the composer and keeps its footer outside the content scroll area. At approximately592×450 logical pixels/Dark20, attachments, command selector, model/usage and Send remained visible; Send correctly stayed disabled while the original failure was pending. Keyboard reverse traversal revealed Copy inside the nested error scroll, then forward traversal returned to editable input. An appended test character was removed before leaving.
- `/` in the actual input opened the real Pi native-command catalog as an unclipped deferred overlay (`slash-overlay`). No command was selected/executed; Escape closed it and the exact original draft was restored.
- Keyboard opened the account repair route from within the scrolled error area (`composer-accounts`). Model repair was also opened successfully (`composer-models`). One traversal immediately after returning from accounts did not reach the expected target; a fresh visible control was used for the model check. Complete focus restoration across every settings origin is not claimed.
- Final read-only/real-runtime assertions (`pi-receipt.json`) passed: same session, pending1 and identical delivery ID/queue payload, one original user presentation, original error, exact attachment bytes, exact Chinese draft and unchanged owned configuration hashes. No retry/new prompt occurred.

Relevant final UI evidence: `composer-fixed-450`, `composer-error-copy-focus`, `composer-copy-keyboard`, `composer-input-keyboard`, `composer-accounts`, `composer-models`, `composer-returned`, `slash-overlay`. Earlier `compact-*` images document intermediate layouts and do not supersede the final composer screenshots.

## External repair and normal composer regression

Copied the previous real Codex conversation to a separate owned state and set only that copy's saved program path to an owned nonexistent file. Native open produced actual ENOENT. The shared repair action targeted the Codex field (`external-target`); direct typing replaced the loaded path, version probe returned original CLI0.167.0, and UI Save persisted only the app-local repair. Back/Reopen restored the original actual Terra/Medium thread and67/3 history without new inference.

At approximately592×450 with the copied state's light appearance/font, seven unsent draft lines fit the bounded input layout while the footer remained visible (`codex-long-draft`). Keyboard reached Send, which was not activated (`codex-send-focus`); reverse traversal returned to input, and actual paste restored the exact original Chinese/English draft (`codex-restored`). Independent `codex-receipt.json` passed: same native thread, one original user input, one successful tool, one original reply, pending0, idle, repaired saved CLI path and exact original draft. These are history/control checks, not new inference.

All native processes were normally quit and their actual handles returned exit0; both independent state checkers exited0. Originals, failure evidence and owned configuration remain separate from the user's global credentials/defaults. This is not a global byte audit of vendor CLI files.

## Verification and limits

Passed final-source checks:

```
cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop
PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop
git diff --check
```

No UI tests added/changed; no full runtime suite run for this presentation/focus change. Existing backend code, queue protocol, authentication and settings persistence were not changed. Transient-diagnostic repair, built-in Pi open-failure repair, Claude/external Pi native repair variants, all first-project layouts, live-generation queue/Stop after the composer reflow, complete return-focus/VoiceOver/IME/font/window matrices and Windows native remain separate verification work. Actual Pi reauthentication and Claude inference remain open. Current ZIP was not replaced by this source slice; the full goal remains active.
