# Search model configuration — 2026-11-10

Source baseline `bbbcb30` plus this change. [Task #23](https://github.com/tt-a1i/hive-desktop/issues/23), part of full goal #1. Evidence `.artifacts/model-search-bbbcb30/`. Actual development app + existing real runtime/SDK/config files; no model request, user credential or global-default change. Candidate remains source fb3dcd0 and does not include this change or the preceding list-focus fix.

## Result and scope

Model configuration previously required browsing every custom model/override. Added an app-local temporary search using the existing input/snapshot/list handlers. Matches provider ID/name/Base URL, model ID/display name and provider/model identity; trimmed, case-insensitive substring comparison. Provider match retains all its models; model-only match keeps its parent and matching model rows. Counts describe this configuration snapshot, including overrides, not native model availability or account readiness. Clear restores the complete list and returns input focus.

Search stays above the scrolling list and is hidden while the existing form is open; query survives editing and settings-section navigation, but is not written to config or restored after application restart. Existing provider/model edit, add, remove, validation and conflict/persistence paths are reused. English/Chinese labels, count plurals and explicit empty results included. No runtime query/store/dependency added.

At Dark/20px and about 592×500 logical pixels, the duplicate content heading left too little room to see the empty-result explanation. Compact settings already display the selected section in their navigation button: removed only the duplicate compact heading and reduced compact content spacing. Wide settings retain heading/spacing. Actual narrow Models/Appearance and wide Models were inspected; no general all-page/VoiceOver claim.

## Actual native cases

Owned copy of three providers/34 synthetic configured models/overrides; copied only models/settings, no auth. Backend SDK subsequently created an empty auth object.

- Ordinary keyboard `  LAYOUT-MODEL-030  ` and `  LAYOUT-LOCAL/LAYOUT-MODEL-030  `: 1 provider / 1 model, exact layout-local/layout-model-030 action. Native keyboard reached visible Model settings and Return opened that exact ID.
- Base URL `127.0.0.1:9`: 2 providers / 32 models. Provider ID `openai`: 1 provider / 2 models. `openai/gpt-6.1-terra`: exactly that override; `Preserve override`: only openai/layout-untouched. No unrelated model actions remained in those filtered AX snapshots.
- Chinese provider query `窄窗新增` through actual plain-text clipboard paste: narrow-new / one model. This is clipboard verification, not Chinese IME composition.
- Unmatched query: 0 providers / 0 models, visible explanatory text and Clear. Keyboard Tab/Return on Clear restored 3/34 and input focus. Searches alone left complete models.json byte-identical (`search-only-preserved.txt`).
- Final native form editing changed only target name to `Search edit proof · 中文`, using actual paste and Tab/Return on Save. The original ID query remained 1/1 after save. Exact full-object comparison confirmed only layout-local/layout-model-030.name changed; all other models, providers, headers and overrides preserved (`save-receipt.json`). Searching the new name returned the saved target.
- Actual section-menu navigation to Appearance, switching this owned app to Chinese, and back to Models preserved the query. Search placeholder/label, counts, Clear and empty text changed to Chinese. Wide/default 16px layout retained its original section heading and complete search/count controls (`chinese-*.ax.txt`, `wide-default-final.*`).
- Final complete exit/reopen reset temporary query, showed all 3/34, then new-name search again located the correct saved ID. Open form read the exact saved name, Save disabled; closing created no draft (`cold-baseline.*`, `cold-result.*`, `cold-form.*`).

## Real dirty-state defect found and fixed

The model-030 fixture's existing display name ended in whitespace. Before any edit, opening it enabled Save and later created a local draft (`false-dirty.*`, `false-dirty-draft.json`). Initial Values retained raw whitespace while existing `values()` trimmed input for dirty comparison. Corrected the common form-opening path to capture initial Values through that same existing reader after populating inputs; input text and persisted source remain untouched.

Final native verification opened the same unmodified trailing-space name: Save disabled, direct Close returned without a discard prompt, full models.json bytes unchanged and no draft created (`clean-form-final.*`, `unchanged-close-final.*`, `unchanged-open-close-preserved.txt`). Subsequent genuine edit/save and complete restart behaved as described above. Preserved the false-draft evidence, then cleared only that owned erroneous fixture draft before fixed verification; no user draft was removed. Existing recovery/validation/conflict rules unchanged.

## Checks and limits

Four owned native processes (execs 43233, 99315, 56358, 13303) exited normally with Cmd+Q, all exit 0. Final source passed `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, LLVM PATH `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, unique/valid new locale keys, `git diff --check` and real configuration/draft/auth assertions (`receipt.json`). Initial experimental search layout was adjusted before final verification; final builds have no new warning.

UI search/layout/dirty comparison only: no fragile UI tests or existing test changes, no unrelated full runtime suite, no runtime/protocol/schema change. Actual edits went through existing real SDK/config persistence. Windows native, full keyboard/VoiceOver/IME/font/theme matrix, every settings section and refreshed candidate remain separate acceptance. Existing candidate ZIP/current ZIP unchanged. Full product goal remains active.
