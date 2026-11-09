# Account focus outlines and saved variants — 2026-11-09

Source baseline `d69862b`; follow-up to [account reading](2026-11-09-pi-account-reading.md).

## Fixes

The previous actual toolbar screenshot showed its focus outline clipped at the scroll container's left edge. Added four logical pixels of inner padding to the existing account body, keeping the focus ring within the clip. No change to shared scrolling algorithms or controls. A single-result English search exposed `1 providers`; count now selects the singular label for exactly one result.

## Actual native evidence

Evidence: `.artifacts/pi-account-variants-d69862b/`. Fresh owned app state, isolated Pi directory, offline mode. Seeded that directory with explicitly invalid synthetic API/OAuth data solely for **local saved-state display**. `PiAuth` creates ModelRuntime with `refreshOnCreate: false`; metadata reads use credential type only. No login/API button activated, no credential refresh/model prompt requested, no valid account or model capability claimed.

- Native catalog exposed Anthropic · Subscription account saved, OpenAI · API configuration saved, and Azure · No credentials saved locally, with the actual corresponding Log in again / Update API configuration / initial configuration action names. `loaded.ax.txt`, `api.*`, `oauth.*`.
- Approximately 822 × 450 logical pixels: Tab reached the API update control and displayed its complete row with the status, remove action and visible focus outline (`api-focused.jpg`). Authentication controls were focused, never activated.
- Final build cold launch retained those saved-state variants. Reverse Tab from search twice showed the complete Refresh account status outline, including its left edge (`final-focus.jpg`). This resolves the specific clipping recorded in the previous validation.
- Actual Anthropic search showed `1 provider`, selected its existing re-login control using Tab, and retained a complete focus outline (`final-singular.jpg`, `.ax.txt`). The earlier `oauth.*` screenshot predates the singular correction.
- Both native processes exited normally, code 0: sessions 83387 and 92093. Read-only checks confirmed fixture auth bytes unchanged, no synthetic credential value in any recorded AX tree, zero workspaces/messages and no chat index. `receipt.json` records pass. Global credentials/defaults were untouched.

Final macOS build and Windows target check passed (`cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`; `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, LLVM on PATH), as did `git diff --check`. UI-only adjustment: no new tests/full runtime suite; existing authentication implementation/tests unchanged.

This proves local display/keyboard geometry, not valid OAuth/API authentication, complete VoiceOver/font/compact-width/theme acceptance or Windows native behavior. Candidate ZIP stays `b539c26`, so these source fixes are not yet packaged. Full product goal remains active.
