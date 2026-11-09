# Native file picker, draft handoff and actual Terra file read — 2026-11-10

Source baseline `4ceb096` (product binary includes 03d3403). No product/test/dependency code changed in this slice. Evidence `.artifacts/file-picker-return-4ceb096/`. Development native app, not a refreshed candidate or Windows runtime result. Full product goal remains active.

## Sibling picker checks

After #24, checked the existing attachment, executable and export-save routes before applying its fix elsewhere. All three tested cancellation routes already behaved correctly with their existing focus, so no blanket activation patch was added:

- Explicitly focused the project composer, opened Add files or images and mouse-cancelled the native picker. Immediately typed `_BEFORE_CANCEL`; it appended to the existing owned draft without another field click (`attachment-before.*`). An initial tool-action description anticipated missing input; the observed value disproved that hypothesis.
- On the actual completed conversation, opened export preview → Save Markdown → native mouse Cancel. Escape then closed the export preview (`export-cancel.*`, `export-escape.*`). No snapshot file was saved in this check.
- Settings → External programs: focused External Pi path, typed an owned unsaved value, opened native Browse and mouse-cancelled. Immediate `_CANCEL_OK` continued that same value. Cleared the temporary value; Save settings returned disabled. Independent runtime settings stayed all-null, Codex effective command remained the per-test environment override. No program settings saved, probe/CLI/global configuration changed by these native actions (`program-cancel.*`, `program-paths-restored.txt`).

This does not establish all selection/error routes or the image-original save picker. The earlier new-project owner-activation bug remains independently repaired; it was not generalized from shared source alone.

## Actual native attachment → main conversation → tool → reply

Owned copy of the prior first-project state. While stopped, copied its tiny project and redirected only the fixture workspace path to that owned copy (`project-copy.json`). This is fixture preparation, not a product relocation test. Only the original installed Codex path `/Users/tsk/.npm-global/bin/codex` was supplied through this test process environment. Pi directory isolated, no credentials copied, no Magpie or global default edits.

1. Created owned 49-byte `sample.txt` with two plain lines. Native Add files picker → explicit Go-to field click/type → observed full exact file path → native selection. The real project draft displayed the file preview/removal controls and retained the complete request (`attachment-draft.*`).
2. Selected Codex through the ordinary native Choose Agent menu. The same request and attachment moved into the new conversation. Before any send, the native default was Sol; explicitly changed only this conversation to **GPT-6.1 Terra / Medium**, and captured the pre-send readout (`attachment-before-send.*`). No inference used the initial default.
3. Clicked native Send once. Conversation bound to the actual workspace orchestrator on first send, as verified both by native Orchestrator label and persisted agentId. No shell/driver injected a model prompt.
4. Actual Codex/Terra commandExecution read the uploaded copy under this fixture's `state/hive/uploads/<workspace>/<attachment>.txt` using `head -n 2`. Native expanded tool view showed the actual path and two-line output (`attachment-tool.*`). Exit code 0; command output was exactly the 49 source bytes. This is not a filename-only or mocked file test.
5. Actual raw final was exactly three lines: `ATTACHMENT_PICKER_OK`, `Owned attachment picker check.`, `No model request.` The last line is literal file data, not a claim that this test avoided inference. Markdown displayed the lines as a paragraph; exact raw final checked independently. Exactly one native user input, one actual read command and one model final.
6. Typed `AFTER_ATTACHMENT_UNSENT` after the reply without sending. Cmd+Q exited 0. New native process restored the same native thread, historical attachment, command/final and exact unsent draft (`cold-history.*`). Opened historical attachment Preview: actual read-only uploaded copy showed the original 49 bytes (`cold-attachment-preview.*`).
7. Temporarily replaced only this owned source sample with `SOURCE_CHANGED_AFTER_UPLOAD`, clicked the preview's Reload, and observed it still showed the original uploaded copy (`independent-upload-preview.*`). Restored the original source bytes. Upload copy, saved original and restored source matched by full bytes/SHA256. Cmd+Q exited 0.

## Independent state verification

`check-cold.mjs` opens the actual saved native thread without prompting/agent-start. Passed:

- One Codex session, same native thread ID, bound `<workspace>:orchestrator`; model openai/gpt-6.1-terra, native reasoning medium, pending 0/busy false.
- One user delivery with one real 49-byte sample attachment; raw prompt includes its actual uploaded path. One native read command, exit 0, exact original output and actual file bytes. One exact three-line model final.
- Workspace draft removed after transfer; session draft exactly AFTER_ATTACHMENT_UNSENT, failed list and pending attachments empty. Historical attachment metadata and copy retained.
- Original Codex version probe 0.167.0, no saved Pi/Codex/Claude executable overrides, isolated Pi auth empty or absent. No model requests in cold checks.
- All three native processes exited normally: initial cancellation check, live native model session, cold GUI. Complete full AX captures used for cold marker assertions. Receipt `receipt.json`; inspection `cold-snapshot.json` preserves original backend evidence.

No source change meant no redundant rebuild/full suite; relevant actual UI/model/backend/file checks above ran. Existing macOS build/Windows target checks retain their source scope. Candidate remains 34aebbe; current unchanged. New native evidence does not claim package inclusion, image-original save/selection errors, every backend/file type/theme/IME/VoiceOver case, Windows native or full T3/Beluga/Cindy parity. Old workdirs/states preserved, no new large bundle copied.
