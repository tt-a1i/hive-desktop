# 打开会话错误的归属

2026-11-12，基于885ab4f。上轮诊断复制抽查时，复制数据缺少独立Claude transcript，打开该会话失败；切到Pi后仍显示Claude原错误。该失败虽然由隔离夹具触发，却暴露真实的前端错误归属问题。

## 根因与修正

共享request已经用connection_generation与open_request拦截旧连接/旧打开请求的晚到失败，但已完成的chat.open失败被写入无归属的全局error。切换会话/工作区会清理Agent诊断和回复失败，却不区分并清理这类打开错误。

添加一个仅用于错误归属的ErrorNotice::Conversation包装，原文与展示方式不变。共享chat.open RPC失败，以及打开响应中的身份/消息数量/解析校验错误均归为此类。复用reset_reply_failures：切换会话、工作区或移除当前工作区时只清掉此类错误；普通后台、草稿保存、退出清理等全局错误继续保留。没有通过统一error=None掩盖全局问题，没有新store、协议字段或后端恢复分支。

检查过shared request、show_session、open_member_session和reset_workspace_view调用链。导航、成员/派单入口和跨工作区均经过同一清理入口；保留既有旧请求序号保护。

## 验证

现场`.artifacts/session-open-error-20261112`，独立数据、未连接账号，两个命名会话：一个Claude记录指向明确不存在的自有CLI路径，另一个健康Pi记录。后者只初始化SDK，不发送模型请求。

一次原生抽查：初始失败会话显示原因与Reopen conversation；点击Healthy Pi，正常查找/聊天控件出现，原错误和修复入口消失；点击Unavailable Claude，原错误及重开入口再次出现。三份完整AX分别保存为failed-first、healthy-after-failure、failed-on-return。一次ScreenCaptureKit捕捉错误后重新读取同一窗口成功，没有重启应用。Cmd+Q退出0，没有发送消息或调用模型。

通过：Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC目标`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`及`git diff --check`。纯前端状态归属修复，没有新增/修改/删除正式测试，没有重复Node或全量suite。

全局错误保留由枚举类别与清理分支核对，本轮未另外注入草稿/退出清理故障，不将上述三状态抽查推广为所有错误/平台矩阵。只处理打开会话及其响应校验错误，其他操作提示未扩大改动。
