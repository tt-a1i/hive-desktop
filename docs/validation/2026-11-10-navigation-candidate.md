# Model search and collaboration candidate — 2026-11-10

Source `34aebbeec01b332bffc1e12d0cb2b2fb0b2aef7d`. Includes the model-list focus reveal, configured-model search/clean form comparison, and collaboration action/navigation fixes following fb3dcd0. Evidence `.artifacts/desktop-candidate-34aebbe/`. This is an unsigned macOS arm64 development candidate, not full product acceptance or a Windows runtime result.

## Package construction

- Canonical ZIP `.artifacts/hive-desktop-macos-arm64-candidate.zip`: 158,260,143 bytes; SHA256 `beb0037ff0e0dca825802fed6bac86d54c803846d3a16812e6fefca91f381cbe`.
- Native binary SHA256 `f23173b98c1b056b552bac089ca9b180323f500019ae9f61ede0ce87748aff1d`.
- Runnable application `.artifacts/desktop-candidate-34aebbe/extracted/bundle/Hive Desktop.app`.
- Confirmed unchanged lockfiles/package metadata, patches and native dependency metadata since prior candidate. Reused its frozen production dependency tree (source 1534986), completely replaced native binary/dist, and compared every entry. Exactly the native binary changed; all 112 runtime files and dependency entries remain unchanged. This is a code-only refresh, not a fresh dependency deployment.
- ZIP CRC, independent ditto extraction, all 28,863 file/link entries including modes and 29 internal symbolic links matched. Complete extracted inventory was checked again after both GUI runs.
- Three current package gates passed, exit 0: `node scripts/check-package.mjs --bundle ...`, `check-packaged-models.mjs`, `check-packaged-dispatches.mjs`. They use actual bundled Node/runtime/SDK/native dependencies and temporary state. Scope includes real PTY/HTTP/SQLite, model JSONC/overrides/default handling, delivery recovery and workflow shutdown, without model inference. Existing synthetic transport cases retain that scope.

## Actual packaged GUI

Launched the independently extracted executable with HIVE_DESKTOP_NODE/HIVE_DESKTOP_RUNTIME/HIVE_CODEX_COMMAND unset. A separate process snapshot verified the app's direct child was its own `Resources/node/bin/node` running its own `Resources/runtime/dist/main.js` (`native-process.json`). Existing saved app-specific Codex 0.167.0 path retained. Copied owned real report history and synthetic model config; no credential copy. Pi SDK created an empty auth object. No prompt/start or new inference requested this turn.

1. Actual existing Late-worker history, original failed tool output and `MEMBER_UNSENT_FOCUS_PROOF` draft restored. The original failure is retained as history, not a new app failure. Light/20px/English and approximately 592×500 logical pixels.
2. Collaboration entry focused Refresh visibly. Seven Tabs (read-only document adds no Tab stop) revealed the actual reported dispatch card with full outline. Return opened its real details; Back to dispatch list focus was visible. Six Tabs revealed Preview; Return opened the actual numeric `{sum:37,count:3}` result in the read-only preview. An initial eight-Tab sequence overshot the card and correctly wrapped to navigation; not counted as successful card activation.
3. Models page showed 3 providers / 34 models. Actual `Search edit proof` input filtered to one provider / one model. Seven Tabs reached the matching model edit action with a visible outline despite the short viewport. Return opened `layout-local/layout-model-030`, saved name `Search edit proof · 中文`, Save disabled. Later Tab/Return activated the fixed settings Back action and left settings without a discard prompt; this is not claimed as a direct Close form button test.
4. A first AX click on the offscreen owner link did nothing. Actual wheel back to the visible owner link, then click, opened the original worker conversation. Without an input click, Cmd+Right/type `x` appended to the existing draft, proving composer focus. Backspace restored the exact original draft; no Send/Enter action followed. Both draft values captured.
5. First Cmd+Q exited normally, exit 0. Independent check using the actual bundled Node and imported bundled runtime restored the same two thread IDs, original main finals `WORK_STARTED` / `LATE_REPORT_OK 37/3`, original worker final count, pending 0/busy false, reported ledger and artifact. Exact saved 1,912-character task document and original project hashes were unchanged; no task draft remained.
6. A second real application launch restored the same member draft/history. Settings → Models showed an empty temporary search and the complete 3/34 list. Normal Cmd+Q exit 0. Full models.json stayed byte-identical to source fixture, actual `chat-state/desktop-model-drafts.json` remained absent and auth stayed `{}`. Earlier draft-file assertion used an incorrect candidate path; corrected it to the real source-defined location before recording final evidence.

No new inference was needed for these native-only changes. The preceding candidate's actual packaged Terra/tool run remains separately evidenced at source fb3dcd0; it is not relabeled as a new 34aebbe model run. No fresh first-launch state was exercised in this slice; prior entry evidence retains its original scope.

## Storage and remaining acceptance

After both native/gate exits and exact-directory lsof checks, removed only the new duplicate base tree and superseded fb3dcd0 extracted bundle. Preserved the prior candidate ZIP with verified original SHA at `.artifacts/desktop-candidate-fb3dcd0/candidate.zip`, all old states, workspaces, manifests and evidence. Allocated bytes removed are recorded in `cleanup.json`; this is not a physical free-space measurement. New candidate pointer updated; current ZIP remains source 1acd805 and hash `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6` verified unchanged.

Final current-source Mac build and diff check passed; the three included UI fixes have their own Windows MSVC target checks. No runtime/protocol/schema/test change or unrelated full suite. Whole-client parity, full VoiceOver/IME/font/theme/long-history matrix, remote devices, real Claude/account renewal, Windows native, signed/notarized installers and final product acceptance remain open. Full goal stays active.
