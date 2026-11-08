# 聊天发送快捷键

日期：2026-11-08。完整桌面目标继续实施。

## 真实问题

1. 用户在聊天输入框写多段需求时，固定 Enter 发送容易把尚未写完的内容交给 Agent；此前也无法选择更习惯的 Cmd/Ctrl+Enter。
2. 设置新增“聊天 → 发送消息”：默认 Enter 发送、Shift+Enter 换行；可改为 macOS ⌘Enter / Windows Ctrl+Enter 发送、Enter 换行。修改后立即生效，重开保留，发送按钮提供对应提示。
3. 不做会迫使所有用户适应同一种多行编辑方式，增加误发和补充消息。

参考 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 [DESIGN §14.3](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/docs/design-rules/DESIGN.md#143-keyboard--ime-send-type-text-fields)。本项对齐发送方式和输入法保护，不宣称已实现该规范中的列表续行或其他编辑能力。

## 实现

- 复用当前桌面偏好的本地原子文件写入方式，在后台返回的既有 dataDir 中保存 `desktop-composer.json`。不改 Pi 或外部 Agent 全局配置，不添加协作账本。
- 写入成功后才更新内存中的生效值。写入失败显示错误，保留原快捷键；缺失文件默认 Enter，损坏文件明确报错且不自动覆盖。
- 后台重连不会覆盖本窗口已经加载的偏好。设置对四个后端共用的输入框生效，传输及业务语义不变。
- 修改键 Enter 在聊天输入框范围内捕获现有组件 Enter action，先于原生文本组件插入换行；因此没有发送时多一行、发送后留下换行的问题。其他文本框保持自己的行为。
- 所有键盘发送共用输入框的实际 marked-text 检查，有输入法组合文字时不发送。点击发送按钮沿用原行为。Shift+Enter 继续交给原生输入组件处理。

## 验证与边界

原生 GPUI 夹具使用真实 Node、Hive HTTP / SQLite、Pi SDK 与本地 Responses SSE 成功对端，不调用厂商模型。每种外观分两次应用启动：

- 默认模式、设置实际点击、保存目标为目录时写入失败及原模式保留，修复后成功保存。
- 修改键模式 Enter/Shift+Enter 插入精确换行、空消息不发送、⌘Enter 发送原正文且不多换行。
- 退出重开保留修改键模式，切回 Enter 后实际 Enter 发送、Shift+Enter 换行。
- 两种模式下注入原生 marked text，确认候选文字尚未提交时按发送键不会产生用户投递；解除组合态后正常发送。
- 设置的外部程序路径输入框按 Enter / ⌘Enter，不发送聊天、不改变其输入。

夹具走查涵盖浅色、深色、760px 窗口，使用 GPUI 注入按键/鼠标事件与真实窗口渲染，不等同物理键盘或第三方中文输入法真机验收。Windows 仅默认桌面特性 MSVC 编译检查，Ctrl+Enter 原生交互未验收。没有在真实用户数据中改偏好或登录信息。

磁盘偏好单元检查覆盖缺省、两种模式替换与重开、损坏配置不覆盖、真实持久化失败和临时文件清理。代码位于 `apps/desktop/src/composer.rs`。本轮没有修改或删除现有测试；没有跑全量 Node suite，后端传输未变。

证据：`.artifacts/composer-preferences/` 下的三份原生收据、设置/多行输入/发送截图与夹具驱动。原生进程、SSE 对端和测试数据均独立；成功响应为合成内容，不作为真实模型验收。

实际命令：`cargo test --locked --manifest-path apps/desktop/Cargo.toml`（19 通过、1 个专用夹具忽略）；`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`。Mac 生产构建与冻结依赖部署后生成 `.artifacts/hive-desktop-macos-arm64-composer.zip`，解压后的真实 Node / PTY / Hive HTTP / SQLite 烟测及原生窗口导航恢复通过。包未签名、公证，不是正式安装发行。
