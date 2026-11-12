# 78fef23 开发包：Codex 长历史与写盘一致性

2026-11-12。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新到 `78fef23612225f0407f4eea62198ca334bc2ba2e`，包含[快照来源索引](2026-11-12-codex-snapshot-index.md)与[显示日志恢复/写盘顺序](2026-11-12-codex-recovery-completion.md)。未签名、未公证的 Mac ARM64 开发包；完整产品验收继续开放。

## 冻结依赖复用

Git diff 核对 3a18b94 到本提交的原生源码/锁文件、vendor/crates、pnpm 锁/工作区、生产 package.json 与全部 patches 均无变化。复用 3a18b94 已冻结并验证的生产依赖、官方 Node 22.23.3、原生程序与资源，没有重复生产部署、下载或构建 Rust。冻结依赖基线仍为 3a18b94，不把本次标为新部署依赖。

复制前先核对旧解压树与原 28,869 项清单完全相等及回退 ZIP 哈希，再替换后台 dist。完整差异精确只有四项：codex.js/map 与 codex-projection.js/map。所有生产元数据、依赖、Node、图标和原生程序字节保持。116 个 runtime 文件匹配当前源码 dist，独立 ZIP 解压后全部文件/权限及 29 个内部链接匹配。

## 包内实际验证

- session-check.mjs 实际加载包内 CodexSession，连接真实 Node JSONL 受控进程，读取 50 页原生形状的合成 5000 回合/10000 条历史。来源重复/回退、动态新增输入及完成投递、完整冷重开通过；初始与新增回合快照 SHA256 精确匹配源码记录。没有把受控历史冒称真实厂商生成。
- 本批并行包检查期间，该夹具会话装载单样本 105.75ms，首快照 8.43ms、后五次快照中位 1.51ms。实际包内恢复 5000 回合的四样本中位：已有缓存 8.87ms、原生导入 5.33ms、legacy 导入 8.84ms，包含真实 JSON 写入。这些是具体组件/受控会话数据，不是窗口延迟或全应用性能保证；源码前后样本见两份源记录。
- benchmark.mjs 六场景（1000/5000 回合 × 三模式）落盘完整 JSON 哈希匹配源码；二次恢复文件逐字节相等及新建对象冷读保持。parity.mjs 直接比较 51d3b49 旧类与包内新类，32 组 native/legacy、四状态与空/缓存/导入/混合场景的首次/再次/冷读 JSON 全部相同，旧重复标记保留。
- failure.mjs 以真实 EISDIR 令 rename 失败，覆盖 recover/complete。故障修复后接受部分原生 item，未提交的完成标记没有进入后续文件，历史缺失提示存在；旧 canonical 备份字节保持。通过公共显示结果核对，没有源码字符串断言、mock 写入或吞异常。
- 使用包内 runtime 只读打开旧真实 Terra 一主控/两成员，完成 delivery IDs、原派单来源、模型、pending 0 与无待处理汇报保持。随后从包内正式 main.js 读取同三会话，保留原 stdout 帧，由原生同版本 serde_json 1.0.154 解析并断言历史、模型、非空完成回执、busy false / pending 0。没有 prompt / Hive resume 或新模型回合。
- 四个既有包检查实际 exit 0：check-package.mjs、check-packaged-codex.mjs、check-packaged-dispatches.mjs、check-packaged-queue.mjs，均 `--bundle .artifacts/release-78fef23/extracted/bundle`。其真实 PTY/HTTP/SQLite 与合成协议范围保持，没有扩展为厂商推理。

本批未打开原生窗口。原生代码/程序完全未变，实际后台响应通过同版 Rust；这不新增原生交互、Windows 或完整 UIUX 验收。

## 安装与交付物

DMG 创建/校验及只读挂载通过，完整 app 清单与 Applications 链接匹配并已卸载。替换 Applications 前核对旧安装清单与无打开文件，分阶段复制/替换后完整新清单相等。安装副本本批未另启动。

ZIP 143,936,186 字节，SHA256 `049c7a2843892707e88d725f4a3426fc08d145ffdcb655f852e692a0e0efaeba`；current/candidate 为同字节硬链接。

DMG 231,913,616 字节，SHA256 `e27e7b1de792b281621ea786567f569a9f2e249e433aa0a1beac7914e6c5ab00`。

组装/差异/原始夹具/实际 gate 退出码、真实旧会话、JSONL/Rust、DMG/安装/发布收据保存在 `.artifacts/release-78fef23/`。

## 边界

源码类型检查/构建、五个现有 Codex 回归及真实文件错误/全量输出差分见源记录；未新增、修改或删除正式测试，未跑全量测试：仅桌面 Codex 临时查找索引与显示日志提交顺序，使用实际原生协议、数据/文件错误、动态追加、冷恢复和包级检查验证。依赖锁与原生源码未改，不重复相同构建。

没有面板操作或厂商模型调用。Pi OAuth 继续搁置，真实推理仅 Terra；Claude 模型、系统通知授权、Windows 真机、正式签名/公证及完整产品体验仍待验收。没有回写旧错误会话或把显示日志完成标记当作 Hive 派单验收。

## 存储整理

核对新安装/旧回退的完整清单、回退 ZIP 哈希及无打开文件后，删除重复 base 与旧安装临时备份，合计 916,551,227 逻辑字节。当前解压包、3a18b94 可运行回退及 ZIP、原始状态/日志/收据与源码保持；没有独立生产部署树，也未清理共享 Rust/pnpm 缓存或用户项目。清单见 cleanup.json。
