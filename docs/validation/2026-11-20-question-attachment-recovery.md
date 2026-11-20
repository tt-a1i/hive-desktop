# Codex 问答附件恢复提示

附件副本校验失败时，待答卡会标出对应的附件，保留已输入的回答，并提示移除标记的附件，需要时重新选择。保存附件草稿与提交答案共用此逻辑。可确认尚未交付时，用户可以编辑附件。原生交付状态不明时，卡片继续阻止重复提交。

后台只为本地附件读取和校验错误发出专用错误码与上传 ID。桌面按 ID 匹配当前卡片上的附件，两个语言版本使用固定文案。交互请求的错误原文也从桌面的 RPC 诊断日志中省略。

## 验证结果

- 真实 Hive HTTP、SQLite、私有上传库和受控 Codex 对端通过专项检查。生产 JSONL 验证了同名上传的精确定位，同长度覆盖后的草稿保存失败，以及副本删除后的提交失败。错误帧只包含固定文案和受影响的上传 ID。两次失败均交付零次，草稿收据保留。
- 人工设置当前收据为已准备交付后，保存、提交与状态查询继续阻止重试。复原测试收据后，通过移除并重选恢复。对端收到一次答复，两题使用各自的副本路径。测试用秘密只进入原生答复，回执、公开事件与后台诊断未带出该值。
- 轻量 Rust 进程监督测试覆盖传输转换。带有错误码字样、路径和秘密的错误文案无法触发附件提示。该提示只接受后台回复中显式的错误码和上传 ID。

```sh
pnpm --filter @hive-desktop/runtime check:interaction-attachments
pnpm --filter @hive-desktop/runtime check:interaction-attachments-wire
cargo test --offline --locked --manifest-path apps/desktop/Cargo.toml \
  --no-default-features --features process-supervisor-check --bin process-supervisor-check
```

Mac 桌面源码使用已有依赖完成 Rust 类型检查。原生窗口与完整链接仍待验收。本轮未调用模型。

## Mac 原生复验

从源码工作树运行以下命令。将 `HIVE_QA_APP` 设为带本次改动的原生候选二进制绝对路径。运行时和 Codex 都用本地夹具，桌面数据和代理配置单独存放。

```sh
pnpm --filter @hive-desktop/runtime build
qa_dir="$(mktemp -d /tmp/hive-question-recovery-mac.XXXXXX)"
mkdir -p "$qa_dir"/{project,files,state,pi,codex}
printf 'original\n' > "$qa_dir/files/first.txt"
printf 'second\n' > "$qa_dir/files/second.txt"
printf '#!/bin/sh\nexec "%s" "%s" "$@"\n' "$(command -v node)" \
  "$PWD/packages/runtime/checks/fixtures/codex-question-attachment-peer.cjs" > "$qa_dir/codex-fixture"
chmod 700 "$qa_dir/codex-fixture"
printf '%s\n' "$qa_dir"
HIVE_DESKTOP_DATA_DIR="$qa_dir/state" HIVE_DESKTOP_RUNTIME="$PWD/packages/runtime/dist/main.js" \
  HIVE_DESKTOP_NODE="$(command -v node)" HIVE_CODEX_COMMAND="$qa_dir/codex-fixture" \
  CODEX_HOME="$qa_dir/codex" PI_CODING_AGENT_DIR="$qa_dir/pi" PI_OFFLINE=1 \
  HIVE_DESKTOP_TRACE_RPC=1 "$HIVE_QA_APP" 2>"$qa_dir/desktop.log"
```

1. 在原生应用打开测试目录下的 `project`，建立 Codex 聊天并发送普通文本。夹具显示 Public、Secret 和 Extra 三道题。填入本地测试回答，Secret 使用虚构值。
2. 在 Public 题选择 `files/first.txt`。等待草稿成功后，从 `state/chats/interaction-attachments` 中的当前草稿 JSON 读取该附件的 `filePath`。只改动测试副本，将其写为同长度不同内容。为 Extra 题选择 `files/second.txt`，触发新的草稿保存。核对标记只出现在 Public 题的 `first.txt`，保留回答，且仍可移除附件。此时 `project/responses.jsonl` 尚未生成。
3. 移除标记的附件并重选原文件。等待保存。用新草稿中的路径暂移新生成的 Public 附件副本，然后点击提交。核对提交按钮暂停，仍可编辑附件，标记只出现在 Public 题，卡片未显示副本路径或秘密。此时仍无对端答复。
4. 移除标记附件并再次选择原文件，等保存成功后提交。确认 `project/responses.jsonl` 有且仅有一行，包含两份正确副本路径。创建 `project/finish`，让夹具确认原生回复。该文件夹内的答复日志会包含测试用秘密，仅在本机检查。
5. 分别在中文和英文运行，使用两个独立测试目录。核对卡片的恢复提示与标记，保存时不出现通用的“重试保存附件”。核对桌面诊断日志无需显示错误正文，关闭窗口后检查夹具子进程和桌面后台已退出。


macOS 最终签名包已通过系统文件选择器及中英原生窗口验收。同长度损坏发生在草稿保存时，副本丢失发生在提交时；各场景的 AX 树均只有受损文件名带“附件副本不可用”或“Attachment copy unavailable”，健康文件没有标记。失败阶段没有交付，移除重选后每种语言各交付一次，两题选择和掩码回答保留。秘密未见于 AX 或桌面日志，两轮退出码均为 0，自有进程均已退出。窗口截图不可用。
