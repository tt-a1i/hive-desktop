# Hive native desktop

Actual GPUI application backed by the Node runtime, separate from the UI selection experiment.

From the repository root, build the runtime using `pnpm build:runtime`, then:

```sh
cargo +1.98.0 run --manifest-path apps/desktop/Cargo.toml
```

The app uses `node` from PATH. `HIVE_DESKTOP_NODE` overrides that executable; `HIVE_DESKTOP_RUNTIME` overrides the runtime entry file. `HIVE_CODEX_COMMAND` selects the external Codex executable; otherwise it uses `codex` from PATH. The current adapter is verified against Codex CLI 0.166.0; an older CLI may not support its app-server protocol. Development starts the runtime from the repository root derived from `CARGO_MANIFEST_DIR`. Packaging must supply the runtime and Node/native dependencies; an installer is not included here.

Workspace creation calls Hive's real API with `autostart_orchestrator: false`. Create a conversation with builtin Pi or external Codex; each retains its own backend identity and native session. Chat history, model availability, prompts, cancellation and streamed tool output come from that backend. User messages render literally; assistant messages use Markdown. No demo conversation or model request runs automatically.

Closing the window or pressing Cmd/Ctrl-Q sends `shutdown`, allows the owned Node child three seconds to exit, then kills/reaps it if necessary. This happens asynchronously before quitting GPUI. The OS application-quit fallback has a shorter grace period because GPUI limits quit observers to 200 ms. Forced termination cannot guarantee cleanup.

Current surface: local workspaces, Pi and Codex orchestrator/worker conversations, pending delivery resume, and collapsible tool details. Both backends have real team dispatch/report and nonempty history restoration evidence. Codex approval and user-input requests currently surface an unsupported-interaction error and interrupt; they are never automatically accepted by this adapter. Full team/member controls, external Claude Code/Pi adapters, runtime reconnect, Windows validation and distributable packages remain separate work.
