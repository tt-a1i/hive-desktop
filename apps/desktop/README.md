# Hive native desktop

Actual GPUI application backed by the Node runtime, separate from the UI selection experiment.

From the repository root, build the runtime using `pnpm build:runtime`, then:

```sh
cargo +1.98.0 run --manifest-path apps/desktop/Cargo.toml
```

The app uses `node` from PATH. `HIVE_DESKTOP_NODE` overrides that executable; `HIVE_DESKTOP_RUNTIME` overrides the runtime entry file. Development starts the runtime from the repository root derived from `CARGO_MANIFEST_DIR`. Packaging must supply the runtime and Node/native dependencies; an installer is not included here.

Workspace creation calls Hive's real API with `autostart_orchestrator: false`. Chat history, model availability, prompts, cancellation and streamed tool output come from the Pi runtime. User messages render literally; assistant messages use Markdown. No demo conversation or model request runs automatically.

Closing the window or pressing Cmd/Ctrl-Q sends `shutdown`, allows the owned Node child three seconds to exit, then kills/reaps it if necessary. This happens asynchronously before quitting GPUI. The OS application-quit fallback has a shorter grace period because GPUI limits quit observers to 200 ms. Forced termination cannot guarantee cleanup.

Current surface: local workspaces and Pi sessions. Team/member controls, external Agent adapters, runtime reconnect, Windows validation and distributable packages remain separate work.
