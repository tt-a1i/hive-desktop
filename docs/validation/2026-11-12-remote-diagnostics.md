# 复用Web已有的诊断复制入口

2026-11-12，基于a9813a7。补远程设置中的“复制诊断信息”，直接读取Hive 2.2.1已有的authenticated support-bundle端点，格式化JSON后复制到本机剪贴板；没有新增后台接口、诊断store或远程授权。

## 真实问题三问

1. 用户在桌面排查远程连接/配对问题、准备反馈运行状态时，现有Web的RemoteAccessSection已有复制诊断入口，桌面相应设置却没有，只能另找日志。依据是固定本地Web源码`/Users/tsk/code/hive/web/src/remote/RemoteAccessSection.tsx`的copyDiagnostics及现有HTTP端点，而非假想的诊断系统需求。
2. 桌面远程设置现在可直接复制同一脱敏JSON，显示读取/成功/错误状态；设置搜索输入diagnostics或诊断可找到入口。观察标准是实际剪贴板JSON与authenticated API的内容一致，并保留metadata、排除正文和凭据。
3. 不做会留下Web协作/配置迁移缺口，并让桌面连接问题的反馈缺少已有的运行摘要。只补原功能入口，不扩展为日志上传或新的遥测。

## 实现

复用现有hive.request、connection_generation保护、remote消息提示、Reveal焦点显露和GPUI剪贴板API。一个仅用于当前窗口的loading标志防止重复点击；断线/关闭不开始复制，旧连接结果不修改新后台状态。请求失败不覆盖剪贴板，显示原始错误。成功内容不写入应用设置，也不会自动发送到任何服务。

说明文案明确包含项目/成员名称、版本和运行状态，排除消息正文、令牌和完整项目路径；中文/英文文本均已加入现有语言表。帮助说明有Label语义，结果提示复用现有Status语义，不新增面板。

## 代码验证为主

`.artifacts/remote-diagnostics-20261112/check.mjs`从本轮自有混合后端检查的已关闭状态复制数据，运行真实Hive/HTTP/SQLite及原support-bundle构建器：

- 未授权HTTP返回403；通过现有authenticated通路读取schema_version=1。
- 实际包含1项目、4成员、4条reported派单；名称、状态和任务/汇报长度均保留，不能以丢掉整个项目来伪造脱敏成功。
- 任务/汇报原文不出现；设置自有非真实token和custom gateway canary，输出均不含原值，custom gateway只保留布尔标志；完整项目路径不出现。
- remote保持disabled，没有打开网关、登录、配对、模型推理或发送通知。模型请求0。

通过 `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC目标 `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`（LLVM bin在PATH）及`git diff --check`。纯UI迁移，未增改删正式测试、未跑无关全量suite或重复后台构建。

## 一次原生复制抽查

源码独立App、独立data/agent，进入设置→远程→Copy diagnostics，实际出现成功Status。返回自有Pi草稿，使用原生Cmd+V粘贴当前剪贴板，没有用工具文本替代剪贴板，也没有发送消息；Cmd+Q退出0后从持久草稿读取完整JSON，与此前真实接口输出逐字段一致。唯一差异是顶层generated_at及action_center.generated_at两个实际生成时间，比较明确只排除这两项。原生结果保存在native-clipboard.json和receipt.json。

该抽查只走新复制链路；后台/脱敏矩阵用代码完成，不反复点面板。初次窗口恢复选中复制状态中的Claude会话，但没有复制其独立Claude目录，因此出现No conversation found；随后切到Pi做未发送草稿核对。该失败保留在native日志中，不视为Claude完整恢复验收或掩盖成全部界面无错误；不影响诊断数据/剪贴板比对。

## 边界

本次复用现有Hive脱敏规则，没有重写或扩展敏感字段策略；名称与运行元数据仍在输出中，用户可自行检查后分享。未验证VoiceOver实际播报、全部窄窗/键盘、复制失败后的系统剪贴板或Windows原生。源码尚未进入645c9a5开发包，完整跨平台产品目标继续开放。
