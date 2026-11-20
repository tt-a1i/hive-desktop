# 桌面操作体验持续对标

目标是让用户在实际项目里少绕路。每轮从三个参考产品中找出能复现的操作差距，先修容易误操作、丢失当前状态或反复多点的地方。记录同一操作在 Hive 的修复前后表现，以及实际验证的平台。任务见 [#57](https://github.com/tt-a1i/hive-desktop/issues/57)。

## 2026-11-21 第一批

Hive Desktop 从 `9555db1` 开始。Hive 上游 `main` 为 `18861b80379bff0d658623a3aa6f1454159d6925`，与已有依赖基线一致。上一轮验收见[记录](validation/2026-11-21-comprehensive-workflows.md)。

| 操作 | 当前改动 | 参考 |
| --- | --- | --- |
| 导入多个项目后进入指定项目 | 导入结果的每个成功项目都有打开入口。用户可以直接进入任一项目查看导入的会话。 | [T3 Code 欢迎页](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md) |
| 只清理一个项目的待办 | 跨项目收件箱可筛选到指定项目。项目身份、暂缓状态和派单跳转都沿用原来的精确记录。 | [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/86c473f9ecc9a4deeb8180bc21deb0416411d828/docs/product-rules/task-inbox.md#L20-L23) |
| 同屏阅读长文件与聊天 | 通过分隔线拖曳或方向键调整文件面板宽度，双击或按 Home 恢复。当前窗口关闭再打开面板会恢复选定宽度。 | [Beluga 面板宽度](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/RightPanel.tsx)、[分隔线操作](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/ResizeHandle.tsx) |
| 用键盘找同一会话里的旧消息 | 查找栏搜索尚未加载的历史。跳页时保留查询和结果，Enter 和 Shift+Enter 在结果之间移动。 | [T3 Code 会话内查找](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/keybindings.md) |

macOS 隔离窗口已经验证，两个项目结果中可直接打开第二个项目并查看它的原始会话。收件箱筛选到 Atlas 时只显示 Atlas 事项，返回全部项目后重现 Atlas 和 Beacon 两项目。文件面板通过键盘从 320 调到 344 像素，拖曳到 364 像素后关闭重开保持宽度，按 Home 回到 320 像素。收据在 `.artifacts/open-project-native`、`.artifacts/inbox-filter-native` 和 `.artifacts/dock-width-native`。

在 360 条消息的 Pi 会话里，用 Cmd+F 搜到两条未加载的消息。实窗用上、下结果按钮往返两个历史窗口，查询和值 `1/2`、`2/2` 保留。一次历史查询供两次导航复用，每次打开只返回 120 条消息，无 Agent 推理调用。GPUI 隔离测试中的 Enter 和 Shift+Enter 分别触发一次导航。实窗注入的 Enter 未触发，连续键盘原生复验待完成。运行时检查覆盖 Codex 已存、已导入及实时会话，含身份、取消请求、可搜索字段和结果上限。证据在 `.artifacts/competitive-ux/conversation-find-click` 和 `.artifacts/competitive-ux/conversation-find-headless`。

## 2026-11-21 第二批

- **更快看出谁在等你。** 收件箱把需要关注、等待 Agent 和已暂缓的事项分开。等待 Agent 默认收起，有明确的条数。需要处理的错误和问题仍直接显示。打开已暂缓后能看到其中的等待事项。 [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/86c473f9ecc9a4deeb8180bc21deb0416411d828/docs/product-rules/task-inbox.md#L34-L52)
- **返回工作树时恢复展开的目录。** 同一个项目路径的会话共享展开记录，独立工作树和导入会话的子目录按原路径各自记忆。切回时重新读取目录，失败后可以重试。树的展开状态在当前窗口保存，文件预览仍由当前工作树决定。 [Beluga 文件面板](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L148-L185)
- **从长项目列表选出想导入的几个。** 先显示最近五个，打开全部项目后按名称或路径搜索。筛选变化会保留被隐藏的勾选项，导入前显示总数以及当前看不见的数量。 [T3 Code 项目推荐](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md#L65-L83)

- macOS 收件箱实窗中，Beacon 的问题立即可见，Atlas 的等待汇报默认收起。展开后可继续暂缓 Atlas，查看暂缓时单独显示。切换到 Beacon 隐藏 Atlas 的暂缓项，点击问题打开准确派单。证据在 `.artifacts/inbox-sections-qa`。
- macOS 文件树实窗中，两个使用项目根目录的会话共享目录状态。A、B 两个独立工作树各自恢复展开的层级。收起 A 的上层目录后切走再回到 A，上层仍收起，主动展开后下层状态恢复。证据在 `.artifacts/tree-restore-qa`。
- macOS 欢迎页在七个项目中先显示最近五个。展开全部项目后，输入 Fjord 只显示匹配项目。对 Blue 和 Fjord 执行批量操作后，后台确认 Blue 的三段会话已经导入，Fjord 新增三段。证据在 `.artifacts/welcome-search-qa/receipt.md`。
- 176 项桌面 Rust 测试通过、1 项原有忽略。macOS 常规构建检查、Windows x64 MSVC 交叉类型检查、运行时类型检查和构建、历史搜索和项目文件专项回归均通过。Windows 原生交互未复验。

## 接下来优先检查

- **收件箱实时更新时保持当前行。** 在键盘选中或指针悬停时插入新的高优先级事项，观察当前操作是否被挪到另一行。打开派单再返回时也要能回到之前的项目范围和位置。 [Cindy 收件箱规则](https://github.com/makecindy/cindy/blob/86c473f9ecc9a4deeb8180bc21deb0416411d828/docs/product-rules/task-inbox.md#L53-L57)
- **从聊天或 Diff 在文件树定位。** 在两个工作树中放置同名文件，从消息或改动点打开，验证能展开对应目录并选中准确工作树里的路径。 [Beluga 文件面板](https://github.com/beluga-labs/beluga/blob/45a484b616a274a886e55bc28c1879719351a7c1/packages/mac/src/layout/panel/FilesTab.tsx#L186-L213)
- **首次导入中断后的继续操作。** 选择多个项目，模拟部分成功后退出或重新打开欢迎页，验证之前的勾选项是否可恢复。保存用户已勾选的原始项目路径，手动刷新后匹配当前的发现结果。也要让用户直接看到被筛选隐藏的导入结果。最后评估有就绪 Agent 但暂时无项目时的临时会话。 [T3 Code 推荐与恢复](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/welcome-page.md#L65-L117)
- **发下一条消息时按次切换队列行为。** 在已有运行中验证，用户能否临时选择排队或引导而无需更改默认设置。保留后端能力限制与现有键盘发送设置。 [T3 Code 输入快捷键](https://github.com/pingdotgg/t3code/blob/ccc1feb15572c422fbdce26ba6077b91459cfd47/docs/user/keybindings.md)
