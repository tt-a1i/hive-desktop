# Native folder picker ownership and first input — 2026-11-10

Fix for #24, baseline `34c1396`. Evidence `.artifacts/picker-focus-34c1396/`. Source development app, not the 34aebbe candidate ZIP. Original full-client goal remains open.

## Root cause evidence

Repeated the exact owned fresh-state English/Light/20px/~592×500 flow: keyboard Open project → native Go-to field explicitly clicked/typed → confirmed correct folder → mouse Choose project folder → actual first text input without a field click. It again did not enter the draft (`diagnostic-input.*`). Temporary native state logging showed the focus request when loading=true/false; all three samples had `active=false`, including `loading=false active=false sidebar=false visible=true input_focused=false` (`native-diagnostic.log`). The owner was no longer the OS keyboard window even though the project had loaded. Treating this as only a field focus/loading problem had not repaired it.

Read the actual installed GPUI macOS implementation: prompt_for_paths uses a standalone NSOpenPanel beginWithCompletionHandler, sends its receiver result, and does not reactivate the captured application window. Hive's completion callback likewise did not reactivate it. This code observation and the native active=false samples support the owner-window diagnosis; it is not inferred from an AX focus label alone.

## Small shared fix

Await the existing receiver, then return activation to the originally captured Window using GPUI's native activate_window before processing its existing result. Apply the same handoff for selection, cancellation and errors; only while the same backend connection still owns the flow and the app is not closing. Existing path/error handling and successful project input-focus request remain unchanged. No timer, key replay, global app activation loop, test-only fallback or new dependency. Removed all temporary logging before final builds. Also renamed the previously unused callback cx argument to _cx, clearing that warning.

## Final real verification

- New independent state, same theme/font/window conditions and final source. Default-focus Return opened the native picker. Mouse Cancel returned to welcome; immediate Return reopened it without another app click. This specifically exercises mouse dismissal, not only Escape (`fixed-cancel-return.*`).
- Explicitly clicked the native Go-to field, typed and observed the entire owned `project-final` path, navigated there and observed Where: project-final. Mouse Choose opened the correct project.
- With no field click or extra input attempt, actual first `PICKER_FIXED_FIRST_UNSENT` immediately appeared in that project's composer (`fixed-first-input.*`). It was not sent. This is the same action sequence that failed in the diagnostic run.
- Normal Cmd+Q exit 0; new native process restored that exact project path and entire draft without a model/session start (`cold-draft.*`). Normal Cmd+Q exit 0 again. Diagnostic process had also exited 0.
- Independent SQLite/file checks: exactly one intended workspace, zero messages/dispatches/agent runs/agent sessions, no chat index and exact persisted draft. The owned Pi directory has no auth file; an initial assertion incorrectly expected an empty auth file, then was corrected to the actual absence without creating one. No credentials, global settings or model request used. `check-state.py`, `receipt.json`.

Final `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, LLVM PATH `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, `git diff --check`, native actions and independent state assertions passed. Both final build logs have no warning. Pure UI/native focus handling: no new fragile UI tests, changed existing tests, backend protocol change or unrelated full suite.

This closes the reproduced #24 owner/first-input issue, including cancellation/reopen and cold draft preservation. It does not establish Windows native picker behavior, forced OS picker-error behavior, every language/theme/input method, signed installation, new candidate inclusion or full product acceptance. Candidate/current ZIPs and old states/workdirs retained; no large bundle copy or cleanup in this slice.
