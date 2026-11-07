# Hive Desktop GPUI patch

Base: the complete published gpui-pre 0.3.11 crate, Apache-2.0.
Published crate SHA-256: `9303c771abf2acff2a96d706f69d9a82088c4770173a07cce88f0002431d887d`.
Source metadata: Zed snapshot `f816e1f9263e3a8ef63c4baaa02e1441ce1c6038`.

The only production source change is in `Window::dispatch_event` in src/window.rs:
classify `PlatformInput::FileDrop` as mouse input before dispatch. Previously a
preceding key press left keyboard modality active. `HitboxId::is_hovered` then
returned false, suppressing both drop hover and the registered on_drop callback.
No extra normal MouseMove is guaranteed between native FileDrop events.

A copied native app fixture reproduces paste then FileDrop::Entered/Submit with
no synthetic normal mouse movement. Before: one upload remains, no drop callback.
After: the dropped image creates a second upload, and the draft remains isolated.
The same fixture also exercises native paste, actual upload/SQLite, send, failure
retention, restart and full-image viewing. See docs/validation/2026-11-08-image-input.md.

Cargo uses this directory through apps/desktop/Cargo.toml. Keep this delta narrow;
remove the override once a pinned upstream release includes this event fix.
The published license and all original crate files are preserved. Cargo's local
`.cargo-ok` cache marker was excluded. No test-rendering hooks are shipped here.
