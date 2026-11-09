# Optimized macOS candidate — 2026-11-10

Native/runtime source `252c19c3210eabeb53a9056f0f26549bec03cabe`. This is the canonical **candidate**, not the current ZIP or a signed installer. Evidence directory: `.artifacts/release-252c19c/`.

## Optimized build and package

`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` exited 0, reporting optimized release in 3m48s. The ARM64 executable is 35,617,600 bytes versus 117,707,472 bytes for the existing debug executable from the same native source (native sources are unchanged between ff86889 and 252c19c): 69.7% fewer bytes. This is executable size, not a memory, startup or responsiveness benchmark.

The packaging guide now names the release command/path. Windows workflow build/package paths were changed together to `release`; Ruby YAML parsing and path agreement passed. Python's optional YAML module was absent; Ruby used the installed standard library instead. No Windows CI was requested or observed; this configuration change proves no Windows runtime result.

ZIP: `.artifacts/hive-desktop-macos-arm64-candidate.zip`, **143,394,027 bytes**, SHA-256 `6e601c1243d6884efa746bcae25f37bf146786785f1bbb1f8d37a439e8c082b9`. Native SHA-256: `1f5619efa72b4ec447eebce749a910d4d2b2c1232233ee38b07eabcb1c965fae`.

Frozen production deployment source remains `1534986f992fe011cef94706618bf2ee2acf40df`. Lockfiles, package metadata and dependency entries matched the verified previous bundle. Relative to 34aebbe, only the native executable and compiled `hive-host.js`/source map changed; all 112 built runtime files matched the source output. The first assembly attempt correctly stopped because its expected-change list omitted the source map. After terminal failure and no-handle checks, removed that partial duplicate and reran with all three expected entries. No dependency drift was accepted.

ZIP CRC, independent ditto extraction and mode/hash/symlink inventory passed: 28,863 entries and 29 internal links. Post-GUI inventory remained exact.

## Gates and actual application

All six commands exited 0 with `--bundle .artifacts/release-252c19c/extracted/bundle`:

- `node scripts/check-package.mjs`
- `node scripts/check-packaged-models.mjs`
- `node scripts/check-packaged-dispatches.mjs`
- `node scripts/check-packaged-codex.mjs`
- `node scripts/check-packaged-remote.mjs`
- `node scripts/check-packaged-claude.mjs`

These exercise package-bound Node/runtime, actual HTTP/SQLite/PTY and relevant native adapters/protocol peers without model inference. Synthetic CLI peers and the local remote gateway are not real vendor/model/production-gateway acceptance.

Launched the independently extracted release app without Node/runtime/Codex overrides, using a cold copy of the prior ff86889 verification state after its processes exited. The app restored the actual original Codex Terra conversation, completed command and `PACKAGED_FF_OK 41/3`, plus the exact Chinese/English unsent draft. Opened the real tool details and observed exact input/output. Submitted an owned nonexistent path through the native workspace form: the **packaged** error included the concrete path reason while retaining draft/input. Normal exit 0; another cold launch restored the same original conversation/tool/reply/draft; second normal exit 0.

`check-cold.mjs`, executed using bundled Node and bundled runtime, verified one original user input, one actual tool result with exact output/exit 0, one final reply, original thread, Terra/Medium, pending 0, original Codex0.167.0 from saved app configuration and unchanged input fixture. No new inference was sent by this release validation; live model execution belongs to the [ff86889 fresh-package evidence](2026-11-10-fresh-package-flow.md). See native AX/screenshots, `cold-receipt.json`, `build-receipt.json`, `receipt.json`.

## Preservation and remaining work

After all gates/native/cold processes exited and lsof found no handles, removed only release assembly `base/`, superseded 34aebbe extracted bundle and unused ff86889 extracted bundle: **1,779,253,248 allocated bytes**. Old34 canonical ZIP was moved to `.artifacts/desktop-candidate-34aebbe/candidate.zip` and hash reverified. ff86889 staged ZIP, all project/state/evidence directories and old chat workspaces remain; current1acd805 ZIP hash was reverified unchanged. The new release app remains directly runnable. This is not a physical free-space delta.

A separate native double-click Add check at 252c19c opened one new owned project without reproducing 409; it does not establish the cause of the earlier picker sequence. No speculative guard was added. That observation, complete onboarding/readiness, Windows native acceptance, actual Claude model collaboration, Pi reauthentication, production remote/phone flows, complete accessibility/IME/performance matrices and formal signing/installers remain open. Release optimization and these checks do not complete the full goal. No full suite was run: no application logic changed in this slice; the six extracted-package regressions and actual optimized GUI/cold checks cover the packaging transition.
