# Member launch-model selection — 2026-11-10

Baseline `4d53de5`. Source/debug UI validation; canonical current/candidate ZIPs remain `edbbae1`. Evidence: `.artifacts/member-model-4d53de5/`.

## Change

The member launch-model list previously had an untracked fixed-height scroll area. It now reuses the existing nested Viewport/Reveal helpers and a dedicated ScrollHandle, including each model option. The page reveals the model trigger, search and bounded model viewport; the inner list reveals the focused model. Opening resets list offset and focuses search after layout. Selecting a model or closing the list returns focus to member search. Loading, empty and failed catalog text use the existing accessible/selectable text helper.

This changes native presentation and focus only. Backend discovery, available-model filtering and the actual launch-config write remain the existing implementation. No permission layer, inference fallback or vendor protocol was added.

## Real-source and native evidence

Used a private copy of the preceding owned thirteen-member fixture. Preparation assigned `hive:codex` to only `Added-by-UI` and saved original Codex `/Users/tsk/.npm-global/bin/codex` (0.167.0) in this copy. `AgentModels.list` opened the actual app-server catalog and disposed it without prompting: ten available native entries, including `gpt-6.1-terra`. Initial preparation used a nonexistent collection URL and received HTTP404; it was corrected to the existing UI-team endpoint. Both logs are retained; no production endpoint/fallback was added.

Actual Mac wrapper used source runtime plus the current bundle's Node explicitly:

1. Opening the picker allowed immediate typing into its search before/after catalog arrival. Filtering `terra` yielded the actual two matching entries.
2. At Dark20 and approximately592×500 logical pixels, ten Tabs from model search reached the final catalog option with a visible focus ring and inner/outer scroll adjustment. Ten reverse Tabs returned to search (`final-last-model`). The last option was traversed only, never selected or run.
3. `no-owned-match` produced accessible `No matching available models` (`empty.ax.txt`); loading text also appeared in AX.
4. Filtering `gpt-6.1-terra`, Tab then Return saved exactly `openai/gpt-6.1-terra`. Picker closed; direct typing entered member search (`saved-terra`). The stopped member then exposed Start, which was not activated.
5. Normal quit/cold launch retained the selected Terra launch configuration (`cold`). The final text-accessibility build repeated last-option traversal, reverse traversal, filtering and Terra save.
6. A separate run set only its process-local `HIVE_CODEX_COMMAND` to an owned nonexistent path. Opening the picker exposed actual spawn ENOENT via accessible text (`error`). Closing it retained the saved Terra choice and returned typing to member search (`error-collapsed`). During this check an unsaved Add form was opened by a stale AX index and immediately closed; no submit occurred. The saved program setting was not overwritten by the environment override.
7. Native runs exited0. Read-only SQLite assertions (`state-check.json`) found the expected `hive:codex` command and exactly `['--provider','openai','--model','gpt-6.1-terra']`; thirteen workers remained. Messages, dispatches, agent runs, agent sessions and workflow runs all remained0; owned Pi auth absent/empty. No inference request or worker start occurred.

## Checks and remaining scope

Passed Mac build, Windows target check and `git diff --check`:

```
cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop
PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop
```

No tests were added or modified for this UI/focus slice; no full runtime suite was run. Windows native, complete VoiceOver/IME/font/language matrices and vendor-specific catalog failures beyond actual Codex ENOENT remain unverified. Native model discovery is not proof of inference, vendor authentication, or collaboration acceptance. The full desktop target remains open.
