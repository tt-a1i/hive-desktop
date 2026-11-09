# Model list keyboard reveal — 2026-11-10

Source baseline `c9fd192` plus this UI fix. Related #20. Evidence `.artifacts/model-dark-large-c9fd192/`; reproduction used the independently extracted fb3dcd0 candidate, fixed verification used the built development app. These are different binary scopes; candidate ZIP has not been refreshed for this fix.

## Actual problem

English, Dark theme, 20px interface font, approximately 592×500 logical pixels. Owned copy of prior synthetic models/settings, no auth copied or model call. The form controls already used focus reveal, but list toolbar/provider/model/add actions did not. From settings search, six actual Tabs reached the first model edit action outside the viewport; the list did not scroll and no focused action was visible (`offscreen-focus-before.*`). Return actually opened gpt-6.1-terra with its original name/context, proving the offscreen focus target (`opened-from-offscreen.*`). AX listing all actions alone was not counted as visual reachability. Reproduction left models.json byte-identical; native exec 89006 quit normally, exit 0.

## Fix

Reused existing Reveal/Viewport and ScrollHandle around the list toolbar, each provider header/address group, each model row and each provider's Add model action. Rows retain content height; a small list inset protects focus outlines. Stable identities use provider/model IDs. Existing keyboard activation, form handlers, validation, JSONC configuration and authentication semantics are unchanged. No new dependency, protocol or general scrolling abstraction. No interception of editor keys.

## Actual fixed verification

- Same window/language/theme/font and configuration. Search → six Tabs now scrolls to the visible first Model settings action with full focus outline (`first-focus-fixed.*`).
- Seventy-four additional Tabs traversed the long list to final provider's Add model; its focused button appeared at the bottom (`last-focus-fixed.*`). Seventy-four reverse Tabs returned to the original model edit action (`reverse-focus-fixed.*`). No remove/add action activated.
- Return opened the original model form. Entered invalid `bad` context, four Tabs to Save, Return: the existing validation message and fixed Save/Close remained visible (`invalid-save.*`). Independently verified the complete models.json remained byte-identical (`invalid-save-preserved.txt`).
- Four reverse Tabs revealed the context label, invalid value and focus outline despite the extra error footer (`correct-field-focus.*`). Replaced it with 32768 and used four Tabs/Return to save. Exact object comparison confirmed only layout-local/gpt-6.1-terra contextWindow changed 16384→32768; other models/providers, overrides and headers preserved (`save-receipt.json`).
- Returned to first edit through search/Tab. Actual wheel moved back to the toolbar and stayed there (`manual-scroll.*`); next Tab to the remove action in the same model row automatically revealed that focused action (`same-group-reveal.*`). Remove was not activated.
- Native exec 64034 quit normally, exit 0. Final source was rebuilt after removing unrelated rustfmt-only changes; final macOS build and Windows MSVC target check passed.
- New native process (64362) with final binary restored Dark/20px/English, reached the first model from search through visible actions, opened it and read 32768 with Save disabled, no unsaved notice (`cold-read.*`). Normal quit exit 0. Independent disk assertions verified exact saved object, empty form drafts, unchanged Dark/20px preferences and empty SDK-created auth object. `receipt.json` records source hash and scoped results.

## Checks and limits

Passed final `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, LLVM PATH `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, `git diff --check` and the above real native/file assertions. Pure UI focus/layout change: no new fragile UI tests, test deletion, runtime change or unrelated full suite. Normal editor persistence uses the existing real runtime/SDK/model file path.

This covers this list's forward/reverse reachability, manual-wheel coexistence and existing form error/save/cold-read at Dark/20px/English. Full VoiceOver narration, other font/theme/language combinations, Windows native and new candidate inclusion remain open; #20 and complete product goal remain active. No new ZIP or large copied bundle was made for this small fix; existing candidate remains source fb3dcd0.
