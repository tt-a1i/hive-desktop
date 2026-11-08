# 聊天模型入口的长名称与信息层级

## 问题与参考

用户配置较长的提供商 ID 后，在 760px 桌面窗口里输入区的模型按钮按完整“提供商 / 模型 ID”扩展，把推理强度、用量和发送按钮推到窗口外；模型按钮本身也无法正常点击打开。原生失败截图与日志保存在 `.artifacts/composer-models/baseline-light-small-*`。这是实际布局故障，不是仅凭源码推测。

参考 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 `apps/desktop/src/renderer/components/new-chat/ModelSelector.tsx:4680` 附近：模型入口 min-width 与 max-width 收缩限制、文本截断及完整 title，把发送等固定操作留在可见范围。只借鉴呈现方式，不复制其模型路由或权限系统。

## 变化

- 模型入口优先使用当前目录中对应 provider/id 的名称；没有目录记录或名称为空时保留原始 ID。
- 常规/窄窗口宽度限制为 260/180px，使用组件原生省略号和下拉指示；完整名称、provider 和 ID 保留在 tooltip。
- 搜索列表分两行显示名称与 provider/id，完整名称与身份作为可访问标签，tooltip 保留全文。
- 当前模型排在匹配结果首位，其他目录顺序保持原样；搜索继续匹配名称、provider 与 ID。选择仍将原始 provider/id 传给原有会话配置逻辑，不改写用户默认模型。

## 验证范围

macOS 真实 GPUI 窗口使用隔离 Pi 配置、真实 Hive HTTP/SQLite 和原生会话。配置含 170 字符级 provider 与很长的中文模型名称：

- 基线窄窗复现按钮溢出和入口打开超时；保留失败现场。
- 修改后浅色窄窗、深色窄窗和常规窗口验证打开列表、长名称截断、搜索 provider、实际切换到同 ID 的另一个提供商；原中文草稿逐字保留。
- 实际点击原生发送按钮，确认原中文正文进入真实会话投递链路，证明发送按钮仍可使用。模型只配置 `gpt-6.1-terra`，请求目标为本地不可达回环地址；没有厂商推理成功声明。
- 截图与回执在 `.artifacts/composer-models/`，使用 GPUI 原生事件注入，不代表物理键鼠、VoiceOver 或 Windows GUI 验收。

本轮仅原生 UI 调整，不新增 UI 单测，不修改运行时/协作协议。验证使用完整桌面编译与 Windows MSVC 默认功能编译检查、原生窗口实际交互；不重复无关运行时全量测试。完整 Cindy 体验、窗口尺寸边界和 Windows 实际交互继续推进。
