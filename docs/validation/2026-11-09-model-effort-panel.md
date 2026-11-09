# Combined model and reasoning panel — 2026-11-09

Source baseline `6847463`. Read-only inspection of installed T3 Code (Beta) 0.1.7 showed a single composer model/effort trigger and a combined search/reasoning panel. Opened its menu, observed layout, then Escape returned it to the original empty draft without selecting anything or changing settings. No upstream code/assets copied. Earlier pinned [source references](../research/2026-11-09-desktop-source-references.md) remain separate provenance.

## Implementation

Hive's composer previously exposed separate model/reasoning buttons that consumed space and wrapped in the short English window. It now has one trigger, showing the actual model and last-read native effort. The bounded popover has independently scrolling model and reasoning columns with existing Viewport/Reveal helpers. Search filters only the model column; reasoning continues to represent the current conversation. Unknown native level strings and diagnostics remain intact.

Extracted the existing reasoning body from its standalone Popover, removed that trigger/open flag, and reused the existing model and native reasoning setters/readers. On open, the shared trigger clears search/scroll positions and reads native settings. During native settings reads, model options are disabled while the panel itself stays mounted; native read state no longer unmounts the shared trigger. Existing busy/connection/loading/closing restrictions remain. No runtime/protocol/dependency/permission changes, favorites/service-tier controls or speculative provider fallback.

## Actual original Codex check

Evidence `.artifacts/model-effort-6847463/`. Owned state cloned the prior actual Terra thread, isolated Pi directory, saved original Codex 0.167.0 command. No model prompt in this step; other model names were metadata/focus traversal only.

- At approximately 822 × 450 logical pixels with sidebar open, input → two Tabs/Enter opened the combined panel. Actual model catalog and native levels appeared together; last-read Medium joined the trigger (`initial.*`).
- Clicked High, observed native applied High in selected option/trigger; restored Medium (`high.*`, `filtered.*`). Typed the exact Terra ID and selected the same Terra through the existing native model operation. The panel closed; Return reopened from restored trigger focus and read Medium again. No other model selected.
- From model search, ten Tabs reached the final actual catalog entry and revealed it (`model-bottom.*`). Seven more Tabs reached Reload in the independently scrolled reasoning column (`reasoning-bottom.*`), while the model column retained its own position. No non-Terra model or login control activated.
- Seventeen reverse Tabs returned search. An unmatched query exposed the complete accessible empty result and retained current reasoning controls (`empty.*`). Escape closed; composer attachment/model+Medium/usage/send fit a single row and the exact unsent Chinese draft remained (`toolbar.*`).
- Normal Cmd+Q exited 0, exec session 45927. Independent cold runtime open plus native reasoning read verified same thread, Terra, Medium, pending 0, exact draft and only the original one user delivery/two tools/one final. `check-cold.mjs` / `cold-receipt.json` record pass; no additional inference.

## Checks and boundaries

macOS build, Windows target check and `git diff --check` passed:

- `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`
- `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`

UI composition/state only: no new tests/full runtime suite; real native settings and cold restore were checked directly. Other backend combinations, busy/error transitions, programmatic auto-open, smaller widths/larger fonts, Chinese second pass, full VoiceOver and Windows native remain unverified for this new layout. The existing busy paths were not exercised with a fresh model turn here. Candidate ZIP remains `b539c26`; full product goal remains open.
