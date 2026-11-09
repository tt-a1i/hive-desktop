# Entry/configuration candidate package — 2026-11-09

Canonical candidate source: `d4c57a1bbb5338a9761ae9c99519c72983c27a5f`.

- ZIP: `.artifacts/hive-desktop-macos-arm64-candidate.zip`, 159,575,042 bytes.
- ZIP SHA256: `5bc0f9f36c196d2e0b450d473f6128abe90555b7a37240a0a506307ec57f6d6e`.
- Native binary SHA256: `406e2d91ed573d1e78e75ba33941539284f6af2377f247538f27bd1d97c36028`.
- Runnable app retained at `.artifacts/desktop-candidate-d4c57a1/extracted/bundle/Hive Desktop.app`.
- Receipts/evidence: `.artifacts/desktop-candidate-d4c57a1/`; pointer: `.artifacts/current-package/candidate-receipt.json`.

## Assembly and integrity

Applied the documented code-only refresh path. Verified prior canonical ZIP digest and unchanged lockfiles, workspace settings, patches, root package metadata, Cargo metadata and runtime production package metadata; the sole manifest exception was development scripts. Reused the target-platform frozen production dependencies/Node from `1534986`. This is not a fresh deploy.

Built the current complete runtime and replaced its entire `dist` plus native executable. All 112 current runtime files matched the bundle; runtime bytes were unchanged, so the native binary was the only changed inventory entry. ZIP was produced and independently extracted. All 28,863 regular-file/link entries, including permissions/hashes and 29 internal links, matched the assembled tree. No dependency was manually trimmed. `manifest.json`, `dependency-baseline.json`, `refresh.json`, `extraction.json` retain the comparisons.

Includes dispatch accessibility/selectable-body semantics, the T3 Code/Beluga-informed welcome, project context and draft/lead-selection explanation, settled chat-open errors, and program readouts/focus/English Browse fixes. Inclusion does not extend the validation scope of each originating UI record.

## Package gates

All ran against the independent extraction, exited 0 and requested no inference:

- `check-package.mjs`: actual bundled Node, real PTY, Hive HTTP/SQLite, account metadata, workspace and shutdown.
- `check-packaged-models.mjs`: actual SDK/JSONC, overrides, production JSONL, reload and default-model precedence/retention.
- `check-packaged-dispatches.mjs`: real dispatch integration and workflow shutdown, including the existing controlled protocol peers.

The existing queue/remote/package adapter gates from the exact reused production runtime/dependency baseline were not rerun; this refresh changed only native UI. Full test suite was not run. Logs are local evidence, not proof of all product behaviors or Windows support.

## Actual packaged window

Started the extracted app with a new owned data directory and no development Node/runtime or CLI override. Observed its actual child process using the bundle's Node and `Resources/runtime/dist/main.js`; `native-process.json` records their paths/PIDs. The first command-path observation assumed an absolute `ps comm` entry, but the shell supplied a relative native path; normalizing that path allowed the actual child verification. No production code changed for this artifact check.

Native operations:

1. English first-entry view exposed complete title/description/three steps in AX.
2. Its external-program entry opened the real configuration page. Entered original Codex path, checked actual version 0.167.0, saved and read updated application source/success notice.
3. Native folder picker created one owned project, exposing project path and draft/lead explanation.
4. Entered a two-line Chinese/English draft and chose Codex from the four-backend menu. One real chat was created; the draft transferred exactly.
5. Selected `openai/gpt-6.1-terra` in the native model picker. The existing user's default had initially appeared as metadata; no other model was invoked.
6. Cmd+Q exited normally, then a completely new app process reopened the same project, chat, Terra choice and exact unsent draft. Second Cmd+Q exited 0.
7. Independently ran `check-cold.mjs` under packaged Node, importing packaged runtime. It verified saved executable source `app`, same chat/model, pending 0/history 0, zero Hive messages and empty delivery spool.

`welcome.ax.txt`, `ready.ax.txt`/PNG, `cold.ax.txt` and `cold-receipt.json` record scope. Native exec sessions 25678 and 95034 returned 0. No model prompt, authentication change or global default change. The `check-cold` artifact is only for this owned scenario.

## Delivery and storage

After checks and owned process exit, atomically replaced only canonical candidate ZIP. Current ZIP remains source `1acd805` with SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`; rehashed unchanged. Candidate was not promoted to current.

Checked no process/handles used the new temporary base, then removed its duplicate 620,339,200 allocated bytes (about 592 MiB). Retained one runnable candidate app, the canonical ZIP, owned test state and evidence. The old candidate digest/receipt remains in dependency provenance; no second copy of the old ZIP is retained. Other chats' directories and user data were untouched.

Unsigned/unnotarized macOS arm64 portable development candidate. Windows native, formal installer/signing, full keyboard/VoiceOver/IME/light-theme matrix, real Pi credential renewal, Claude model collaboration and complete reference-product parity remain open. Full product goal is active.
