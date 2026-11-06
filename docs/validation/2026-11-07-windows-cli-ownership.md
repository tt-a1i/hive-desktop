# Windows native CLI ownership

## Observed failure

Two actual Windows no-inference jobs failed before this change:

- [42299029525](https://github.com/tt-a1i/hive-desktop/actions/runs/42299029525): official CLI installation succeeded; first bare Codex case failed while awaiting close.
- [42300103363](https://github.com/tt-a1i/hive-desktop/actions/runs/42300103363): initialization and `model/list` (9 items) succeeded. After stdin EOF, cmd/Node/Codex exited with code 0, while two original Git processes and a conhost remained alive by PID **and creation time**, holding inherited stdout/stderr open. The diagnostic receipt is [artifact 13544171256](https://github.com/tt-a1i/hive-desktop/actions/runs/42300103363/artifacts/13544171256).

This is a real lifecycle defect: `taskkill /T` cannot follow the original process tree once its ancestors have exited. Waiting longer, destroying the local streams or accepting the root exit would not establish descendant cleanup.

Codex 0.166.0 has a detached marketplace-upgrade thread in [manager.rs](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/core-plugins/src/manager.rs#L2764), and that path's [Git launcher](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/core-plugins/src/marketplace_upgrade/git.rs#L170) creates ordinary children and kills only on its timeout branch. Another git-utils path uses a Job. These source facts establish why native exit is insufficient for all Git paths; the CI did not capture the specific Git subcommand, so its exact upstream call site remains unproven. No credentials, environment or arbitrary Git command lines were collected.

## Bounded implementation

On Windows only, `OwnedCliProcess` starts a small Node launcher. Before starting the user's original CLI, the launcher assigns itself to a Windows Job with `KILL_ON_JOB_CLOSE` and no breakaway permission. It then uses the existing `cross-spawn` argv handling for native executables and npm/pnpm shims. The parent still awaits natural stdio close, with the existing bounded forced-shutdown path if the live launcher does not stop.

When the original CLI root closes, the launcher sets its exit code and lets its event loop end naturally. It never calls `process.exit`, `job.close` or `job.terminate` to force a test result. Windows closes the launcher's sole, non-inheritable Job handle on exit, terminating remaining Job descendants even after their original parents disappeared. The launcher joins the Job **before** spawning, avoiding the race of assigning an already-running CLI. The main Node runtime and other sessions are outside this Job.

Mac retains its existing direct launch and shutdown route. No CLI configuration, credentials, approval policy, model routing or collaboration ledger is changed. This is process ownership, not a sandbox; processes created outside this ancestry through services such as WMI are not included.

## Dependency provenance and cost

The runtime pins `@seydx/win-job` **0.1.0**, npm `gitHead` **84948f5579bf7b0eef4198e30e1740cefd713a6b**. Reviewed sources: [native Windows operations](https://github.com/seydx/win-job/blob/84948f5579bf7b0eef4198e30e1740cefd713a6b/src/windows.rs) and [binding API](https://github.com/seydx/win-job/blob/84948f5579bf7b0eef4198e30e1740cefd713a6b/src/lib.rs). The handle has no Drop implementation, so JavaScript garbage collection does not close it prematurely; explicit close or process exit releases it. Microsoft documents [Job inheritance and kill-on-last-handle-close](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

Measured registry unpacked sizes: root package 41,386 bytes, Windows x64 platform package 285,695 bytes, ARM64 260,620 bytes. The root has no OS/CPU restriction or install script; the platform binaries are optional dependencies. Its non-Windows entry is a stub, and only the separate Windows launcher imports the binding. Cost is one extra Node process per active Windows CLI and this small native dependency. It is a young 0.1.0 dependency, so version pinning, source review, actual Windows execution and platform-asset packaging checks are part of acceptance.

## Actual Windows validation

Commit **1914a06**, [Windows run 42302725509](https://github.com/tt-a1i/hive-desktop/actions/runs/42302725509), `native-clis` job: **passed on Windows x64**. The receipt reports `pass:true`, `platform:win32`, `inference:false` and `runtimeArtifact:compiled dist/agents/*.js including owned-windows-cli-host.js`. It is saved as `.artifacts/windows-native-clis.json` in the job's receipt artifact; the inspected local copy was `/tmp/hive-native-job-fix/windows-native-clis.json`.

All nine cases completed their assertions:

| Scope | Actual checks |
| --- | --- |
| Official Codex 0.166.0 | Bare `codex`, explicit official `.cmd` in an installation path containing spaces, and native `codex.exe`; initialize, model catalog, stdin EOF and awaited close. |
| Official Pi 1.4.0 | Bare `pi` and explicit official `.cmd`; RPC initialization/state/messages, paths containing spaces, no model start, stdin EOF and awaited close. |
| Unexpected native exit | The check killed only its owned native Codex or Pi descendant; subsequent requests were rejected and the owned tree was gone after awaited cleanup. |
| Separate natural-root fixture | A synthetic root left an explicitly detached inherited-stdio child. The launcher actually exited with code **17**, no grace deadline expired, and all original process identities disappeared. This verifies natural exit and Job cleanup without `process.exit` or forced tree termination masking the result. |
| Separate forced-tree fixture | A synthetic stubborn Node tree required bounded shutdown; all captured process identities disappeared afterward. |

Successful official Codex cases captured Git, Git remote-helper and conhost descendants before stdin EOF. After close, the checks verified that every captured PID **with its original creation time** was absent. Thus this run covers actual background Git/console descendants as well as the separate deterministic orphan fixture. It still did not collect the Git subcommands, so the exact upstream Git call site remains unproven.

The former taskkill-only limitation is resolved for descendants already owned by the session Job when their original CLI parents exit. This run directly verified normal CLI exit, unexpected native-child exit and controlled forced-tree shutdown. It did not separately kill only the launcher without `/T`, crash the entire parent runtime during inference, or exercise processes created outside Job ancestry through services such as WMI; those should not be described as independently tested guarantees.

Runtime/check-script typechecks, runtime build, Mac real Pi RPC, native Codex metadata/control/close/death checks and the synthetic Codex JSONL peer also passed without inference. Packaging-owner isolated Windows x64/ARM64 deploys selected the corresponding native binding; ARM64 asset selection is **not ARM64 execution**.

## Additional launcher-death case: pending Windows execution

After the nine-case run above, the existing stubborn-process fixture was reused for one additional check: keep stdin open, capture the owned launcher/child/grandchild identities, kill **only** the launcher PID with `process.kill(..., 'SIGKILL')` (no `/T`), then await outer close and verify all original identities are gone without grace expiry. Only the check script changed; production code did not. Its TypeScript/diff checks passed, but this new case is **not part of the nine-case Windows success** and awaits the next actual Windows job.

## Remaining boundaries

This is Windows **x64 CI process/protocol validation**, not native GUI interaction or a Windows ARM64 run. No model inference, model prompt, active LLM tool execution, model-driven team dispatch/report loop, or cancellation/process death during an active LLM tool was exercised. Synthetic process fixtures remain separate from official CLI controls and are not team-live evidence. The Mac route was preserved and regression-checked; this receipt does not extend the Windows results to other platforms or untested CLI versions.
