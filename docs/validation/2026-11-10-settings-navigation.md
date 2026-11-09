# Short-window settings navigation — 2026-11-10

Source baseline `1f308d9` plus this navigation fix. Evidence `.artifacts/light-large-1f308d9/`. No new inference; reused an owned completed original Codex / Terra thread. User/global configuration unchanged.

## Observed defect and implementation

At approximately 822×450 logical pixels, English, Light theme and 20px interface font, settings used a fixed vertical list with no overflow handling. External programs was cut at the bottom, Role templates was below the viewport, and Models and providers was clipped at the right (`settings-before.png`). AX listing an offscreen button did not prove visual reachability.

The existing navigation now has its own ScrollHandle and reuses the existing Viewport/Reveal focus geometry helpers. Only the tab list scrolls; Back and the settings heading remain fixed. Rows keep their height, long labels truncate deliberately, and full accessible labels/tooltips remain available. Existing section handlers, compact dropdown, data and permissions are unchanged. No dependency, runtime/protocol or test changes.

## Actual native verification

- Launched the fixed development app with the same isolated data/Pi directories; restored English/Light/20px. Normal-sized window showed all nine sections. Resized again to approximately 822×450 (`fixed-top.*`).
- Clicked settings search, Shift+Tab: Role templates automatically scrolled into view with its focus outline fully visible (`fixed-bottom-keyboard.*`). Return actually opened the role template name/responsibilities form (`fixed-roles.*`); no template created.
- Clicked search, nine reverse Tabs: Appearance automatically became visible with its focus outline (`fixed-reverse.*`). Back stayed fixed at the top.
- Actual mouse wheel over the navigation exposed External programs and Role templates (`fixed-wheel.*`).
- Resized to approximately 632×450: existing compact dropdown still offered all nine sections with the current check mark. Selected Appearance through that menu (`compact-menu.*`).
- Actual Back returned to the conversation with the exact original two-line Chinese/English unsent draft (`returned-draft.*`). No send/start/stop or account write.
- Cmd+Q exited normally, exec session 83081 returned 0. Independent cold runtime read passed (`check-cold.mjs`, `cold-receipt.json`): Light/20px/English, exact draft, same Terra/Medium thread, pending 0, same two user inputs and ordered replies, only the prior actual command result. No model turn requested.

## Checks and limits

Passed `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` with LLVM on PATH, and `git diff --check`. Initial compile errors were missing scroll element identity and invalid tuple ID; corrected before the successful build/native run. Accidental unrelated rustfmt-only changes were removed; final product diff is confined to navigation/settings.rs and the dedicated app scroll field.

Pure UI layout/focus fix: no fragile UI tests or unrelated full runtime suite. Actual native keyboard, wheel, activation and read-only cold verification cover this case. Other languages/themes/font sizes, VoiceOver narration and Windows native execution remain separate acceptance. Current candidate ZIP is still `b539c26` and excludes this fix. Full product goal remains active.
