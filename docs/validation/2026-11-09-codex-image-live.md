# 原生 Codex Terra 图片与恢复

2026-11-09，在既有图片上传/聊天链路上取得原生Codex的真实识图证据。它补充此前内置Pi因OAuth invalid_grant失败的记录，不把后者改成成功。

## 实际运行

执行已安装的原样 `/Users/tsk/.npm-global/bin/codex` 0.166.0，app-server stdio接入，明确选中openai/gpt-6.1-terra。PATH首位 `/opt/homebrew/bin/codex` 仍是0.46.0，本次显式指定路径，没有替换全局CLI、默认模型、认证或使用Magpie。

自建320×180 PNG，左蓝右粉，经真实Hive HTTP/SQLite上传副本、生产Chats.prompt的图片引用及Codex原生图像输入发送。提供商实际返回：`左：蓝色；右：粉色`。没有工具调用、派单或文件读取；pending=0，非中断settled完成，选定模型仍为Terra。

原图SHA256：`0834812fa26e2a6c3b7b18d7725d03c4a1695721cfdcc9ac6e0892048eb48ed5`。真实原生线程：`01a1bc14-b958-72e0-ab03-b57c2697f0fc`。上传、原生历史和完成标记收据见 `.artifacts/image-attachments/live-88bwiS/receipt.json`、`snapshot.json`、`events.json`。本轮只发送一条显式图片消息，不能据此统计厂商内部重试次数。

整个runtime正常关闭后，在新的runtime中只打开同一会话。原生thread ID、completed delivery IDs保持，busy=false/pending=0；回答逐字相同，恢复历史的base64原图、Hive上传副本的SHA256均与原图一致。未再调用prompt，没有新settled或tool_start事件。见 `.artifacts/image-vision-codex/recovery-receipt.json`、`recovery.log`。

## 检查入口调整

`checks/image-attachments-live.ts` 增加显式 `--backend pi|codex|pi-rpc`，仍默认Pi、必须传--live，固定Terra。Codex的实际历史没有Pi特有的model/stopReason字段；按它的实际选定模型、settled/队列/错误状态核对，不向生产投影补造字段。失败时保留backend信息，成功保存实际snapshot，读取错误时不等待到完整超时才报错。

```sh
HIVE_CODEX_COMMAND=/Users/tsk/.npm-global/bin/codex pnpm --filter @hive-desktop/runtime exec tsx checks/image-attachments-live.ts --live /absolute/path/blue-left-pink-right.png --backend codex
```

本次真实检查、完整runtime重开和 `pnpm check` 通过。没有生产源码、Hive协议/schema、依赖或默认配置变更；不重跑无关全量业务suite，也不为检查脚本重新打包。

## 证据边界

取得的是Mac原生Codex经实际Hive图片链路的真实模型与恢复结果，不是系统鼠标/图片选择器或Windows真机验收，也不证明复杂截图、多图长历史的识别或性能。内置Pi早先OAuth失败、外部Pi实际图片推理、Claude在仅Terra模型约束下的未验收状态分别保留。该结果不替代当前导出窗口的复制、窄窗和正常退出检查。
