# 图片系统选择、另存为与可读取反馈

2026-11-10。针对图片入口此前未覆盖的真实Mac系统对话框，使用已安装db6fb20开发应用和隔离项目完成实际交互，并修复原图查看器的辅助技术正文缺失。证据 `.artifacts/image-save-native/`。

## 操作与结果

- 新建独立无主控工作区，Pi目录为空且PI_OFFLINE=1，不选模型、不启动Agent。旧真实图片项目的源路径已不存在，首次准备复制明确ENOENT；从其已保留真实Codex历史中的base64恢复原PNG，核验原SHA256，不修改旧状态或虚构旧路径还存在。
- 通过安装应用“添加文件或图片”打开真实系统Open面板，输入自有PNG完整路径，确认中文及空格原名；上传后草稿出现“左右色块 原图.png”，点击进入320×180原图预览。
- “另存为…”实际打开系统Save面板，默认项目目录及完整原文件名；Cancel返回预览，没有“已保存”成功提示。
- 选择独立exports目录，以 `保存副本 🐝.png` 保存，实际屏幕出现“已保存原图”。磁盘文件与原图1,151字节完全一致，SHA256 `0834812fa26e2a6c3b7b18d7725d03c4a1695721cfdcc9ac6e0892048eb48ed5`。
- 将本轮自有导出副本替换为明显的测试sentinel；再次同名保存，系统真实出现Replace/Cancel覆盖提示。取消后sentinel完整保留；再次保存并确认Replace后，文件逐字节恢复原PNG。没有覆盖用户既有文件或原图。
- 安装应用⌘Q实际exit0，运行句柄69606终结。

系统对话框AX、取消、覆盖提示及实际保存截图分别为save-default.ax.txt、cancel.ax.txt、overwrite-confirm.ax.txt、saved.png。图片在草稿中，无聊天或模型输入。

## 发现并修复

保存成功和尺寸在屏幕上可见，但旧原图查看器的原生辅助技术树仅含三个按钮，没有这两段正文。复用项目已有div角色/标签模式，为尺寸、加载、成功提示及错误说明补充稳定ID、Label和实际可见文本；没有新增抽象、依赖或改变保存/覆盖逻辑。

最终源码Mac构建后，独立开发窗口冷恢复同一草稿，AX实际读到 `320 × 180`。再经系统Save保存 `可访问保存.png`，AX实际读到 `已保存原图`；修复前同条件缺失的文本不再缺失。证据fixed-dimensions.ax.txt、fixed-saved.ax.txt。

约593×507逻辑尺寸窄窗，完整工具栏与保存反馈可见；点击原尺寸，Retina截图实际320×180物理像素蓝粉图，按钮变“适应窗口”；Escape关闭预览，草稿保留。截图narrow-saved.png、narrow-original.png。最终窗口⌘Q/句柄96931 exit0。

## 冷读与检查

- 独立新runtime经真实Hive HTTP/SQLite打开同一隔离数据，重新读取实际上传附件，ID/原中文名/base64原字节均相同；两个系统保存文件与原图逐字节相同，草稿仍只有原上传引用、空正文。
- SQLite工作区1、workers/messages/agent_runs/dispatches/workflow_runs均0。无厂商模型请求、无新增聊天或投递。见check-cold.mjs、cold.log、cold-receipt.json。
- 最终 `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` 和Windows MSVC目标 `cargo check --locked … --target x86_64-pc-windows-msvc --bin hive-desktop` 通过；首构建发现&str无效clone警告，已移除，最终两构建无该警告。
- `git diff --check`通过。纯UI辅助技术语义修复，不增加脆弱UI测试，不运行无关Node全量suite，没有删改既有测试。

本轮证明列出的Mac系统选择/保存/取消/覆盖和冷读行为；不等同VoiceOver朗读全流程、加载瞬态/解码失败注入、所有语言主题字号、物理拖放、超大图片历史性能或Windows原生。安装包/Applications仍db6fb20，本轮读取语义修复尚未打包；不为单个小改复制完整生产依赖。完整目标保持进行中。
