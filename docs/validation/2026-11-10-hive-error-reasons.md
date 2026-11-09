# Hive HTTP error reasons — 2026-11-10

## Problem and change

The fresh packaged onboarding check exposed a generic HTTP 409 notice. `HiveHttpError` already retained the server response body and `main.ts` serialized it, but its message contained only the status, and the current Rust transport displays that message. Users could not distinguish an existing project from another conflict.

The shared error constructor now appends a nonempty string `body.error` from the actual Hive response. Status and structured body are unchanged. Non-object bodies and non-string/empty error values retain the existing status-only message. It does not stringify arbitrary response bodies, swallow failures, retry requests, or change workspace creation/conflict semantics.

This improves every existing caller of the shared error constructor. It does not establish why the first folder-picker sequence submitted conflicting work; that observation remains open.

## Verification

Evidence: `.artifacts/desktop-candidate-ff86889/error-*`, `duplicate-error.ax.txt`, `missing-error.ax.txt`, and `native-error-detail.log`.

- Extended the existing `packages/runtime/checks/hive-host.ts` with a real HTTP/SQLite duplicate-workspace request. It verifies status, actual server reason, displayed message, and the existing subsequent workspace-list assertions ensure only one project exists. The added check failed before the change (exit 1: message lacked the reason), then passed after it (exit 0).
- `pnpm --filter @hive-desktop/runtime build` and `pnpm check` passed. No full suite or Rust rebuild: the change is limited to the TypeScript error message and its real integration check.
- Actual native app, with an explicit development runtime override: opened the owned existing project and submitted its path again. The visible/accessible notice included `Hive request failed (409): Workspace path already exists:` followed by the correct path.
- Submitted a nonexistent owned path. The visible/accessible notice included `Hive request failed (400): Workspace path does not exist:` followed by the correct path. The attempted path stayed in the form; `PICKER_SINGLE_CHOOSE_UNSENT` remained in the composer and persisted unchanged after exit.
- Normal native exit 0; final SQLite contained one workspace and zero messages. No chat or inference was started. `error-detail-receipt.json` records these assertions.

The first draft of this UI check accidentally reused an AX index after a full capture reset indexes; it failed with `elementHasNoFrame`. A fresh full tree identified the actual field, and the nonexistent-path check then passed. No product change was made to accommodate the automation.

## Boundaries

This is a source/runtime validation, not a refreshed bundle: the native executable came from the staged ff86889 package and `HIVE_DESKTOP_RUNTIME` explicitly selected the newly built source runtime. Canonical candidate 34aebbe, staged ff86889 and current 1acd805 archives remain unchanged. Windows native behavior, first-picker 409 reproduction, and full product acceptance remain open. No existing tests were removed or skipped to make the check pass.
