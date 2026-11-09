# Optimized member/workflow development package — 2026-11-10

Current and candidate now share the same Mac ARM64 release bytes from `7ae27ca400251a7eb8f7d167532b2b52a86c62f0`:

- ZIP: `.artifacts/hive-desktop-macos-arm64-current.zip` and `.artifacts/hive-desktop-macos-arm64-candidate.zip`, hardlinked aliases.
- Size:143,404,131 bytes. SHA256:`a4a448fe35a86639f7a85b1d8685108cc4d9fe82fb025495767ab827e94dd3a3`.
- Binary SHA256:`915ebeff953d173452465c1c370c3dda90330daff3aa6c7febc824dfaf5f71f9`.
- Runnable app:`.artifacts/release-7ae27ca/extracted/bundle/Hive Desktop.app`.
- Receipts/inventories/native evidence:`.artifacts/release-7ae27ca/`.

This refresh adds the member and workflow keyboard changes from `4d53de5`, `62abe27`, `6592d7b`, plus the focus handoff correction below. It retains the earlier setup/configuration and runtime behavior. Full product acceptance remains false.

## Initial package caught an additional focus failure

The first optimized assembly at `6592d7b` passed all six backend package gates, and actual member rename/model traversal worked. However, immediately typing after model-save did not update member search: `members-saved.ax.txt` retained `Added-by-UI` instead of the typed `Packaged-06`. This assembly was not promoted. Its ZIP, owned states, complete log/AX/screenshots and assembly inventory remain under `.artifacts/release-6592d7b/`.

The shared member-action success callback assigned the pending search-focus flag from rename/delete state. For model-save both were absent, so a fast successful callback could erase the focus intent before rendering consumed it. `7ae27ca` preserves existing intent with `|=` and consumes it only after the member write/read finishes. No timer, input replay, backend behavior or new focus state was added.

The new optimized package reproduced the same operations successfully: save the actual Terra catalog entry, then Cmd+A/type without clicking a field changed member search to exactly `Packaged-06` (`focus-repaired`). Rename then moved that member out of the old filter; direct typing after refresh found exactly `Release-7ae-06` (`rename-search`). This is the evidence for the correction; passing backend gates alone did not detect it.

## Bundle and backend gates

Built with `cargo build --release --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`. Compared source metadata/locks/patches to the previous accepted package; no production dependency change. Copied the frozen dependency baseline (`1534986f992fe011cef94706618bf2ee2acf40df`) and replaced only the native executable. All112 runtime outputs matched source. Independent ZIP extraction matched28,863 entries/modes, including29 internal symlinks. The only changed entry versus the accepted `edbbae1` package is the native executable.

All six gates ran against this exact extracted bundle and exited0:

- `check-package`: real bundled Node, HTTP/SQLite/PTY and lifecycle checks.
- `check-packaged-models`: real SDK/configuration and reload checks, no inference.
- `check-packaged-dispatches`: actual Hive dispatch/store contract checks.
- `check-packaged-codex`: synthetic native-protocol contract checks, no vendor inference.
- `check-packaged-remote`: loopback remote contract checks.
- `check-packaged-claude`: official SDK with the explicit-handshake synthetic CLI, no Claude inference.

The debug Mac build and Windows target check passed for the two-line focus correction. No UI test files were added or modified; no full runtime suite was run for these native UI-only changes. Gates, logs, `gate-results.json` and manifest support the exact scopes above.

## Native use of extracted application

Launched the extracted executable with `HIVE_DESKTOP_RUNTIME` and `HIVE_DESKTOP_NODE` unset, using three separate owned data/Pi directories. The app used its own Resources runtime/Node. No inference was requested in this round.

1. **Members:** thirteen stopped members, actual Codex0.167.0 ten-entry catalog, Terra save and direct search handoff, correct single-member rename, Dark20 compact window, last-model/reverse keyboard traversal and close-to-search. Read-only SQLite verified all original IDs/roles/descriptions/configs unchanged except the intended member name. Messages, dispatches, Agent runs/sessions and workflows stayed0.
2. **Workflows:** nine retained real historical records and one paused schedule. Keyboard traversed to the ninth run and back to the dispatch-containing first run. Actual disk artifact Preview opened; workflow→reported dispatch→same workflow round trip succeeded. Delete confirmation was cancelled, never confirmed. Next Tab after Cancel focused/revealed the feature row, proving toolbar focus restoration. All run results/statuses, logs, schedules, dispatches/reports, messages and Agent execution records matched the prepared source copy exactly.
3. **Existing real Codex conversation:** the new app cold-opened the original `edbbae1` Terra/Medium session/thread, expanded its actual completed command/output `{"values":[13,19,35]}`, displayed `RELEASE_EDB_OK 67/3` and retained exact unsent `完整包内草稿 · RELEASE_EDB_UNSENT`. Independent bundled-runtime open after native quit confirmed exactly one original user input, one original successful tool result, one original reply, pending0 and idle. This is recovery of the earlier real inference, not a new inference or a new team-collaboration test.

All three final native processes exited0; the bundled conversation checker also exited0. `state-receipt.json` and `conversation-receipt.json` passed. Member/workflow chat indexes stayed empty; owned auth files remained absent. Original state/project directories were preserved.

## Promotion, cleanup and remaining scope

Archived both old `edbbae1` aliases as `.artifacts/release-edbbae1/current.zip` and `candidate.zip`, retained their hashes/receipts and the previous runnable bundle. New current/candidate receipts name this exact source/hash. Post-GUI/post-promotion inventory and archive hashes matched.

Only after exact-path `lsof` checks found no references, removed the new assembly base and the rejected assembly's base/extracted bundle:1,615,147,008 allocated bytes by file-block accounting. Rejected ZIP, failure evidence, every state/project and the prior accepted ZIP/runnable bundle remain. This is removal of rebuildable copies; it is not deletion of old chat working directories or an exact APFS free-space measurement.

The package is an unsigned development bundle. Windows native, signing/notarization/installation, complete VoiceOver/IME/language/font/window matrices, Pi reauthentication, real Claude inference, production remote gateway/phone and final destructive schedule action remain open. Actual previous team tests remain separately scoped; this refresh does not establish complete parity with T3 Code/Beluga/Cindy/Maka or complete the desktop goal.
