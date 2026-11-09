# Workflow keyboard navigation — 2026-11-10

Baseline `62abe27`. Source/debug UI validation; current/candidate Mac ZIPs remain `edbbae1`. Evidence: `.artifacts/workflow-keyboard-62abe27/`.

## Change

Workflow list and detail scroll areas now use the existing Viewport helper. Stable Reveal groups cover filters, run cards, executor setting controls, schedule actions/confirmation, detail copy controls, dispatch artifacts and dispatch navigation. Existing separate list/detail scroll handles and polling remain unchanged. Opening workflows or changing list/detail requests focus on the Refresh toolbar after its read completes; ordinary polling does not request focus. Cancelling schedule deletion returns focus to that toolbar instead of retaining a removed confirmation control. Read/action failures use existing accessible/selectable text.

This is a presentation/focus change. No Hive protocol, schedule, runner, execution policy or business store was added or changed.

## Fixture and actual Mac flows

Created a SQLite backup of the earlier owned `.artifacts/workflows/state` into the new evidence directory. No chat index or delivery queues were copied. Preparation used the real runtime/HTTP endpoints to verify nine non-running workflow records and pause the single schedule in the copy. Historical completed, failed, stopped and interrupted runs, actual logs and a reported dispatch were retained. The dispatch explicitly identifies its report as test-driver submitted, not model output. Original data and artifact directories were preserved.

Used source runtime and the current bundle's Node through an explicit debug wrapper. At Dark appearance,20-point text and roughly592×500 logical pixels:

- From the ready Refresh toolbar, fourteen Tabs reached the ninth run with visible focus and automatic list scrolling (`last-run`). Return opened the intended completed result.
- Three Tabs reached the detail's source/parameter Copy action at the bottom, automatically revealing it (`detail-metadata`). Actual Copy then paste into the owned project's unsent composer yielded the exact source path, hash and JSON arguments (`metadata-copy`); the temporary draft was cleared without sending.
- Keyboard traversal opened executor settings and reached the Pi default control in the eight-executor list. Selecting the already-allowed Pi then traversing to Save persisted `{"default":"pi","allowed":["codex","pi"]}`, independently read from SQLite. Reverse traversal then restored Codex as default and saved the original policy. No CLI or model was run by these configuration changes (`policy-save`).
- Keyboard traversal reached the final schedule's Delete entry, then its confirmation Cancel with visible focus (`schedule-cancel`). No confirm/delete was executed. In the final build, Cancel returned to Refresh: the next Tab focused and revealed the feature-toggle row (`cancel-return-focus`). The feature toggle itself was not activated.
- The final build opened the historical reported-dispatch run via keyboard. Tab revealed the artifact Preview (`artifact-focus`); Return opened the actual disk text `Owned workflow UI verification artifact.\nNot model output.\n` (`artifact-preview`). Escape then Tab/Return opened the matching reported dispatch. Back to workflows restored the original run detail.
- During a diagnostic batch, Tab presses were sent before the list-read/focus handoff had settled, so they opened another existing completed run rather than the intended schedule. No mutation occurred. The final schedule check waited for the actual ready list before traversing; the focus handoff is not a guarantee that keystrokes during an incomplete read are queued or replayed.
- Both native runs exited0. A second process launch read the already-restored configuration. Read-only SQLite comparisons (`state-check.json`) found all nine run results/statuses, logs, dispatches/reports, messages, agent runs and agent sessions identical to the source backup. One schedule remained paused and retained. Original Codex default/allowed policy was restored. The chat index stayed empty and no auth file was created. No inference or new Agent run occurred.

## Checks and limits

Passed:

```
cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop
PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop
git diff --check
```

No UI tests added/changed and no full runtime suite run for this UI-only change. Final destructive schedule deletion, live-run Stop under this new layout, new failure-banner native reproduction, member-chat navigation with an actual copied chat index, huge-history performance, full VoiceOver/IME/language/font matrices and Windows native remain outside this slice. Previous runtime/Stop evidence is not expanded by this check. Issue13 and the full desktop goal remain open; no package promotion occurred.
