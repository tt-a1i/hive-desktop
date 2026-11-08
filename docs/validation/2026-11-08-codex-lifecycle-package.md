# 包内 Codex 生命周期验证与更新包

`.artifacts/hive-desktop-macos-arm64-codex-lifecycle.zip` 更新到工具日志、中断/拒绝、后台命令与默认分页历史的当前实现。原 reading 包的依赖继续冻结：先核对原 ZIP 的 SHA256，再确认锁文件、工作区设置、补丁及 runtime 包元数据（除开发 scripts）未改。整体替换当前构建的 104 个 runtime 文件及生产 Rust 二进制，重新生成 ZIP 并单独解压；不是沿用旧 JS 冒充当前 runtime。

## 包内检查

新增 `scripts/check-packaged-codex.mjs --bundle PATH`：复用现有两份 Codex 生命周期/分页行为检查，以项目已有 TypeScript 编译器去除类型，并把生产适配器入口绑定到解压包。测试进程和其 JSONL 子进程都使用包内 Node；真正执行的是包内 runtime 和生产依赖，不使用项目适配器。编译器仅是开发验证依赖，不是应用运行依赖。

检查覆盖累计输出、执行中快照、缺少工具完成事件的中断与失败、取消投递不误确认、回合结束后的后台工具、迟到输出与完成、原生历史/显示账本重开、新分页线程不得提前读取历史。JSONL 对端明确为合成实现，没有厂商/模型请求。分页探针增加 finally 清理自己的临时目录。

反向校验：在解压副本临时回填旧 reading 包的四个 Codex 适配器模块，检查在“执行中工具快照缺失”处失败；随后恢复当前文件，逐一验证 104 个 runtime 文件哈希，再运行正向检查通过。原始新 ZIP 未被修改。证据位于 `.artifacts/codex-lifecycle-package/`。

## 实际运行

新 ZIP 解压后以下检查均通过：

- `node scripts/check-package.mjs --bundle PATH`：内置 Node、真实 PTY、Hive HTTP/SQLite、工作区与关闭。
- `node scripts/check-packaged-codex.mjs --bundle PATH`：上述合成 JSONL 生命周期与分页行为，实际运行包内适配器。
- `node scripts/check-desktop-startup.mjs --bundle PATH`：生产原生窗口实际打开保存项目并恢复会话选择。
- 使用包内 Node/runtime 连接已安装的原始 Codex，恢复上一轮真实 Terra 分页线程：线程/工具 ID、失败终态及两段实际输出与原记录一致；这是只读恢复，没有新推理回合。

依赖复用、完整 runtime 更新、包内文件哈希、生产入口无夹具标记和 ZIP 哈希均保存在收据中。没有重新跑无关 Rust/Node 全量测试；本批产品代码没有改变，只增加可复用的包检查、CI 接线及探针清理。脚本完成 Node 语法检查，工作流完成 YAML 解析。

Windows 工作流增加解压包的 Codex 检查步骤，目前仍是接线，不能算原生执行。最新已核对的 Windows CI 因付款/支出限制没有执行步骤，见上一轮记录。本包仍未签名、公证，仅适用于 macOS arm64；Windows GUI、物理输入、原生实时日志、真实其他后端与正式发行继续待验收。完整产品目标保持开放。
