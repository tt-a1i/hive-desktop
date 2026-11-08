# 模型配置表单草稿

任务：[#4](https://github.com/tt-a1i/hive-desktop/issues/4)，完整产品目标继续实施。

## 实现

“模型与接口”的当前编辑表单复用现有 450ms 延迟保存与退出 flush，持久化到桌面数据目录中的 `desktop-model-drafts.json`。字段、接口类型、思考/图片开关及尚未填写完整的数字均按表单文本保存。未修改表单不新增草稿。

草稿按实际 models.json 路径隔离，保留原始 revision、原始字段值和当前编辑值。重新读取配置后自动恢复；配置已有外部修改时明确提示，保存继续经过原有后端 revision 冲突检查。不会自动套用新基线覆盖配置。实际配置路径改变时拒绝提交旧表单。

正常保存与明确放弃清除对应草稿，保留其他路径的草稿。读取、解析、临时写入及原子替换失败均返回错误；丢弃失败不替换当前表单。实际配置已经保存但草稿清除失败时明确说明两者状态。使用现有 tempfile，不增加依赖；不涉及 API Key 字段、登录交互或全局模型默认。

## 验证

- `cargo check --manifest-path apps/desktop/Cargo.toml` 通过。
- `CARGO_INCREMENTAL=0 cargo test --manifest-path apps/desktop/Cargo.toml settings::pi_models::draft::tests -- --nocapture`：3 项通过。实际文件往返保留原基线/未完成数字/能力与多路径草稿；损坏文件不会被改写；macOS 真实只读目录导致保存和丢弃失败，旧文件字节不变且可重新读取。
- 原生 GPUI 验证使用当前源码的隔离副本、真实 runtime 和独立 Pi models.json，无模型推理。窗口填写中文显示名称、接口地址和首个模型，点击 Responses，等待真实自动保存并正常退出；完整新进程恢复字段与接口类型。修改配置原文后完整重启显示外部修改提示，点击保存触发真实后端冲突，表单和草稿仍保留；明确放弃后新进程不再恢复。
- 同一原生检查另验证成功保存实际写入 models.json、清除当前草稿，完整重启不再恢复。收据、日志与恢复/冲突截图：`.artifacts/pi-model-drafts/`。
- 验证初始使用 `{}`，实际配置读取按现有约束拒绝缺失 providers，未达到模型表单检查；改为独立有效配置后重跑。未为此修改生产校验。
- `cargo test --manifest-path apps/desktop/Cargo.toml drafts::tests -- --nocapture`：原有聊天、任务、附件草稿 3 项回归通过。
- 最后 `CARGO_INCREMENTAL=0 cargo build --manifest-path apps/desktop/Cargo.toml` 通过，重新构建生产源码，替换根目录构建输出中的验证专用二进制。

## 边界

原生窗口检查通过 GPUI 输入状态和真实按钮事件执行，未将其当作系统输入法/物理键盘验收。Windows 原生交互、权限语义和开发 ZIP 尚未验证；本轮不刷新开发 ZIP。依用户要求不再等待或处理 GitHub CI 账单限制。未运行全量 runtime 测试：本次仅改桌面配置表单与本地草稿，不改协作协议、模型执行或 runtime 配置服务。其他配置表单草稿不计入本任务。
