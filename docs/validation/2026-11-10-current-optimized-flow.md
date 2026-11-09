# Current optimized Mac development package — 2026-11-10

Native/runtime source **edbbae16af01a342272d839a668a2fc99d676e4d**. Current Mac ARM64 development package, not a signed installer or full product completion. Evidence root `.artifacts/release-edbbae1/`.

## Artifact and integrity

- Current ZIP `.artifacts/hive-desktop-macos-arm64-current.zip`; candidate alias has identical bytes and is hardlinked, without a second full ZIP allocation.
- ZIP **143,400,169 bytes**, SHA256 `085354108c29b318a0b6b8fd0f438fe3793dd7c62918cc7e485d59da8d91eded`.
- Native SHA256 `4638605b763f63c3659f51948aeaa0124b105730d03d40d6644573fa330c7d41`.
- Runnable app `.artifacts/release-edbbae1/extracted/bundle/Hive Desktop.app`.
- `cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`: exit0, optimized, 34.61s incremental build. No memory/startup performance claim.

Frozen production source remains1534986f992fe011cef94706618bf2ee2acf40df. Compared with252c19c, only the native executable changed; all112 runtime files, dependency entries and metadata matched. ZIP CRC, independent extraction, 28,863 entries/modes and29 internal links matched; post-GUI inventory remained unchanged. Latest source UI includes the four Agent setup entries, asynchronous field edit protection and loaded-field focus/selection handoff.

## Fresh native flow with real inference

No source Node/runtime/Codex overrides. The owned process record proves native app → its bundled Node/runtime → original Codex. Fresh empty app/Pi state and owned `project/numbers.json` containing `{"values":[13,19,35]}` plus newline.

1. Actual welcome Codex card → loaded targeted field → type original `/Users/tsk/.npm-global/bin/codex` → actual version0.167.0 → save → welcome reports detected version without unsaved qualification. No account-ready assertion.
2. One native folder Choose created the correct project. Direct typing populated the project draft; native Agent menu moved it into the new Codex conversation. No409 in this observed selection; the older picker observation is not thereby explained.
3. Explicitly selected Terra/Medium before one native Send. Initial Sol metadata made no inference request. Requested one delayed Node command reading the file, then independent sum/count; expected numbers were not in the prompt.
4. UI observed the actual tool in progress/completed and final **RELEASE_EDB_OK 67/3**. During execution entered `完整包内草稿 · RELEASE_EDB_UNSENT` without sending/queueing it.
5. Normal quit/cold launch restored original reply, tool and exact draft; both native exits0. Bundled Node/runtime `check-cold.mjs` verified same thread, one user input, one command result/exit0/exact output, one exact reply `RELEASE_EDB_OK 67/3`, Terra/Medium, pending0, original saved CLI/version and unchanged input.

Session `6a790fba-b9bf-49e9-8fb5-53bd11ee1ec8`; native thread `01a1c1db-03cb-74c2-8431-1329f1f2f7ab`. Native AX/screenshots, process record, cold script and receipts retain the evidence. This round sent **one** actual Codex/Terra request; cold checks sent none.

## Package gates and the initial failure

Final exit0 for `check-package.mjs`, `check-packaged-models.mjs`, `check-packaged-dispatches.mjs`, `check-packaged-codex.mjs`, `check-packaged-remote.mjs`, and `check-packaged-claude.mjs`, each with `--bundle .artifacts/release-edbbae1/extracted/bundle`. Scope remains actual package Node/HTTP/SQLite/PTY/SDK and synthetic CLI/local gateway where specified, not vendor Claude inference or production remote acceptance.

Initial Claude gate exited1 in the early-exit case: first input was durable and the synthetic CLI exited before followup acceptance; the second request got real HTTP409 No active run. The check expected an already-queued followup without synchronizing that prerequisite. Preserved log and complete failing synthetic state under `claude-first-failure/`; an older shared output receipt was not treated as proof of this run.

Test-only commit **c151d137ba0b1b55374116119157774a57af7b41** adds a file-watcher handshake: all four scopes first assert two actual accepted inputs and exact queued originals, then release the result/exit. The fixture's one-shot completion and stdin-end cleanup prevent duplicate completion/retained watcher. No fixed delay, production fallback/retry or swallowed409. All previous loss/completion, paused queue, cold recovery, explicit resume and exact native-query counts remain asserted. A first early-exit-only gate passed; the final shared handshake passed both package and source checks, plus `pnpm check` and fixture syntax check. No whole-suite run: application production bytes did not change during this test repair.

## Promotion, storage and remaining scope

Promoted to current after gates, real fresh flow, normal exits, cold assertions and post-GUI inventory. Old current1acd805 archived as `.artifacts/current-package/1acd805-current.zip`, with historical receipt `receipt-1acd805.json`; old candidate252c19c archived in its original evidence directory. Both archive hashes reverified. Removed only the unused new assembly base and superseded252c19c extracted bundle after lsof checks: **1,076,756,480 allocated bytes**. All projects, states, failing/passing evidence and ZIPs preserved. Physical free-space delta was not measured.

Current/candidate receipt pointers agree. Unsigned/unnotarized; Windows native unavailable, full accessibility/IME/performance/other-backend matrices, actual Claude model collaboration, Pi reauthentication, production remote/phone flows and formal installers remain open. Fresh Codex success does not close the full goal or prove every feature/reference interaction has been repeated in this package.
