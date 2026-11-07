# Hive native desktop

Actual GPUI application backed by the Node runtime, separate from the UI selection experiment.

From the repository root, build the runtime using `pnpm build:runtime`, then:

```sh
cargo +1.98.0 run --manifest-path apps/desktop/Cargo.toml
```

Development uses `node` from PATH and the runtime from the repository root derived from `CARGO_MANIFEST_DIR`. Portable bundles locate their own Node/runtime relative to the executable and add bundled Node to the child PATH. `HIVE_DESKTOP_NODE` and `HIVE_DESKTOP_RUNTIME` remain explicit overrides. External executable resolution uses `HIVE_CODEX_COMMAND` / `HIVE_PI_COMMAND`, then the app's external-program settings, then PATH. Saved changes apply to new processes and never modify agent-global configuration. Verified versions are Codex CLI 0.166.0 and Pi 1.4.0. See [portable packaging](../../docs/portable-packaging.md); these unsigned development bundles are not production installers.

Workspace creation calls Hive's real API with `autostart_orchestrator: false`. Create a conversation with builtin Pi, external Codex, external Pi or Claude; each retains its own backend identity and native session. Chat history, model availability, prompts, cancellation and streamed tool output come from that backend. User messages render literally; assistant messages use Markdown. No demo conversation or model request runs automatically.

Closing the window or pressing Cmd/Ctrl-Q sends `shutdown` and awaits cleanup of the owned sidecar scope: a private process group on macOS and a Job on Windows. It allows three seconds for graceful exit before terminating that scope; cleanup failure retains the window and original owner for retry. macOS processes that intentionally leave the group, including Pi's detached tools, are outside this confirmation boundary. The OS final-quit fallback uses zero grace and is best effort because GPUI limits quit observers to 200 ms. See [supervision evidence](../../docs/validation/2026-11-07-sidecar-supervision.md) for platform results and remaining limits.

Current surface: local workspaces, four structured backend integrations, pending delivery resume, collapsible tool details, collaboration overview, role templates and member create/rename/start/stop/delete controls. Starting a member requires an explicit provider/model. Builtin Pi, external Pi and Codex have real team dispatch/report and nonempty history restoration evidence. Claude uses the original native CLI through its official SDK; native controls, shared-session persistence and the UI are checked without inference, while real Claude collaboration remains unverified. Its executable setting uses `HIVE_CLAUDE_COMMAND`, app settings, then PATH. Unsupported approval and user-input requests fail visibly; the adapters never automatically accept them. External Pi dialogs without a responder are cancelled. Full settings, runtime reconnect, Windows native interaction and production installers remain separate work.
