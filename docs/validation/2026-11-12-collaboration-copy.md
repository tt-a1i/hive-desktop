# 协作详情复制原文

2026-11-12，基于9399404。现有派单详情有复制编号/任务，却没有复制汇报、取消原因及协作消息的入口；用户查看含换行/代码块的较长原文时，无法直接取出完整文本。现复用同页按钮与GPUI ClipboardItem，增加汇报/取消原因复制及每条消息的复制，直接使用后台原字段。中英文可访问名称按内容区分，键盘聚焦沿用Reveal滚动显露。无新API、store、权限或依赖。

## 验证

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`及`git diff --check`通过。纯UI改动，无正式测试增改删，未跑无关全量suite。

`.artifacts/collaboration-copy-20261112/seed.mjs`通过真实runtime创建独立工作区、停止成员及空会话；仅在自有SQLite准备两个终态派单和一条progress消息，随后真实HTTP逐字段回读。它们是UI夹具，不是模型真实执行或汇报。

一次Mac原生窗口分别点击复制汇报、协作消息和取消原因，粘贴进自有空会话的未发送草稿。完整AX可见对应入口，正常Cmd+Q exit0。`check.py`读取保存后的草稿和真实SQLite，断言：

- 汇报代码块、Unicode、换行和尾部空格精确保留；协作消息逐字存在，移除该单次精确片段可恢复精确原汇报。
- 取消原因重复复制后，退出保存的草稿与原字段逐字相同。
- 两派单和一消息保持原值，messages=0、agent_runs=0；没有发送消息或模型请求。

消息粘贴时指针聚焦使插入点位于汇报中部，测试未假装其在末尾拼接；改为核对精确插入片段及移除后原文。一次完整AX重读导致旧索引失效，点击被拒绝后按新索引继续。取消原因首次粘贴前系统剪贴板已被其他活动改变，未发送内容已在自有草稿清空，随即重复复制/粘贴验证成功；不保留该无关剪贴板正文。收据仅记录这一限制，不将此尝试当产品复制失败或成功证据。

完整脚本、原生AX、最终草稿、HTTP及构建日志与receipt保存在上述目录。英文标签本轮仅静态检查，Windows原生、所有消息种类和长历史性能未扩张为验收。当前已安装3ed07c9包未包含本次源码修复，完整产品目标继续。
