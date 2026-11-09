# First-use Agent setup routing — 2026-11-09

Source baseline `5eafaa3`. The generic welcome action previously always opened built-in Pi accounts, sending users who intended Codex/Claude/external Pi into the wrong configuration context. It now offers three explicit existing destinations: built-in Pi accounts, models/providers, external programs. All routes reuse normal settings navigation; no onboarding completion state, readiness inference, automatic backend fallback or new permission layer.

External programs also explains that external CLI login/configuration belongs to that CLI. A successful version probe does not establish signed-in/model-ready status. This is ordinary setup guidance and does not trigger a probe or model request.

## Native evidence

Evidence: `.artifacts/welcome-setup-5eafaa3/`. Fresh app-local English data, empty isolated Pi directory, offline mode; no existing project or conversation.

- `menu.jpg` / `menu.ax.txt`: all three named destinations visible.
- Actual clicks opened each matching page. `programs.*` includes the complete English login/version guidance in native AX; `models.*` shows the existing Pi model configuration page; `accounts.*` shows existing Pi account management with the owned directory.
- At approximately 822 × 450 logical pixels, the full menu fits and is readable (`narrow-menu.*`). Clicked welcome noninteractive title area, Tab reached the project action with visible focus; Tab/Enter opened setup, three Down presses selected External agent programs, Enter opened that actual page (`keyboard-programs.*`). No login/API action, version probe, settings save or project creation was activated.
- Native exec session 49271 exited normally, code 0. Read-only SQLite/artifact assertions found zero workspaces/messages, no chat index and an empty SDK-created auth object. `receipt.json` records pass. The Pi SDK created empty auth storage during metadata loading; no credentials were added.

## Verification and limits

Passed macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, Windows `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` (LLVM on PATH), and `git diff --check`. UI-only change: no new tests/full runtime suite.

This verifies setup navigation and guidance, not full Beluga readiness parity or external account verification. Chinese native second pass, all keyboard/focus routes, VoiceOver and Windows GUI remain separate acceptance. Candidate ZIP not refreshed in this step. Full product goal stays open.
