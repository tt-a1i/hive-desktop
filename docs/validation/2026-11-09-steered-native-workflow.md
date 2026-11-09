# 同回合通知的原生恢复与工作流详情

## 实际界面验证

本轮使用生产 `ca0023c` runtime 和原生 GPUI 开发应用，对上一轮真实 Codex/Terra 工作流历史作实际 Mac 窗口补验。不是重新生成模型回复，也不是合成 native peer。

原数据 `.artifacts/workflow-steered-main-final` 保留；验证数据位于 `.artifacts/steered-native`。通过原始 Codex `thread/fork` 建立独立分支 `01a1bfff-adb3-7492-b10e-763755727d08`，沿用原生完整回合/item ID，并复制对应桌面来源元数据。此过程不启动模型。原始主控 thread 及验收收据不作为本轮写入目标。fork 行为依据[官方接口](https://learn.chatgpt.com/docs/app-server#api-overview)，实际返回及单回合 ID另有脚本断言。

原生实际查看：

- 主控恢复后，系统输入显示为“团队通知”，默认折叠；展开原文能核对 run `1650d754-c4b5-484f-a75d-a77dada6c1fc`、原始结果与47/3汇报，收起恢复。
- 主控最终答复只有原来的 `MAIN_STEER_OK sum=47 count=3`，通知没有被当作第二条普通用户消息，也没有再请求模型。
- 工作流列表/详情读取同一实际记录、结果、派单和日志；进入成员会话展开工作过程，实际 `team messages` 和 `team report --seen 0 --artifact ...` 均显示完成。
- 从成员返回工作流，先前阅读位置保留。
- 新构建窗口再次读取相同结果，产物只读预览显示 `{"sum":47,"count":3}`；退出预览返回原详情。

## 发现并调整的布局

详情页原先在运行信息前重复显示全局实验开关和执行器配置卡，挤占结果阅读区。现在只在运行列表页显示该卡；详情直接显示当前运行信息、结果和成员。沿用原组件和状态，一处条件渲染，没有新增设置、依赖或业务接口。

[修改前](assets/steered-native/detail-before.png) · [修改后](assets/steered-native/detail-after.png) · [1184×1000窄窗](assets/steered-native/detail-narrow.png)

实际验证列表仍有全局开关和执行器入口；窄窗返回列表、展开执行器可看到原 codex 默认/允许项，保存按钮仍是无改动状态。未切换执行器或更改功能开关。较窄窗口的导航遮罩可通过原有按钮关闭后阅读结果。

## 检查与收尾

通过：

```sh
cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop
cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc --bin hive-desktop
git diff --check
```

只改 Rust 条件展示，未新增 UI 测试，未重跑后台/模型套件或全量测试。新原生程序与 target 构建硬链接及 SHA-256 已记录。runtime 使用源码构建产物，Node复用既存便携包的22.23.3；不称作新包验收。

两轮原生 PID 36862、40912 都由 Cmd+Q 正常退出、exit0，随后确认 PID 不存在。克隆主控元数据仍为user/system两条独立来源、两条已完成delivery；按实际send行中的JSON方法名统计：首轮initialize=1/chat.open=2，次轮initialize=1/chat.open=1；两轮chat.prompt/chat.resume均为0。首次按JSON method字段匹配得到空计数，已修正并增加上述正向控制，不以空匹配证明没有调用。截图、原生日志、fork/准备脚本、进程/退出与 receipt 均在 `.artifacts/steered-native/`。

## 未覆盖

本轮补的是冷历史的实际展示与结果阅读，不代表已验证原生窗口中正在流式执行时的追加通知；动态长历史、所有字号/语言/VoiceOver、Windows原生、定时计划最终删除及跨连接在途操作仍待。未更新候选0e72137 ZIP，current未推广；#13及完整桌面目标保持开放。
