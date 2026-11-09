# Main layout and sidebar containment — 2026-11-10

Baseline 52c7b65; evidence `.artifacts/layout-reference-52c7b65/`. This pass changes native layout, not collaboration semantics. Full product goal stays open; current candidate 34aebbe is unchanged.

## Reference and actual defects

Read-only observations of installed T3 Code's existing new-thread page and Beluga's welcome: a compact navigation utility row, clear primary work area, and restrained empty-state hierarchy. No thread, onboarding step or setting in either reference app was changed. An ambiguous T3 bundle identifier initially matched installer and installed copies; inspection used `/Applications/T3 Code (Beta).app`. Local reference screenshots contain existing sidebar content and remain ignored artifacts, not published assets. No upstream code, assets, branding or copy imported.

Hive baseline used its default 16px font, English/Light, real prior Codex history and the same window dimensions as the after capture (2360×1706 physical pixels, checked with sips). Its long session row visibly extended across the sidebar divider into the conversation, as did menu placement (`chat-before.*`, `home-before.*`). This regression followed the earlier focus-scroll viewport wrapper: its flex child lacked horizontal shrink constraints. The sidebar also spent three separate rows on New/Search/More; project starters stacked vertically and the secondary team label competed with the main heading.

## Changes

- Constrained the existing workspace-navigation scroll child with min_w_0/w_full. An isolated build containing only that constraint already kept long labels and menus within the sidebar (`width-only.*`). No per-title cropping, stored-name edit or loss of tooltip/accessibility text.
- New, search and more now share one utility row. Team and Collaboration remain labeled, always-visible sidebar actions. Existing menu contents, selected states, shortcuts, handlers and accessible names retained; compact icons keep tooltips.
- Project start page uses a lighter heading, icon-only accessible/tooltip team action, and wrapping horizontal starter actions. Actions still append existing prompts to the draft and focus input; they do not send. Typography preference, palette, model controls and runtime unchanged.

## Actual final native checks

- Default Light/16 before/after screenshots have the same physical dimensions. Workspace/session titles truncate within the sidebar; the primary project area and single-row starters are visible (`home-after.*`). Native New still offered all four Agent backends; it was closed without creating another record.
- Mouse search opened actual results. More retained workflows/memory/terminal/archives; selecting Memory opened real workspace memory. Project's icon team action opened the real member panel. Returned through existing navigation without edits or Agent actions (`search-after.*`, `more-after.*`, `memory-route.*`, `home-team-route.*`).
- Actual Explore this project populated only the expected draft. Eleven Tabs from that known focused input reached visible More with a full outline; Return opened its menu. Escape/reverse Tab/Return opened search (`starter-action.*`, `keyboard-more-focus.*`, `keyboard-search.*`). No Send/Enter action was applied to a composer.
- Changed only the owned fixture to Dark/20px. Wide view stayed contained. About 592×500 logical pixels: full-page navigation, compact search and main project view inspected; starter actions wrapped and input controls remained visible. Selected only the empty test conversation's model as Terra without sending (`dark-wide.*`, `narrow-navigation.*`, `narrow-search.*`, `narrow-terra.*`). Earlier default model readouts were metadata, not model calls.
- Pinned the actual long historical title through its visible row menu. Narrow and wide pinned/workspace rows remained within the sidebar, with full accessible names (`pinned-narrow.*`, `pinned-wide.*`).
- Full quit/reopen restored Dark/20px, compact controls, pinned history, Terra selection, exact starter draft and prior AFTER_ATTACHMENT_UNSENT draft (`cold-layout.*`). Four native processes exited 0: baseline, isolated constraint build, final interaction pass and cold run.
- Independent `check-state.py`: original native Codex cache/projection bytes unchanged, no additional messages/runs/sessions/dispatches, new record unbound/no native thread and all new derived caches have zero deliveries/completions. An initial assertion assumed one new cache file; metadata/model/layout reads actually left four empty derived caches. Corrected the artifact assertion to verify every cache's real contents and record its count. No new inference. `receipt.json`.

Final current-source Mac build, LLVM PATH Windows MSVC target check, diff check and actual native/file/SQLite assertions passed. Pure UI layout: no fragile UI tests, existing test changes or unrelated full suite. No new large bundle or deletion; old states/workdirs preserved.

This is a visible layout improvement and regression repair, not full T3/Beluga/Cindy parity. Windows native, full VoiceOver/IME/language/font/theme/history/member matrix and refreshed package inclusion remain open. Reference application sizes differ, so only Hive's same-size before/after frames support the layout comparison; source and platform evidence are not interchangeable.
