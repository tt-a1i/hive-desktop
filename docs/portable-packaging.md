# Portable development bundles

These are unsigned development bundles, not production installers. macOS bundles are not notarized. Build and package on the target operating system and CPU architecture; this workflow does not cross-compile or establish compatibility on another platform.

Install dependencies with `pnpm install --frozen-lockfile`, build the runtime with `pnpm build:runtime`, and build the native desktop executable. Then run:

```sh
node scripts/package-desktop.mjs --executable PATH_TO_NATIVE_EXECUTABLE --output PATH_TO_NEW_OUTPUT_DIRECTORY
node scripts/check-package.mjs --bundle PATH_TO_NEW_OUTPUT_DIRECTORY
```

The output path must not already exist. macOS produces `Hive Desktop.app`; Windows produces a folder containing `hive-desktop.exe` and `resources`. Copy the whole app or folder together. The package includes Node 22.23.3 from the official Node distribution, verified against the official SHA256 list, its license, and the runtime's production dependencies. Node and dependencies must match the build host's platform and architecture. The check runs the bundled Node from a temporary directory, starts a real Hive host, creates a workspace without starting an orchestrator, checks shutdown and launches a real PTY; it sends no model request. It does not validate native window behavior or external Agent installations.

The dependency deployment uses pnpm 10.30.3 `deploy --prod --frozen-lockfile` with a hoisted layout and `injectWorkspacePackages` enabled. The configured Hive patch is applied during deploy. An earlier development attempt used legacy deploy and re-resolved dependencies; that attempt is not the locked bundle.

Windows CI is configured to run the runtime-location unit tests, build and package the desktop, extract the final zip into a separate folder, smoke the extracted runtime and PTY, and upload that zip as an artifact. These are configured checks, not a claim that a Windows run has passed; consult the actual CI run for its result. A successful macOS check is not Windows validation.
