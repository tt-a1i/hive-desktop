# Pi account status and keyboard reading — 2026-11-09

Source baseline `aa69b8d`. In the previous native [setup inspection](2026-11-09-welcome-setup.md), account provider names, locally saved status, count and explanatory text were visible but absent from AX; only action buttons were exposed. Source inspection also found this long list lacked the existing focus/reveal viewport used by other settings pages.

## Change

Reuse shared selectable text for provider names/status, account description/notices, count, empty/loading state and storage caveat. Provider rows expose a native group named with provider and exact existing saved status. Authentication action accessible names now use their actual visible labels (including re-login/update). Existing known local status meanings remain unchanged; no claim of valid credentials or server connectivity is added.

Wrap the account scroll container with existing `focus_scroll::Viewport`; toolbar/search/provider groups use existing Reveal. No new focus algorithm, dependencies, backend or credential behavior. The initial build caught an owned provider ID moved into an existing remove callback; the reveal key now uses the borrowed provider ID. Final builds pass.

## Actual native evidence

Evidence `.artifacts/pi-account-reading-aa69b8d/`. Fresh owned English state, isolated empty Pi directory, offline mode. No real login/API operation started and no model request sent.

- Approximately 822 × 450 logical pixels: native AX now contains 42 provider groups, names, locally absent status, count and complete caveat.
- Focused provider search, then 30 Tabs: the actual OpenAI Codex legacy subscription button scrolled into view with its provider/status and visible focus ring (`keyboard-forward.*`). No activation key pressed on login actions.
- Thirty reverse Tabs returned the search field into view (`keyboard-return.*`). Typed an unmatched query: 0 providers and the complete empty explanation were accessible (`empty.*`). Replaced with OpenAI: actual search matched Azure, OpenAI and legacy Codex, count 3 (`filtered.*`); not assumed to match only names.
- Switched only the owned application's language to Chinese. Search query stayed OpenAI, and count, provider groups/status and storage caveat appeared in Chinese (`chinese.*`). Reverse Tab from provider search twice exposed the refresh toolbar (`toolbar-return.*`); the left edge of its focus outline is partly clipped at the scroll boundary, so this is not full visual focus acceptance.
- Normal Cmd+Q returned 0, exec session 4090. Read-only SQLite/artifact assertions verified zero workspaces/messages, no chat index and empty SDK-created auth storage; `receipt.json` records pass. No credentials changed outside the owned test directory.

Checks passed: macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, Windows target `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` with LLVM on PATH, and `git diff --check`. UI-only change: no new tests/full runtime suite. Existing authentication implementation/tests untouched.

Saved OAuth/API row variants, active login/prompt flows, loading/error transitions, text copying, complete focus outline/VoiceOver/compact-width/font matrix and Windows native remain separate acceptance. Candidate ZIP remains `b539c26` and does not include this new source change. The complete product goal stays open.
