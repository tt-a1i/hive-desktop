# Unified model/settings candidate — 2026-11-10

Candidate source `fb3dcd03f909a1fbb7680dabc11cb759dd01b32b`. Supersedes the [b539c26 candidate](2026-11-09-composer-candidate.md); current ZIP remains separate and unchanged.

- Canonical candidate ZIP: `.artifacts/hive-desktop-macos-arm64-candidate.zip`, 158,245,333 bytes.
- ZIP SHA256: `6e54e24d89918413ae87ae98e578d5e1a287e30493dff43df39de508bc893779`.
- Native SHA256: `c46baa54c093710c6d4e86695fc09580922fa6328f62a2ed24610ffdc888e692`.
- Runnable app: `.artifacts/desktop-candidate-fb3dcd0/extracted/bundle/Hive Desktop.app`.
- Evidence: `.artifacts/desktop-candidate-fb3dcd0/`; pointer `.artifacts/current-package/candidate-receipt.json`.

## Source included and assembly

Includes the merged model/effort panel, busy-state close fix, account reading/focus improvements, short-window settings navigation and corrected Codex verification metadata. Source-level live queue/input and CLI repair evidence retain their original scope; package-specific evidence follows.

Code-only refresh: prior ZIP hash and complete bundle manifest matched; production lock/workspace/package/patch/Cargo metadata unchanged. Reused frozen macOS arm64 Node/dependencies from `1534986`. Complete current runtime dist and rebuilt native executable replaced the old copies. All 112 runtime files match the checkout. Exactly two changed entries: native executable and runtime `dist/desktop-settings.js`; every dependency entry unchanged. No manual dependency trimming or fresh dependency deploy.

New manifest includes modes, hashes and links. ZIP CRC passed; independent ditto extraction matched all 28,863 file/link entries and 29 internal links. Post-gate inventory matched. Initial assembly check incorrectly applied the older mode-less symlink normalization to this newer full manifest; changed the artifact check to exact mode-preserving comparison, then passed. No product change for that artifact expectation. `assembly.json`, `dependency-baseline.json`, `manifest.json`, `post-check-integrity.json` retain provenance.

## Extracted package checks

All three gates exited 0 against the independent extraction, using actual bundled Node/runtime/dependencies:

- `node scripts/check-package.mjs --bundle <bundle>`: image payload, real PTY and Hive agent-manager, account metadata, HTTP/SQLite workspace and shutdown.
- `node scripts/check-packaged-models.mjs --bundle <bundle>`: actual SDK/JSONC config, overrides, wire/reload and default-model precedence/retention.
- `node scripts/check-packaged-dispatches.mjs --bundle <bundle>`: real dispatch/lifecycle and workflow shutdown, including existing controlled protocol peers.

Logs are `gate-package.log`, `gate-models.log`, `gate-dispatches.log`. Source-level `check:programs` also passed with original Codex --version. No unrelated full runtime suite; no dependency/core protocol changes.

## Actual packaged application

Development Node/runtime/Codex overrides unset; owned data and Pi directories. `native-process.json` confirms the native process actually parented its bundled Node executing packaged `Resources/runtime/dist/main.js`.

1. Fresh English state: setup menu offered Built-in Pi accounts, Models and providers, External agent programs. Actual Pi account destination showed 42 providers with local credential status and provider-specific actions (`fresh-menu.*`, `fresh-accounts.*`). No login/account write/probe activated. Cmd+Q exit 0 (exec 77511).
2. Separate owned state restored the real Terra thread and exact Chinese/English draft from the [program repair](2026-11-10-agent-program-repair.md). At approximately 822×450 logical pixels, Light/20px, opened the merged model/effort panel. Search and actual native Medium appeared. Seventeen real Tabs scrolled both columns to the final model and Reload with visible focus (`model-panel.*`, `model-bottom.*`); no model or effort changed.
3. Escape, Cmd+comma, search → Shift+Tab: bottom Role templates scrolled into view. Return actually opened the template form; Back returned to exact draft (`navigation-bottom.*`, `roles.*`, `returned-draft.*`). No template created. Cmd+Q exit 0 (exec 19533).
4. Independent bundled Node/runtime cold read confirmed same thread, Terra/Medium, pending 0, two prior user inputs and one actual command result; zero fresh workspaces/messages, empty fresh auth object and no fresh chat index. Original Codex --version returned 0.167.0 with verifiedVersion 0.167.0. `restore-cold-receipt.json` preserves this pre-inference result.
5. Launched the extracted package again. Through native editor keyboard input and actual Send click, requested exactly a shell waiting 20 seconds and printing `PACKAGE_FB3_TOOL_OK`, then exact final `PACKAGE_FB3_OK`. Only original Codex 0.167.0 / `openai/gpt-6.1-terra` / Medium. Actual In progress command appeared (`live-running.*`). While that tool was still running, clicked native editor and pasted the original complete Chinese/English multiline draft; it remained unsent and Queue enabled (`live-draft.*`). Tool completed, exact final appeared, Send restored, no model panel reopened and draft remained (`live-settled.*`). Cmd+Q exit 0 (exec 30534).
6. Final independent packaged cold read (`check-cold.mjs`, `cold-receipt.json`) confirms same native thread, Terra/Medium, pending 0, precisely three user deliveries, ordered original two replies plus `PACKAGE_FB3_OK`, two actual command results. Latest command aggregated output equals `PACKAGE_FB3_TOOL_OK`, exitCode 0; exact original draft retained. Cold read requested no inference. This is a fresh actual package-generated response, separate from restoration evidence.

No global account/default changes, alternative model or Magpie. Runtime diagnostics around experimental SQLite retain their ordinary Node scope.

## Delivery and storage

Atomically replaced canonical candidate ZIP and pointer after gates/native restore checks; added final live result afterward. Candidate is not promoted to current. Current ZIP remains `1acd805`, SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`, rehashed unchanged.

After normal native exit and lsof confirming no open handles, removed only this assembly's `base` duplicate and the superseded b539c26 extracted bundle: 1,240,694,784 allocated bytes (about 1.16 GiB; not a controlled physical-free-space measurement). Retained one new runnable app, canonical ZIP, old manifests/receipts, validation data and evidence. `cleanup.json` records exact paths; prior receipt marks bundle absent/superseded. No source, chat workdir, user data or shared cache removed.

Unsigned, unnotarized macOS arm64 development candidate. Windows native, full VoiceOver/IME/font/theme/history matrix, real Claude/account renewal and remote-device acceptance, formal installers and full T3/Beluga/Cindy parity remain incomplete. Full goal stays active.
