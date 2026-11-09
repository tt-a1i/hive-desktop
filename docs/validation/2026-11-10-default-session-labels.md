# Default session labels in English — 2026-11-10

Related #17, source baseline cd0148f. Evidence `.artifacts/untitled-label-cd0148f/`. Development app; no new package or model inference.

## Problem and implementation boundary

The preceding real attachment flow captured a new Codex record in an English interface as `Codex · 新会话 · Conversation` (`file-picker-return-4ceb096/attachment-session-ready.txt`, retained as this fixture's baseline.txt). Runtime createRecord deliberately stores this default title and first send recognizes it for automatic naming. Passing a translated title to runtime or translating arbitrary stored titles would change those semantics.

Added one shared UI label rule: only a non-custom, unbound record with the exact existing default title gets the current locale's New conversation label. Current runtime binds records to an Agent before delivering their first user turn. Explicit custom titles and bound conversation titles remain verbatim. Existing member-name precedence remains unchanged. Applied that same rule to ordinary navigation/title readouts, pinned navigation and search-result text/accessibility labels. Added the English locale key. No metadata migration, stored-title write, send/automatic naming change, runtime API or new state/dependency.

## Actual native verification

Owned clone of the prior real Codex attachment state; isolated Pi directory, original Codex CLI through per-process environment. Two owned native runs, both normal Cmd+Q exit 0.

- English new Codex record displayed New conversation before any send. After native Pin conversation, both the pinned and workspace rows showed New conversation (`english-new.*`, `pinned-english.*`). The record remained unbound with the original stored Chinese default and no native thread ID or deliveries.
- Opened the existing real bound conversation. Its original automatic English title displayed unchanged before the rename. Used native Rename and explicitly saved exactly `新会话`; in the same English interface, that custom title stayed Chinese while the default record stayed New conversation (`custom-chinese-english.*`). This is an explicit edit only in the cloned fixture; the original evidence's record remains untouched.
- Cmd+K unfiltered search listed Open New conversation and Open 新会话 consistently (`search-labels.*`). This verifies result labels, not searching the localized English alias against the canonical stored title; query matching is unchanged.
- Actual Appearance language switch to Chinese updated the default/pinned label to 新会话; the explicit same-text title remained unchanged. Switching back restored only the default/pinned English label (`chinese-labels.*`, `english-again.*`). No locale value is baked into cached hit titles.
- Full exit/reopen restored English, pinned New conversation, explicit 新会话 and the prior AFTER_ATTACHMENT_UNSENT draft (`cold-labels.*`). Independent index checks confirmed the default title is still stored as 新会话 without customTitle, while the explicit rename is customTitle true. Prior native Codex cache and display projection files matched original fixture bytes exactly; the only new cache had zero deliveries/completed turns. SQLite message/run/session/dispatch counts matched the baseline. `check-state.py`, `receipt.json`.

## Checks and limits

Passed final macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, LLVM PATH `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, unique English locale keys, `git diff --check` and the actual native/index/cache/draft assertions above. Pure UI presentation change: no new fragile UI tests, deleted tests or unrelated full runtime suite. No inference used an unintended model; there was no new inference at all. Runtime first-send naming code was left untouched, rather than relabeling a new model run as verification.

This does not complete #17 or full title/search localization: localized-alias search, every legacy imported record/role label, full theme/font/IME/VoiceOver matrix and Windows native remain separate. Candidate still 34aebbe and does not include this fix. No large bundle copied, deleted or regenerated; original workdirs/evidence preserved. Complete product goal stays active.
