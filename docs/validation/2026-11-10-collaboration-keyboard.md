# Collaboration keyboard navigation — 2026-11-10

Source baseline `96655a8` plus this UI fix. Evidence `.artifacts/team-navigation-96655a8/`. Final native development binary verified; the fb3dcd0 candidate ZIP does not include this change. Related full keyboard matrix #20 and product goal #1 remain open.

## Reproduction and scope

English, Dark, 16px, approximately 592×500 logical pixels. Reused an owned copy of the actual late Worker report fixture: two original Codex/Terra threads, one reported dispatch, completed workflow and numeric result `{sum:37,count:3}`. No new inference was requested. The ephemeral Worker has already left the live team; its historical conversation remains available. The dispatch message table is empty in the original fixture too; the genuine report resides in the ledger. Neither condition was fabricated or repaired.

With a 40-line task draft, three Tabs from the editor focused Cancel editing outside the viewport. No button or focus outline was visible, but Space actually cancelled the editor (`task-cancel-offscreen.*`, `task-cancelled.*`). Edit tasks also did not explicitly focus the input. After route changes the prior keyboard origin could remain on unrelated navigation, making a counted Tab sequence unreliable.

The first launch found the fixture had no saved executable path and inherited an old incompatible Codex on PATH. The existing UI/runtime error was real. Saved the installed original Codex 0.167.0 in this fixture through the existing settings API (204 response, subsequent GET verified), then reopened history. This is fixture preparation, not a production fix or compatibility claim about the old CLI. A first assertion expected 200 and was corrected to the actual 204; the successful write was not repeated.

## Changes

- Extracted existing collaboration rendering into `collaboration/view.rs`; preserved routes, task persistence, filters, reports, pagination and member lookup.
- Reused existing Viewport/Reveal and ScrollHandles around task actions, filters, dispatch cards, pagination, detail links, artifact actions and message controls. Small content inset preserves focus outlines. No editor key interception or new scrolling abstraction.
- Explicit Edit tasks focuses its draft input. Opening collaboration/details focuses the matching fixed navigation action when that pane is visible; returning to the list focuses Refresh. Returning from a member chat restores the details navigation focus. Existing mouse navigation remains available.
- Shared member-session opening focuses the composer after switching the real session; updated its three callers. No message is sent by navigation.
- Corrected English singular `1 artifact`.

## Actual verification

1. Same small window: actual Edit tasks → paste 40 lines worked without a second field click. Forward/reverse Tab exposed Save, Copy and Cancel with visible outlines. Space on Cancel discarded the unsaved draft. Saved captures `task-save-focus.*`, `task-copy-focus.*`, `task-cancel-fixed.*`.
2. Explicit Copy → paste back into the owned task input preserved the complete 1,912-character document. AX text snapshots cap the textarea value at 512 characters, so full-copy correctness was checked against the actual persisted draft, not inferred from AX. Then native Save wrote all 40 lines and removed the draft file. `copy-draft-receipt.json`, `task-save-receipt.json`, `tasks-expected.md`.
3. Save testing used an owned project copy. While the app was stopped, only this fixture's workspace path was redirected to that copy (`project-copy-receipt.json`). This is not a product workspace relocation test. Every hashed file in the original project stayed unchanged.
4. Long document → filter actions and reported dispatch card scrolled into view through keyboard navigation (`dispatch-filters-fixed.*`, `dispatch-card-fixed.*`). Actual detail entry showed Back to dispatch list focus (`detail-entry-final.*`). From that known focus, six Tabs revealed Preview with its outline; Return opened the real result JSON in the read-only preview (`artifact-preview-*-final.*`). Escape closed it.
5. Four reverse Tabs exposed the owner's conversation link with visible focus; Return opened the existing Late-worker history and Back to dispatch details breadcrumb. Typing `MEMBER_UNSENT_FOCUS_PROOF` without another input click created only an unsent draft (`member-*-final.*`). Clicking the breadcrumb restored Back to dispatch list focus; Return restored Refresh focus; one Tab visibly revealed Edit tasks (`detail-return-final.*`, `list-return-final.*`, `list-edit-focus-final.*`).
6. All three native processes quit normally (exit 0). Independent cold runtime check restored the same two thread IDs, pending 0/busy false, original two main finals, reported ledger and exact artifact. Exact saved task document, cleared task draft, unsent member draft and unchanged original project hashes passed (`check-cold.mjs`, `cold-receipt.json`). No new prompt/start was issued.

An early final-run sequence used eight Tabs and opened the narrow sidebar; it is not counted as successful card navigation. A later packaged check established that the read-only document adds no Tab stop: seven Tabs from Refresh reach the card, while eight wrap to the navigation toggle. The later explicit detail entry and known-focus sequences above supply the final navigation evidence. An initial Rust attempt mutably rendered collaboration through the borrowed root chain and failed E0500; the final immutable helper and disjoint field flag consumption compiled and were used in the final native run.

## Checks and limits

Passed final macOS `cargo +1.98.0 build --locked --manifest-path apps/desktop/Cargo.toml`, LLVM PATH Windows `cargo +1.98.0 check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`, cold assertions and `git diff --check`. Pure UI focus/layout change: no new fragile UI tests, deleted tests, runtime/protocol changes or unrelated full suite. Existing real persistence/history backed the native actions.

This does not prove full VoiceOver narration, IME, other theme/font/language combinations, many-message/many-member pagination, Windows native or a newly packaged build. Full product goal remains active; the existing candidate and current ZIP are unchanged. No new large bundle was copied and old chat workdirs/evidence were preserved.
