# Narrow composer and native usage — 2026-11-09

Source baseline: `4905ea0`; this source change is not yet in the `d4c57a1` candidate ZIP.

## Problem and change

At roughly 822 × 450 logical pixels with English UI and the workspace sidebar open, the single-row composer placed Send outside its card. Opening inline native usage pushed input/actions below the window. The welcome heading also truncated mid-sentence.

Extracted existing composer actions into attachments/commands, wrapping model/reasoning/usage, and fixed send/stop groups. Existing handlers/enabled conditions remain. Native usage uses the existing GPUI Popover and shared keyboard-scroll reader bounded by the viewport; it no longer changes composer height. The welcome heading wraps. Known application-owned usage labels/notices are localized; unknown/model strings stay intact. Report groups/counters expose complete labels and exact values through accessibility APIs. No runtime token calculation, queue protocol, backend or permission behavior changed.

This follows the shared footer/action boundary observed in T3 Code; see [pinned source references](../research/2026-11-09-desktop-source-references.md). No upstream source/assets copied or dependencies added.

## Actual native evidence

Evidence: `.artifacts/narrow-chat-4905ea0/`. App-local English state and isolated Pi directory; original Codex 0.167.0 selected through saved app settings. All inference explicitly used `openai/gpt-6.1-terra`, Medium. No global authentication/default changes.

- `before.png` reproduces overflow; `grouped.png` shows grouped controls and wrapped welcome heading.
- Sent exactly: `Read README.md using a tool, do not modify any files, then reply exactly NARROW_UI_OK.` The actual native thread completed `team list` and `cat README.md`, then one final `NARROW_UI_OK`.
- `running.png` / `running.ax.txt`: actual busy state exposes Stop/Queue inside the card. Typed an unsent Chinese follow-up while running; did not click Queue or Stop.
- `live-usage.png` / `live-usage.ax.txt`: actual native totals visible and accessible in English. Thread input 61481, output 128, native total 61609; latest input 30837, output 9, total 30846. Capacity 475000 is the last reported capacity, not claimed live occupancy or billing.
- `usage-end.jpg` / `usage-end.ax.txt`: focused the reader and used End to reach the final capacity/notice in the short viewport. Send remained visible. Escape closed the popup and retained the draft. `completed.jpg` / `completed.ax.txt` show the final reply/draft.
- Native exec session 13613 exited normally, code 0. Independent cold runtime open resumed the same Codex thread, Terra/Medium, one user delivery, two successful tools, one exact final, pending 0 and unchanged unsent draft. README matched the initial owned fixture. `check-cold.mjs` / `cold-receipt.json` record assertions; cold open issues no prompt.

The initial cold self-check expected null rather than the API's absent failure field, then expected an extra blank line in the fixture README. Corrected those check expectations; no production changes to satisfy them. Final check exited 0.

## Checks and limits

- `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml` — passed on macOS arm64.
- `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc` — passed; no Windows native acceptance implied.
- `git diff --check` — passed.

UI-only change: no new UI tests or full runtime suite. Actual Queue/Stop activation, complete focus return/VoiceOver, every width/font/backend/theme and Windows GUI remain separate acceptance items. This is one verified chat path, not complete T3/Beluga/Cindy parity. Candidate/current packages were not replaced; the full product goal remains open.
