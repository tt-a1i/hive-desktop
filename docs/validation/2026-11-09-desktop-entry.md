# Desktop entry and design references — 2026-11-09

This is an entry-page slice, not full design or feature parity.

## Reference observations

- Installed T3 Code (Beta): observed the existing new-thread page and General settings, then returned to the same thread. Project selection, composer, model/effort and checkout context are colocated; settings have a searchable grouped navigation. No messages or preferences were changed.
- Installed Beluga: observed the welcome and agent-selection step, then returned to the welcome where the user left it. Quiet welcome surface, one main action, agent detection and explicit unavailable states are useful patterns. Its visible progress advertised 22 steps; Hive does not need to copy that length or request unrelated OS permissions.
- Beluga source pinned at `cab72cb7a85efef601651c516d4ddae46fde516f`: [Onboarding.tsx](https://github.com/kayraucklnc/beluga/blob/cab72cb7a85efef601651c516d4ddae46fde516f/packages/mac/src/onboarding/Onboarding.tsx), plus its repository README. Source confirms separate introduction, agent, tour, appearance and access sections. No third-party code, icons or branding imported.
- Cindy remains the collaboration/settings reference already documented; T3 Code and Beluga now guide visual and entry interaction decisions alongside it.

## Changed behavior

Previously the no-project view used an empty composer with unavailable actions. It now presents one primary project-folder action, optional Pi account configuration and external-program setup, plus three concise steps explaining the real project → lead → team flow. All actions reuse existing handlers. Opening the view creates no Agent or chat. Existing project/session navigation remains authoritative; no completion flag or duplicate onboarding configuration was added.

The existing project welcome title now asks what to work on. Actual conversation rendering is unchanged. New static text has English translations; title, description and step explanations expose native accessibility labels.

## Native verification

Evidence directory: `.artifacts/design-reference-4b3aa5f/`.

- Chinese welcome observed, Pi account and external-program entries opened the correct existing pages. Native directory picker opened; cancel returned to the welcome.
- Final English isolated state: all three step explanations appeared in full AX output; normal and approximately 822 × 597 logical-pixel windows inspected. Narrow content scrolled until the entire external-program button was visible; clicked it and observed the External programs settings, then returned.
- Corrected viewport height: the outer flex layout must fill available height for the existing focus/scroll viewport; it otherwise sizes to content. Added a tracked scrollbar using the existing component. Temporary dimension logging was removed.
- Automation scroll direction follows the machine's current mapping: `up` moved content toward the bottom. Earlier `down` calls stayed at offset zero. This was distinguished from the layout constraint; no vendor wheel mapping was changed.
- Final screenshot and full AX: `hive-welcome-en-narrow-bottom.png`, `hive-welcome-en-final.ax.txt`. Initial captures predate the final layout and are not final rendering evidence.
- Final owned process exited normally (exec session 97179 exit 0). Owned SQLite retained zero workspaces and zero messages. No model request, account sign-in or credential mutation was performed. An earlier launch inspected shared Pi account metadata read-only; subsequent English runs used the owned state/agent directories.

Commands passed:

```sh
cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml
PATH="/opt/homebrew/opt/llvm/bin:$PATH" cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc
git diff --check
```

No backend/protocol change, new test, or full suite for this UI slice. Windows typechecking does not prove Windows native behavior. Light-theme, full VoiceOver, complete keyboard traversal, project creation end-to-end on this page, and broader T3/Beluga parity remain to verify. The existing candidate/current ZIPs were not rebuilt or promoted.

## Project storage

After builds and owned GUI exit, checked no cargo/rustc process and no lsof handles in either exact incremental directory. Removed only macOS and Windows-target Rust incremental caches: 1,237,602,304 allocated bytes (about 1.15 GiB). Preserved dependency build outputs, executable, source/vendor, all evidence and ZIPs. Receipt: `storage-cleanup.json`. These caches will regenerate when needed; this is not automatic deletion of project state.
