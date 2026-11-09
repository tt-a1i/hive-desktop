# Program settings reading and keyboard access — 2026-11-09

UI slice after `7f40f77`, not complete desktop acceptance.

## Problem and change

The previous native program-repair flow worked visually, but its version/result/source and save feedback were absent from AX output. In a short window, lower fields and footer actions also needed deliberate focus/reveal handling.

Moved the existing Programs body from the settings switch into its existing module. Reused shared `selectable_text` for the description, effective command, checked command, status/error, compatibility note, stale-check warning, unsaved indication and save notice. It preserves the same rendered content and adds native reading labels. Reused the existing tracked scroll viewport and per-field/footer `Reveal` wrappers so keyboard focus scrolls controls into view. No new framework or backend readiness interpretation. Corrected the missing English translation for “浏览…”.

This follows the source-observed focus/control organization from [T3 Code references](../research/2026-11-09-desktop-source-references.md), implemented using existing GPUI helpers. No upstream code/assets were copied.

## Actual native evidence

Directory `.artifacts/program-reading-7f40f77/`: copied the previous owned validation state while no process held it; independent state/agent directories and English preference. Existing project files were only referenced. No model request was sent.

- Actual original Codex `0.167.0` version probe produced AX-readable result, effective `App settings` source, exact checked command and separate `0.166.0` reference-version compatibility note. A version check remains metadata, not full compatibility proof.
- Changed the draft to a nonexistent owned test pathname. Old successful version/source disappeared and the stale-check explanation appeared. Real probe returned Program not found and the exact entered path; no invalid path was saved. `missing.ax.txt` captures the failure without the old successful version.
- At about 822 × 547 logical pixels, Tab traversed from Codex path/browse to Claude check/path/browse and Save. The existing reveal handler scrolled lower field controls and the complete focused Save button into view. `keyboard-save-visible.png` is an inspected full-size screenshot; its earlier Browse labels predate the translation correction.
- Tab/Enter on Reload restored the saved valid path and disabled Save; then entered an equivalent valid `bin/./codex` pathname, ran its actual version check and reached Save using five Tabs from the Codex input. Enter saved it. Full AX contained the complete success notice and updated effective command. Restored the original command through the same keyboard save flow.
- Final build reopened with the original saved path, disabled Save, retained conversation draft/Terra and English Browse buttons. Final version probe again exposed the same complete metadata. `final-version.png` and `final-version.ax.txt` cover this final source build.
- Both owned native processes exited normally: exec sessions 7991 and 12316 returned 0. A read-only SQLite/artifact check asserted restored command, zero Hive messages, unchanged chat index and exact original draft bytes; `receipt.json` records pass.

Targeted checks passed:

```sh
cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml
PATH="/opt/homebrew/opt/llvm/bin:$PATH" cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc
git diff --check
```

UI-only change: no source tests/full suite, backend protocol, executable probing policy or dependency change. Physical VoiceOver announcements, copy/selection of these particular new readouts, light-theme/Chinese matrix, system executable picker and Windows native interaction remain to verify. Shared selectable-text behavior has separate prior native evidence; this record does not substitute that for a new program-readout clipboard check. Candidate/current ZIPs were not rebuilt or promoted.
