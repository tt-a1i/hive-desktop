# 工作流阅读与启动焦点：更新开发包

current/candidate 已更新为源码 `61488017561e0e5e821c07b0dfec9ed6fad6719e` 的Mac ARM64优化release，ZIP **143,419,605字节**，SHA256 `ea78c52c37a8273e541c79229e560b2a2915c0f9ad417caec8623419a24e0321`。

- ZIP：`.artifacts/hive-desktop-macos-arm64-current.zip`，candidate为同字节硬链接。
- 实际解压：`.artifacts/release-6148801/extracted/bundle/Hive Desktop.app`。
- 收据：`.artifacts/release-6148801/receipt.json`及`.artifacts/current-package/receipt.json`。
- 未签名/未公证开发包；未达到完整产品交付。

## 纳入与包检查

纳入6edac4c之后的协作任务正文默认收起、草稿恢复光标/冷启动输入焦点、定时计划可读正文、工作流结果/日志阅读与列表刷新位置、键盘逐页阅读、计划确认说明及具体计划操作名称。旧功能保留，不冒称全部重新原生验收。

`cargo build --locked --release --manifest-path apps/desktop/Cargo.toml --bin hive-desktop` exit0，33.07秒。依赖声明、锁文件、补丁、工作区配置及runtime源码与前包一致，复用已验证的同平台冻结生产依赖，不宣称全新deploy。独立解压后112个runtime输出、28,863个文件/模式条目、29个内部链接逐一匹配；唯一变更为原生程序，其SHA256为 `a55379ffdfda3c59f67a4d4bd4e0eacdca0dac6616e8f3a175ae283173067010`。

包内Node执行六组检查：`check-package`、`check-packaged-models`、`check-packaged-dispatches`、`check-packaged-codex`、`check-packaged-remote`、`check-packaged-claude`全部exit0。真实SDK/HTTP/SQLite/PTY与合成协议对端范围沿各脚本保留，不等于Claude/Codex真实厂商推理或生产远程验收。

## 实际解压包窗口

只设置隔离data/Pi目录，不设置HIVE_DESKTOP_RUNTIME或HIVE_DESKTOP_NODE，实际使用包内后台和Node。原始两套状态的22个文件哈希在复验后仍一致；窗口操作使用副本。

1. 真实三终态工作流副本，English20字号。冷启动后不点击输入框直接键入 `_PKG_CHECK`，准确追加到原中文多行草稿末尾，随后逐键删除恢复原文。Tab进入工作流、到完成卡、Space进入详情，完整12条日志可读；宽窗End、592×506窄窗阅读区End到LOG_12/来源参数，Home/Next翻页通过。缩窗后第一次Tab/End停在结果复制按钮，随后ShiftTab明确进入有边框阅读区才成功，不把第一次尝试算通过。
2. 真实定时执行副本，A有唯一完成运行/last_run_at，B从未触发。原草稿保留；实际设置切换English20，工作流读取两条暂停计划及Last run。Tab到A删除入口、Space仅展开确认，AX完整读出原计划名、英文保留历史说明、确认及取消名称。592×506窄窗滚到确认区，说明与两个动作完整显示；点击Cancel后确认区消失。最终删除未执行。缩窗/切换导航后两次Tab没有建立可见取消焦点，不计键盘窄窗取消通过。

两个原生进程均Cmd+Q exit0。未发送用户输入、未继续暂停队列、未新增厂商推理。原始Codex/Terra的真实请求证据沿用相同字节后台的[前包记录](2026-11-10-composer-package.md)，本轮不是重新推理。

## 包内独立冷读与保留

`check-workflow.mjs` / `check-schedule.mjs`由新包Node执行并导入新包runtime，均exit0：三条终态与14条日志不变、精确日志/错误草稿不变、三条系统通知保留原ID且暂停；两条计划的ID/参数/源码/时间保留且暂停，唯一真实定时运行逐字段不变，中文选区草稿精确保留；两套messages/dispatches/dispatch_messages均0，原模型历史保持，计划副本English20偏好写盘。原有文件/状态逐项哈希不变。证据根`.artifacts/release-6148801/`。

原6edac4c ZIP/收据归档回其目录，原可运行解压包保留。仅在lsof确认无打开引用后删除新包重复base目录，分配块统计清理538,435,584字节（约513.5MiB）；源码、ZIP、状态、失败尝试及证据均保留。

未新增/删除测试，未跑全量suite；风险匹配验证为六组包gate、实际包窗口与独立冷读。此前源码Windows目标编译通过，但本包Windows原生、正式安装/签名公证、完整VoiceOver/IME/视觉/性能、最终删除及全部协作/远程矩阵仍未完成。#1和#13保持开放。
