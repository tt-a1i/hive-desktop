# Hive Terminal

GPUI grid view adapted from the MIT-licensed gpui_xterm 0.2.1. Source and changes
are recorded in NOTICE. This crate does not create a process, PTY or I/O thread.

Create an `Entity<TerminalView>` with `TerminalView::new(TerminalConfig::default(), cx)`
and subscribe to `TerminalEvent`. The host owns attachment identity, disconnection,
restore ordering and the bounded output transport.

- `feed(bytes, cx) -> usize` parses synchronously. Only its returned byte count may
  be acknowledged to the upstream terminal stream.
- `reset(cx)` clears the actual parser, grid, selection and IME composition.
- `resize(cols, rows, cx)` applies an authoritative restore size without emitting
  a resize event. Layout changes emit `Resize` after the local grid changes.
- `set_input_enabled(enabled, cx)` gates manual keyboard, IME and paste input.
  New views default to disabled; enable only after restore is complete. Native
  protocol replies still emit with `user_input: false` while manual input is
  disabled; the host owns attachment validation and transport readiness.
- Render the entity directly. `focus_handle()` supplies its terminal focus handle.

`Input { bytes, user_input }` carries bytes for the host's I/O channel. Keyboard,
committed IME and paste are user input; native cursor-position replies are not.
`Resize { cols, rows }`, `Title(String)` and `Bell` are the other host events.
There is no separate acknowledgement queue or snapshot store in the component.

Mac uses Cmd-C/V, Windows/Linux use Ctrl-Shift-C/V, and the context menu provides
explicit copy/paste. Ctrl-C still reaches the shell. Paste honors the native
BRACKETED_PASTE mode. ANSI OSC 52 cannot read or replace the system clipboard.
IME preedit belongs to the view and is sent only on commit. Parser/UTF-16 checks
are not evidence that a platform IME or clipboard has passed native UI validation.

Run the focused real-parser checks from the repository root:

```sh
cargo +1.98.0 test --manifest-path apps/desktop/Cargo.toml -p hive-terminal --lib
```
