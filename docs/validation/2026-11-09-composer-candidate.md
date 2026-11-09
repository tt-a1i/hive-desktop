# Composer/setup candidate package — 2026-11-09

Candidate source `b539c263e85a1d350af178785c53bcbd8ee154c6`.

- Canonical ZIP: `.artifacts/hive-desktop-macos-arm64-candidate.zip`, 158,241,970 bytes.
- ZIP SHA256: `fedbfc2954732c9175f242ea4cc5605359260c1511e299709c3f0ed5f41e34bc`.
- Native SHA256: `6470ec32e63cd5c00386e2e72f304f4e7c1a5bea39ef506a3e8e6eb540ad7b9d`.
- Runnable app: `.artifacts/desktop-candidate-b539c26/extracted/bundle/Hive Desktop.app`.
- Receipts/evidence: `.artifacts/desktop-candidate-b539c26/`; canonical pointer `.artifacts/current-package/candidate-receipt.json`.

## Assembly and checks

Used the documented code-only refresh, not a fresh dependency deploy. Verified prior ZIP hash and prior bundle inventory; lockfiles, workspace, patches, package and Cargo metadata are unchanged from `d4c57a1`. Reused its `1534986` frozen macOS arm64 dependencies/Node. Rebuilt runtime/native, replaced the entire runtime dist and executable; all 112 runtime files match. Only the native binary changed among 28,863 file/link entries. ZIP CRC passed; independent ditto extraction matched every regular-file hash, mode and 29 internal links. `assembly.json`, `manifest.json`, `dependency-baseline.json` preserve provenance.

The first artifact inventory comparison failed because the old manifest omitted symlink modes. Normalized that old representation only; new assembly/extraction comparisons retain modes. No package or product behavior changed to satisfy a check.

Three gates against the independent extraction exited 0, without inference:

- `scripts/check-package.mjs`: bundled Node, actual PTY/Hive agent-manager, HTTP/SQLite, account metadata, workspace and shutdown.
- `scripts/check-packaged-models.mjs`: actual SDK/JSONC, overrides, wire/reload and default-model precedence/retention.
- `scripts/check-packaged-dispatches.mjs`: real dispatch and workflow shutdown, including existing controlled protocol peers.

The same frozen runtime/dependency baseline's queue, remote and other adapter gates were not repeated for these native-only changes. No unrelated full suite.

## Actual packaged window and cold read

Launched the independently extracted app with owned data/Pi directories and development Node/runtime/CLI overrides unset. `native-process.json` confirms its actual child used bundled Node and `Resources/runtime/dist/main.js`.

1. Fresh English state: short-window welcome setup exposed all three destinations; External agent programs opened the real settings page with complete sign-in/version guidance (`welcome-menu.*`, `program-guidance.*`). No configuration/login/probe activated. Cmd+Q exited 0, exec session 22990.
2. Separate state cloned the earlier **real** Codex Terra thread from the source-level [composer case](2026-11-09-composer-usage.md). Package restored the same final and exact unsent Chinese follow-up. At approximately 822 × 450 logical pixels, Send stayed inside its card (`cold-chat.*`).
3. Input → four Tabs/Enter opened native usage; Tab/End reached the complete last capacity/notice. The bounded popover left Send visible (`usage-end.*`). Exact existing native totals were restored; no new inference requested.
4. Escape, Shift+Tab/Enter opened reasoning, showing English accessible description and actual Medium selection (`reasoning.*`). No level changed. Cmd+Q exited 0, session 48566.
5. Independent `check-cold.mjs` ran with bundled Node/imported packaged runtime, resumed the same thread/model/Medium, one prior user delivery, two successful tools, one exact prior final, pending 0 and unchanged draft. Fresh state had zero workspaces/messages, no chat index, and an empty SDK-created credential object. `cold-receipt.json` records pass. The initial check used an encoded URL pathname for the space-containing App path; corrected the artifact check to `fileURLToPath`, then passed. No production change for that expectation.

These are package restoration/interactions of an existing real thread, **not a fresh package-generated model response**. The source-level inference retains its original scope. No global defaults/authentication changed.

## Candidate delivery and cleanup

Atomically replaced canonical candidate ZIP and updated its receipt. Current ZIP remains source `1acd805`, SHA256 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`, rehashed unchanged. Candidate is not promoted.

After owned native exit and lsof checks, removed only this assembly's duplicate base and the superseded `d4c57a1` extracted bundle: 1,240,678,400 allocated bytes of duplicate trees (about 1.16 GiB; not a controlled physical free-space measurement). Retained the new single runnable app, ZIP, original evidence/test state and prior receipt; marked prior bundle absent. No chat workdir/user data/cache dependency store deleted. `cleanup.json` records exact paths.

Unsigned, unnotarized macOS arm64 portable development candidate. Full Windows native, real Claude/Pi account renewal, complete keyboard/VoiceOver/IME/font/theme matrix, signed installer and T3/Beluga/Cindy functional/visual parity remain open. The complete goal is active.
