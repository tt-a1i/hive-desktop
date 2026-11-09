# 同回合通知与近期界面修复进入候选包

## 产物

- 源码基线：`768d085081db488cc307d565d59258d46075c31f`。本轮包检查器的ESM解析修复另为 `c6df676`，不改变归档内生产文件。
- canonical ZIP：`.artifacts/hive-desktop-macos-arm64-candidate.zip`，171,866,310字节，约164MiB。
- ZIP SHA256：`b792ae16d978801a3d1c479f7538604ad44fcb70cb992e4abb69f0f6516b6881`。
- 原生程序 SHA256：`509e049e3fbe5113e0aefb1006a1d136b6eceeeb91bcc86453aee1ba1daf641e`。
- current仍为1acd805，SHA256保持 `f7174572fb16248b3a3f40e467a1322951a9ac4a26bd5c728ce0e52aceb57cf6`，没有推广候选。

包纳入空会话恢复保持空闲、Codex系统通知进入当前回合及来源/结算修复，以及远程/模型焦点显露、紧凑设置导航、工作流详情优先展示结果。仍为未签名/未公证的Mac arm64便携开发包，未作为完整产品交付。

## 构建与逐项核对

以已验证0e72137包为部署基线，只替换实际变化的10个runtime JS/map、脚本元数据和原生程序。锁文件、补丁、依赖及部署变换保持原值；不是重新执行生产依赖部署。

独立解压校验：112个runtime文件全部与当前生产dist匹配；28,822个未变普通文件、29个内部链接及31,570个AppleDouble元数据条目逐一保持，ZIP testzip通过。原生程序与上一轮Mac实际走查构建的SHA一致。

`.artifacts/steered-candidate/build.py` 保存这一次更新的严格允许列表、构建和对比；不要把它当作可以对任意新基线盲目重跑的通用打包器。

## 四组包检查

实际执行以下脚本，均exit0：

```sh
node scripts/check-package.mjs --bundle <独立解压bundle>
node scripts/check-packaged-dispatches.mjs --bundle <独立解压bundle>
node scripts/check-packaged-queue.mjs --bundle <独立解压bundle>
node scripts/check-packaged-remote.mjs --bundle <独立解压bundle>
```

分别穿透包内Node/真实PTY/Hive/SQLite与关闭；已汇报派单队列结算、Codex原生工具确认和同回合通知、空会话恢复；内外部Pi队列/附件/暂停/冷恢复；远程加密HTTP/终端/撤销恢复。

通知的多来源、早到消息、停止/缺应答、未消费/未持久化、重试旧来源、拒绝/断线/写盘失败和用户排队顺序在合成JSONL厂商peer中检查，不称作真实模型运行。检查代码来自checkout并擦除类型，所有生产模块、依赖和执行Node来自独立解压包。

新增空会话检查的SDK定位初次遇到 `ERR_PACKAGE_PATH_NOT_EXPORTED`：检查器用CommonJS解析了import-only的Pi SDK。改由包内Node的ESM loader解析，原SDK/runtime/依赖均未改动；原失败日志和重跑结果保留。

## 包内真实主控与 Worker

`.artifacts/steered-candidate/live/` 使用包内Node和runtime、原始Codex CLI 0.166.0，两位Agent均固定 `openai/gpt-6.1-terra`，不改全局模型或认证。

- 工作区 `3bb015b2-37e7-45ec-b52a-e79111cd5068`；主控会话 `2c4c190f-5a2e-4415-90ea-cdd50eb8c806`。
- 主控真实读取预存源码，通过原生team工具启动该原文；驱动不启动workflow、不替Worker提交report。只有一个run `9d9e1147-96c2-4981-8d43-1b419542b727`。
- Worker会话 `ccf298c7-831d-4ec7-ab0f-f9214d5f6aaa` 实际读入 `[5,19,23]`、写出47/3、messages/seen/显式带产物report；原生report工具completed/success=true，成员pending=0，输入原字节不变。
- 验证驱动只在实际workflow terminal时打开自有fs.watch gate，让主控在等待期间收到真实完成通知，不注入计算结果。
- native turn `01a1c009-2651-7fd0-8637-04a6f6e88491` 中保留user/system两个不同item和两条已完成delivery；主控实际读取输入/产物、show同一run，只有一条final `MAIN_STEER_OK sum=47 count=3`。
- 完全关闭并新建runtime冷恢复后pending=0，完成ID和来源保持。receipt记录实际包内Node及runtime绝对路径；audit另检查源码原文、同run和native item关联。

这轮请求由验证驱动送入生产聊天接口；没有冒称用户在原生窗口输入或已验收实时流式追加界面。

## 实际原生包

直接运行解压包的可执行程序，不提供HIVE_DESKTOP_NODE/RUNTIME源码覆盖。实际窗口读取上述主控唯一final及折叠通知、同run详情的新布局、47/3产物预览。设置宽窗进入后缩至1184×1000，九入口菜单/当前勾号、切换远程分区、Escape直接返回工作流而不留导航遮罩通过。未更改远程、执行器、语言或全局模型设置。

原生PID52352通过Cmd+Q正常exit0，随后PID不存在。实际RPC trace有initialize/chat.open正向记录，chat.prompt/chat.resume为0；窗口走查没有再次发起模型请求。

截图、进程/退出、原生日志、四组gate及真实模型receipt保留在 `.artifacts/steered-candidate/`。

## 清理与边界

确认所有gate/模型驱动/原生窗口终止，解压目录及模型项目lsof均无打开引用后，清理本轮解压副本，再原子替换canonical候选ZIP；旧0e候选ZIP不额外保留，旧收据标为superseded。current未改，`.artifacts`回到约3.9GiB，没有删除旧聊天工作目录或现有源开发包依赖。

本轮不重复无变化的源码全量suite、所有页面/字体/语言矩阵或远程原生配对；这些证据按原记录保留。Windows原生/正式签名发行、实时原生追加界面、其他厂商同回合能力、Pi登录、Claude真实模型及完整Cindy/Maka体验仍有缺口。完整目标保持开放。
