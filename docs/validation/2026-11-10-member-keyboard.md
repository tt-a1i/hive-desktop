# Member page keyboard and compact-window flow — 2026-11-10

Baseline `9da157b`. Source/debug UI check only; current and candidate optimized ZIPs remain `edbbae1`. Evidence is in `.artifacts/member-navigation-9da157b/`.

## Reproduced problem and change

With twelve dormant members, Dark appearance, 20-point text and a roughly 592 × 500 logical-pixel window, focus could reach a member menu outside the viewport. Rename then opened an invisible input. The before snapshots (`before-offscreen`, `before-hidden-rename`) show this on the current package.

The member page now uses the existing focus-scroll helpers, with stable member-ID reveal groups for actions, descriptions and inline editors. The whole page scrolls so that filters, validation text and the creation form cannot permanently consume the viewport. Its outer full-height container bounds the viewport; without this wrapper, an intermediate implementation allowed intrinsic content height to escape the horizontally centered parent. The member rendering moved out of the root view into `members/view.rs`; collaboration/runtime APIs did not change.

Entering the page focuses search. Rename focuses its input on the next native frame, after the editor exists. Valid rename/create clears the old validation notice. Save/create and explicit editor cancellation return focus to search. New-member form controls reveal as keyboard focus moves through them. Existing create semantics still use `autostart: false`.

## Actual native checks

An owned real runtime/HTTP/SQLite fixture created twelve stopped members (`coder` role). The first preparation attempted unsupported `developer` and received HTTP 400; the corrected preparation reused the existing owned workspace. No global account or model selection was changed. The debug wrapper used the source runtime and current package's bundled Node explicitly; it is not package verification.

- From search, fifteen Tab presses reached the sixth member's menu with a visible ring and automatic scroll (`bounded-sixth`). The actual immediate Rename input was visible (`bounded-rename`).
- Empty-name submission displayed the real validation notice. A subsequent valid keyboard save changed only the sixth member to `Reviewed-06-final`; the stale notice disappeared. Direct typing after save entered search.
- Filtering, keyboard responsibility expansion and Copy succeeded in the compact window. Pasting the product clipboard into search yielded exactly `Owned dormant navigation fixture 6. No execution or inference.` (`bounded-copy`).
- Reverse keyboard traversal revealed the header. Add member focused the name input. Keyboard traversal through name, responsibilities, template, four roles and four backends revealed the final submit button. Actual submit created `Added-by-UI` with the owned description, stopped and without a selected launch model (`bounded-created`). Direct typing entered search after creation.
- Delete confirmation was opened but not confirmed. Keyboard traversal revealed Cancel; Return cancelled and subsequent typing entered search (`bounded-cancel`). No member was deleted.
- Normal quit and cold launch retained both `Reviewed-06-final` and `Added-by-UI` (`cold-rename`, `cold-added`). Final native runs exited 0.
- Read-only SQLite verification (`state-check.json`) found exactly thirteen workers: original twelve IDs/roles/descriptions preserved, only the intended name changed, and exactly one intended new member. Messages, dispatches, agent runs, agent sessions and workflow runs were all zero; no worker had a last session. Owned Pi auth remained absent/empty. No inference request was sent.

## Verification and limits

Passed:

- `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`
- `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`
- `git diff --check`
- Native flows and persisted-state assertions above.

No UI test files were added or changed; no full runtime suite was run for this presentation/focus change. Member model-picker nested scrolling, all language/font/window combinations, complete VoiceOver/IME behavior and Windows native execution remain outside this check. The full product target remains open. Upstream T3 Code/Beluga/Cindy remain experience references; this change reuses Hive Desktop's own scroll helpers and includes no copied upstream source or assets.
