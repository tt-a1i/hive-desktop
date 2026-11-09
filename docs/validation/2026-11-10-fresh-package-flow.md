# Fresh packaged Codex flow — 2026-11-10

Source: `ff86889e219db98cccbc099aecf8c859e313a3ac`. Additional staged development package; canonical candidate/current ZIP unchanged. No product code or tests changed in this slice.

## Package checks

Local evidence: `.artifacts/desktop-candidate-ff86889/`.

- `staged.zip`: 158,270,861 bytes; SHA-256 `e916601f4e2a4e0c2a596ff9ef3cf83342a1c1d8b6e976cc12b42c1f365b1d2f`.
- Independent extraction matched 28,863 entries, modes and 29 internal symlinks. Compared with 34aebbe, only the executable changed; all 112 runtime files and frozen dependency entries matched.
- `node scripts/check-package.mjs --bundle .artifacts/desktop-candidate-ff86889/extracted/bundle`, plus `check-packaged-models.mjs` and `check-packaged-dispatches.mjs` with the same argument, each exited 0. These request no inference.
- Post-GUI inventory and ZIP hash still matched. After process/handle checks, removed only the unused assembly `base/` duplicate: 620,457,984 allocated bytes. Extracted app, ZIP, all states, project fixtures and evidence remain. Physical free-space change was not measured.

## Actual native flow

Launched the independently extracted app with owned `fresh-state/` and `fresh-pi/`, without Node/runtime/Codex overrides. Process observation showed the native app owning bundled Node and bundled `runtime/dist/main.js`, which launched original Codex app-server.

1. Welcome → External programs → entered `/Users/tsk/.npm-global/bin/codex` → checked version `0.167.0` → saved.
2. System folder picker selected the owned project. Entered a draft, then native Agent menu created a Codex conversation retaining it.
3. Explicitly selected `openai/gpt-6.1-terra` / Medium before one native Send. Initial Sol metadata was never used for inference.
4. Requested one delayed Node command reading `input.json`, then an independent numeric sum/count. File bytes were `{"values": [7, 11, 23]}` plus newline. Expected numbers were not supplied in the prompt.
5. Native UI displayed the real command in progress/completed, then `PACKAGED_FF_OK 41/3`. During execution entered `重启后保留的草稿 · PACKAGED_FF_UNSENT` without sending/queueing.
6. Normal exit and cold launch restored reply, tool and exact draft. Both exits were 0.

`check-cold.mjs`, run using bundled Node/importing bundled runtime, verified one user input, one command result with exact fixture bytes and exit 0, one exact final reply, same native thread, pending 0, Terra/Medium, app-saved CLI path/actual version, unchanged project input and final/cold AX evidence. Cold checks send no inference.

Session `4c3acaa2-037a-491f-81da-81a32833232a`; thread `01a1c1ae-5557-71a0-9882-ada6bc8dd798`. See `tool.ax.txt`, `final.ax.txt`, `cold.ax.txt`, `cold-receipt.json`, `post-check.json`.

## Folder-picker observation remains open

First run: mouse Choose followed immediately by typing left the dialog visible. Subsequent Return completed selection and the project displayed HTTP 409. Cause is not established; the first handoff is not claimed as passing or fixed.

Isolated second fresh state: a different empty project, one mouse Choose, observe project page, immediately type without clicking input. This passed with `PICKER_SINGLE_CHOOSE_UNSENT`, no error banner, one SQLite workspace and zero messages. Normal exit 0. This narrows the observation but does not explain the first failure. No speculative guard/delay/fallback added.

The cold-check script initially expected different fixture JSON spacing and settings source `settings` instead of the existing typed `app` value. Corrected those expectations to actual fixture bytes/contract; production behavior unchanged. Final assertions passed.

## Remaining scope

Canonical candidate remains 34aebbe; current ZIP remains 1acd805. Staged ff86889 retained for investigation. This verifies packaged Codex execution/recovery, not full onboarding reliability, complete UI/reference parity, Windows native acceptance, optimized release performance or signing. No full suite run: no product code change; package regressions and actual GUI/runtime checks cover this slice.
