# Codex 问题附件的桌面验收

2026-11-19 在 macOS 用隔离状态目录和本地临时签名开发包运行。真实流程采用 `ac42b63` 的桌面和包内后台，Codex CLI 为 `0.169.0`。后续包只替换 Rust 桌面二进制并重新验证签名。推理只在测试会话中使用 `openai/gpt-6.1-terra`，包内观察器核对了模型及回合数量。

## 真实问答

1. 从桌面输入框发送一条请求。Codex app-server 发出一个非保密的 `item/tool/requestUserInput`，桌面显示问题卡。
2. 在问题卡打开 macOS 系统文件选择器，一次选中测试文本和 PNG。确认草稿收据指向 Hive 私有上传副本，逐字节比较副本和原件。提交前把两份测试原件移到另一个测试目录，核对原路径已无法访问。
3. 选择 Verify attachments 并提交。原生请求收到一次回复，答案包含两个私有路径。Terra 在同一回合执行只读命令并返回文本标记及两份文件的 SHA-256，和本地计算结果一致。协议中只有一次 `turn/start`，没有 `turn/steer`。
4. 关闭后冷启动，核对原模型回复和只读附件收据。观察器确认冷启动没有新回合或附件回调。桌面以 0 退出，后台确认 `shutdown`，运行时和 Codex 子进程均退出。

## 辅助功能和移除

重建包的 macOS 辅助功能树可读取问题正文、各回答选项、其他回答及带文件名的移除按钮。用受控本地 Codex JSONL 子进程在真实窗口中选入并移除测试文件，核对已保存草稿及其副本。可编辑问题卡已显示的草稿省去重复的只读收据。移除最后一个文件后，后台仍保存空草稿，界面略过这份无文件的收据。已有只读交付收据会提供状态和文件名，即使原问题卡已经关闭。

专项 `pnpm --filter @hive-desktop/runtime check:interaction-attachments` 通过。Rust 测试 115 项通过，1 项按设计忽略，Windows MSVC 交叉编译通过。

此次附件引用通过 Codex 答案字符串报告私有路径，PNG 验证基于实际文件字节。适用范围为 Codex 非保密问题。Windows 原生选择器与回合仍需验证。测试包仅保存在本地，没有替换已安装实例或发布。

本地证据位于 `.artifacts/codex-question-window-rn01kG/`。真实回合见 `receipt.json`、`live.native.jsonl` 和 `saved-private-copies.json`。冷启动和子进程见 `cold.native.jsonl`、`accessibility.native.jsonl` 和 `accessibility.processes.json`。辅助功能、移除和冷启动复核见 `accessibility-receipt.json` 和 `ax-controls-cold.desktop.log`。重建包摘要见 `final-accessibility-package.sha256`。

## 多问题草稿、切换会话与收据损坏

GPT-6.2 Sol 独立核对了原生窗口和后台。旧包从文件选择器为两道问题各选择六份文件，十二份先经过上传，第二次保存才被后台拒绝。新版按整张问题卡核对十份上限，超额时在上传前提示。选择器、导入和保存期间限制同卡附件操作。原问题仍待回复时，导入和保存可按原问题身份完成。部分文件导入失败的提示独立于草稿保存状态。

运行时使用原交互的收据核对无附件的回复请求。用真实 Hive HTTP、SQLite 和两个独立的受控 Codex 子进程验证，一份损坏收据不会阻断另一工作区的普通问答或审批。当前问题的损坏收据和结果不明的准备记录都阻止再次交付，两种回复请求共计零次额外交付。历史收据列表仍会报告损坏。桌面只保留当前会话的收据缓存，切换会话时释放上一份，并丢弃已过期的读取结果。

macOS 从新版 ZIP 解包后，用系统选择器为第一题选入六份文件。测试转发器在草稿已落盘后暂缓成功回执，原生窗口切换到另一工作区后才放回回执。返回时可继续回复，无需重试保存。为第二题选六份文件时，窗口提示超额，日志中没有新增上传请求。改选一份有效文件和一张损坏图片后，有效文件落盘，失败提示仍可见。移走原文件再提交，两题的原生答案分别带有各自的私有副本路径，只发送一次。冷启动显示收据，无交付重放。再次切换工作区时，另一会话不显示该收据，返回后重新读取。正常退出和冷启动后的退出均收到后台确认，系统进程表中没有本次运行的桌面、后台或 Codex 子进程残留。

源码为 `ec0624c`。127 项 Rust 测试、Windows x86_64 MSVC 交叉检查、附件与 Codex 交互专项以及包内 Node、PTY、Hive HTTP/SQLite 和退出冒烟均通过。macOS ZIP 签名和归档内容核对通过。ZIP SHA-256 为 `c907ed1e5152cc1ef188efde3ea341f75fb105813bbfea3eb65a925179d1baef`。本轮使用受控 Codex 子进程，未调用模型或发布。Windows 原生选择器和真实回合仍待验收。

本地证据位于 `.artifacts/question-delivery-9Pm8No/`，含 `runtime-receipt.json`、旧窗口 `baseline-LpMgCW/receipt.json`、新版 `candidate-FvFHyt/receipt.json`、原生辅助功能记录及归档核对清单。
