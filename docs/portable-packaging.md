# Portable development bundles

These are unsigned development bundles, not production installers. macOS bundles are not notarized. Build and package on the target operating system and CPU architecture; this workflow does not cross-compile or establish compatibility on another platform.

Install dependencies with `pnpm install --frozen-lockfile`, build the runtime with `pnpm build:runtime`, and build the native desktop executable. Then run:

```sh
pnpm --filter @hive-desktop/runtime deploy --prod --frozen-lockfile --node-linker=hoisted PATH_TO_NEW_RUNTIME_DIRECTORY
node scripts/package-desktop.mjs --executable PATH_TO_NATIVE_EXECUTABLE --runtime PATH_TO_NEW_RUNTIME_DIRECTORY --output PATH_TO_NEW_OUTPUT_DIRECTORY
node scripts/check-package.mjs --bundle PATH_TO_NEW_OUTPUT_DIRECTORY
```

The deployed runtime and output paths must not already exist. Run the deploy on the target OS and CPU immediately before packaging. macOS produces `Hive Desktop.app`; Windows produces a folder containing `hive-desktop.exe` and `resources`. Copy the whole app or folder together. The package includes Node 22.23.3 from the official Node distribution, verified against the official SHA256 list, its license, and the runtime's production dependencies. Node and dependencies must match the build host's platform and architecture. The check runs the bundled Node from a temporary directory, starts a real Hive host, creates a workspace without starting an orchestrator, checks shutdown and launches a real PTY; it sends no model request. The isolated PTY probe checks that its Node process exits naturally after the shell exit event. A Windows run observed shell exit but the probe process remained alive, with an active MessagePort and an additional PipeWrap. A platform-package patch closes the input socket and disposes the ConPTY output worker on the existing normal-exit cleanup path. Windows CI must verify that both direct PTY and Hive agent-manager probes exit naturally, including a stopped run; until then this fix is unverified on Windows. The check does not validate native window behavior or external Agent installations.

The dependency deployment uses pnpm 10.30.3 `deploy --prod --frozen-lockfile` with a hoisted layout and `injectWorkspacePackages` enabled. The configured Hive patch is applied during deploy. An earlier development attempt used legacy deploy and re-resolved dependencies; that attempt is not the locked bundle.

Windows CI is configured to run the runtime-location unit tests, build and package the desktop, extract the final zip into a separate folder, smoke the extracted runtime and PTY, and upload that zip as an artifact. These are configured checks, not a claim that a Windows run has passed; consult the actual CI run for its result. A successful macOS check is not Windows validation.

Windows CI run 42284937037 passed native compilation and all four runtime-location tests but failed packaging while launching the pnpm Windows shim from Node. The workflow now runs deploy directly in PowerShell before packaging; this correction awaits a new Windows CI run.
