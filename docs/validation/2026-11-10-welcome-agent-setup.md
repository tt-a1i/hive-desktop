# Agent setup entry and input preservation — 2026-11-10

Baseline `a8ce080`. Source/native UI change; canonical release candidate remains 252c19c. No upstream code/assets or new dependency were copied.

## User flow

A first-time user with a preferred external Agent previously had a generic setup menu and a separate external-programs link, then had to locate the correct program field. The welcome page now names built-in Pi, Codex, Claude Code and external Pi individually. Built-in Pi opens the existing account page; external entries open the existing program settings and focus the selected executable path. All existing project/setup/menu actions remain.

The existing version probe supplies the external cards' last-check text. Unknown stays untested; an edited path invalidates the old result; an unsaved path is explicitly marked. Saving clears that unsaved qualification. Nothing is automatically probed, installed, signed in, selected as the lead or started. This is not complete readiness detection: version results do not prove login, compatibility or a usable model, and are not persisted as ready after restart.

This follows the separation of Agent entry/status/repair in [Beluga's pinned setup source](https://github.com/kayraucklnc/beluga/blob/cab72cb7a85efef601651c516d4ddae46fde516f/packages/mac/src/onboarding/agent-setup.ts). Hive reuses its actual version checks and existing configuration semantics; Beluga's readiness policy/default fallback was not imported.

## Real input defect found and fixed

Dark/20px, narrow window: keyboard entered external Pi settings for the first time and immediately typed `ZH_PI_FOCUS`; only `CUS` remained. The shared asynchronous settings read unconditionally replaced all inputs when it returned, including fields edited after the read began. Direct entry made that existing overwrite observable.

The shared reader now captures the submitted/read-start values and replaces each input only while it still matches that baseline. New typing survives; untouched fields still receive saved configuration. The same rule covers initial load and explicit Reload for all three external programs; no timer, retry, global flag or deferred input queue was added. Save still writes its submitted values; subsequent edits remain unsaved.

Replayed the same Chinese/Dark20 narrow keyboard entry: full `ZH_PI_FOCUS` survived. Selected the field, clicked Reload and immediately typed `AFTER_RELOAD_INPUT`; the entire new value survived. See `initial-read-overwrite.ax.txt`, `initial-read-fixed.ax.txt`, `reload-edit.ax.txt`.

## Native checks

Evidence root `.artifacts/welcome-agents-a8ce080/`:

- First Codex card → direct typing into the correct path → actual original CLI version `0.167.0` → unsaved summary → changed-input invalidation → real nonexistent-path failure → repaired path/probe/save → saved summary. No model request.
- Claude and external Pi entries accepted exact distinct markers in their own fields; markers were cleared without saving. Built-in Pi opened the actual 42-provider account page against an empty owned Pi directory; no login was started.
- Welcome rows wrap in equal-width columns with equal row borders. Initial unequal intrinsic widths were corrected by placing flex sizing on the actual flex child; final border/stretch belongs to that outer layout element. Light16 English and Dark20 Chinese wide/narrow captures checked the result.
- In an approximately592×500 logical window, collapsed the full-width compact sidebar, then Tab revealed an offscreen Configure action; Return opened the correct program field. English Claude and final Chinese external Pi entry were exercised. No fixed Tab count is promised across render transitions.
- Cold restart reset all three probes to untested while retaining the app-saved original Codex path. Final SQLite: zero projects, messages and dispatches; no chat index entries or credentials. `check-state.py` and `receipt.json` verify exact saved values and native evidence.
- Final `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`, LLVM-assisted `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop`, and `git diff --check` passed. Native processes exited normally. No source tests were added/removed: this is UI/configuration-form behavior, verified in the actual window; no full suite was run.

## Evidence limits

The owned debug wrapper initially lacked its runtime location and then its runtime working directory; those two launch attempts failed visibly and exited before the UI checks. Corrected the wrapper and used explicit source runtime/bundled Node overrides. Initial appearance fixture keys were incorrect and followed the system theme; final captures use the actual `mode`/`font_size` schema. Neither harness issue led to a production fallback.

This is not a new extracted package, Windows native result, complete account/readiness detection, VoiceOver/IME acceptance, or full reference-product parity. The observed partial input and its fix concern asynchronous settings replacement; no broader IME conclusion is drawn. The original picker409 cause and all full-goal gaps remain open.
