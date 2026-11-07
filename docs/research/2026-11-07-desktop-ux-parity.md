# Hive Desktop 日常界面对标线索

日期：2026-11-07。只读检查 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的公开前端源码，以及 Hive Desktop 当前 GPUI 实现。以下是可验证的实现线索和待验收建议，不是同条件竞品实测，也不表示 Cindy 其他界面均已检查。Maka 固定快照本轮未取得对应前端证据，因此不将具体设计归因于 Maka。

## 真实问题三问

1. 使用多个项目和会话时，用户要在侧栏穿过常驻的添加项目表单；长对话中还要区分连续工具调用和最终回答；设置分散在滚动页和会话输入区，定位需要扫读。
2. 调整后，项目和会话更容易定位；工具摘要与重要回答有清晰层次；设置有可直接到达的分组。用真实窗口中的项目、长对话和窄窗口分别检查。
3. 不做则这些日常动作仍可完成，但随项目、会话和工具调用增加，查找与滚动成本持续上升。

## 三个候选

1. **项目与会话导航**。[Cindy ProjectNode](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/features/cc-agent/sidebar/sections/ProjectNode.tsx#L180-L225) 将项目头与受控折叠的会话列表组合，并在项目头提供项目内新建会话的回调；[ProjectsSection](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/features/cc-agent/sidebar/sections/ProjectsSection.tsx#L933-L947) 传入项目自身的会话与活动状态。Hive 的 `apps/desktop/src/navigation.rs` 当前常驻路径输入与添加按钮。候选：把添加项目表单收进已有交互入口，保留项目、会话及必要操作，不改变工作区语义。验收：常规窗口和 760×583 下，以 5 个项目、10 个会话检查切换、新建和同名路径辨识；会话列表不必先滚过添加表单。

2. **聊天工具信息层级**。[Cindy ToolCallCard](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/components/chat/ToolCallCard.tsx#L60-L101) 为部分工具提取一行摘要且默认折叠，[同文件](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/components/chat/ToolCallCard.tsx#L137-L173) 保留可访问的展开按钮与详情。Hive 的 `apps/desktop/src/view.rs` 已折叠工具，但工具、普通消息和复制动作仍占相近卡片空间。候选：压缩工具头与次要操作，强化用户消息、最终回答和待回应交互的层次；不隐藏失败或改变来源语义。验收：含十个工具、失败和待回应的真实窗口中可定位状态，展开、复制、键盘操作仍可用。

3. **设置定位**。[Cindy SettingsView](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/components/settings/SettingsView.tsx#L370-L395) 使用设置侧栏导航与独立内容区；[同文件](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/src/renderer/components/settings/SettingsView.tsx#L410-L425) 按选定分区渲染。Hive 的 `apps/desktop/src/navigation.rs` 和 `view.rs` 将自动组队、通知与外部程序混在通用页，模型切换在会话输入区。候选：使用已有组件提供团队、通知、外部程序等清晰分组，并解释会话模型与原生配置的关系；不新增配置能力。验收：窄窗下可找到并保存各组，切换页面保留未保存草稿和处理中状态。
