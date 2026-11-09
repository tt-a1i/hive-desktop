# Reasoning description and keyboard acceptance — 2026-11-09

Source baseline `7cc484b`. Evidence: `.artifacts/reasoning-reading-7cc484b/`; owned state reused from the actual Terra case in [composer/usage](2026-11-09-composer-usage.md). This step sent no inference prompt and did not change the selected model/effort.

## Reproduced and fixed

At approximately 822 × 450 logical pixels, opened the real original Codex 0.167.0 reasoning panel using Tab/Enter. Its application-owned description remained Chinese in English UI and was absent from the native accessibility tree (`before.ax.txt`, `before.jpg`).

The heading, description, loading, error and unavailable explanation now use the existing selectable-text accessibility wrapper. The panel has an accessible group label. Only the two known Hive-owned Codex/Claude adapter descriptions pass through static translations; unknown descriptions/diagnostics and native level values are preserved. No backend/queue/settings logic changes.

## Actual native check

- New source build reopened the same thread/draft. Input focus → three Tabs → Enter opened reasoning. Native AX exposed the complete English heading and description; all six actual provider levels remained available, Medium selected.
- Resized to approximately 822 × 450; description wrapped and every option plus Reload remained visible. `fixed.jpg` / `fixed.ax.txt`.
- Seven Tabs reached the final Reload control, with visible focus ring. Enter read the native state; Escape closed and Enter reopened the panel through restored trigger focus. Medium remained selected and the draft retained. `keyboard-reopen.jpg` / `keyboard-reopen.ax.txt`.
- The prior source build also verified input → four Tabs → Enter opened Usage, Tab/End reached its bottom, and Escape/Enter reopened it. This validates this keyboard route, not every focus/VoiceOver interaction.
- Both owned native runs exited normally: exec sessions 20265 and 55954 returned 0. Windows cross-check session 69069 returned 0; macOS build session 58787 returned 0.

Checks: `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, Windows target `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` with LLVM on PATH, and `git diff --check` passed. No new UI tests/full runtime suite for this UI-only change. Windows native, real Claude description display, failure/loading transitions, copy interaction, larger fonts and broader width/theme/VoiceOver matrix remain unverified. Candidate ZIP not refreshed; full goal remains open.
