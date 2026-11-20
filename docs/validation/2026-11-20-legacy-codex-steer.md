# Explicit resolution of legacy Codex queued messages

The candidate is based on `3c199b4` after #58, #59 and #60. The changes run in the local runtime and the native desktop panel.

## Behavior

Older Codex spools used `pending` and `steerUser:true` for messages that were never sent, had an unresolved request, or were acknowledged before the process disconnected. The runtime holds these messages before draining the queue. It preserves each delivery ID, original message and attachments. A saved completion with the exact delivery ID clears the wait. Matching text in native history is insufficient evidence of completion.

New Codex messages record `steerQueueVersion:1` when accepted for a possible append. The runtime can still edit or continue new messages when it knows they were never attempted or were definitively declined. Pi, Claude, and ACP keep their existing queue behavior.

The desktop panel lets the user discard the local wait or approve a resend that may repeat native work. Both choices have a Cancel button. After approval, select **Resume processing** to send the original message and attachments. Starting a Hive run leaves an approved retry held. The approval uses the reviewed profile revision, original native session and exact delivery ID. It also checks the complete original delivery and, where available, the Hive source message. Only the local wait changes when discarded. The original source and audit stay on record.

The runtime saves decisions before changing the spool. A repeated approval returns the original receipt and grants no further attempt. If the spool write failed, reopening applies the decision only when the saved session and original delivery still match. If a retry was interrupted by a stop or a crash, the runtime holds the same item for another decision. Subsequent decisions keep their own receipts.

## Verification

- TypeScript check and runtime build passed on macOS. Existing delivery queue, Codex and Claude queue peer, compiled queue wire and handoff queue cold-retraction checks passed.
- `checks/codex-legacy-steer.ts` used real Hive HTTP and SQLite with the production Codex JSONL adapter and a controlled peer. A cold review opened no native process. The fixture checked old pending shapes, exact delivery receipts, a native text lookalike, preserved transport text and attachment references, stale and interrupted decisions, Hive source cancellation, explicit restart and stop. An approved resend produced one native request after **Resume processing** and settled the original delivery ID. The Hive source stayed unchanged for the resend.
- Before the corrections, three targeted runs reproduced an extra `turn/start` during Hive ready, disappearance of a stopped retry, and silent acceptance of a changed spool after a receipt had been written. A separate run confirmed stale failure state hid the desktop resume action after approval. The corrected candidate passes the same assertions.
- The first candidate passed a macOS `cargo check`. The final native labels were checked against the composer label in both desktop languages and parsed with rustfmt. Native window, Windows and real model acceptance remain open. No account or model call was used.

The new regression runs through `check:queued-inputs-peers`.


macOS 签名候选的原生窗口验收使用真实 Hive、SQLite 和离线 Codex 对端。取消“可能重复”的重发确认后，原队列文件摘要保持，未写入决定，也未开启新回合。确认批准后仍未发送；正常退出并冷开仍能核对决定。用户点击“继续处理”后，同一原生线程仅新增一次原文及原附件，按原投递 ID 完成。独立草稿和原 Hive 输入保持；两次桌面正常退出，两个后台子进程均已退出。窗口截图工具未能捕获图像，证据为七份按 PID 获取的 AX 树、协议日志、数据库与文件核验。
