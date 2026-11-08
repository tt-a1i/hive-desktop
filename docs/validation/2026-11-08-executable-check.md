# 外部程序检查

日期：2026-11-08。完整桌面产品仍在实施。

## 真实问题三问

1. 用户在外部程序页选择路径时，原先必须保存并启动聊天才能发现程序不存在或版本不对。本机 PATH 实际指向 Codex 0.46.0，已验证适配器使用的另一路径为 0.166.0，说明这个问题已经发生。
2. 每个程序现在有独立“检查程序”入口：检查当前输入、显示检测版本/本次实际命令与来源；错误路径可改后重试，未保存内容不写入配置。与已验证版本不同时明确提示尚未验证兼容性。
3. 不做则用户仍需靠终端定位问题、保存试错并启动会话；未保存的输入与环境覆盖也难以区分。

## 参考与实现

参考 Cindy 固定提交 `4fd670b72af9d060785e29c553a14b6794dfe8eb` 的 `apps/desktop/src/main/agent-binaries/binary-version-probe.ts`（本地源码只读核对）。借鉴有界 `--version` 和明确的版本输出形状；没有照搬其下载、托管安装或 semver 依赖。只读取版本，不启动 Agent 会话，不选择模型，不读登录状态，不修改全局默认。

- `desktop.settings.probe {backend,command}`：backend 为 pi/codex/claude，command 为可执行文件名/路径或 null；null 对应输入留空后的 PATH 查找。使用现有设置的字段校验与 env > 输入路径 > PATH 优先级，不保存。
- 返回 `{backend,command,source,status,version?,message,verifiedVersion}`。source 为 env/app/path/draft；draft 仅表示本次输入不同于已保存配置，在界面显示“输入路径”。status 为 available/not_found/failed/timeout/cancelled。
- `available` 仅表示程序正常返回可解析版本，不能证明协议、登录或模型可用。已验证版本是 Pi 1.4.0 / Codex 0.166.0 / Claude 2.1.317，不当作“所有更高版本都兼容”的最低版本门槛。
- 复用 OwnedCliProcess 及 Windows shim/Job 路径；仅传固定 `--version`，不拼 shell 命令。输出总量上限 64 KiB，5 秒无结果则停止检查；返回结果之前等待进程清理。运行时关闭会取消并等待活动检查，再关闭其余服务。
- 不返回程序原始输出，只返回解析版本和状态提示；没有输出/认证日志存储。未知输出、非零退出、启动失败分别可见。标准 EPIPE 只表示已经关闭 stdin，仍以真实退出为准；其他 stdin 错误会结束检查。
- 设置页输入变化会标记上次结果已过期；env 覆盖时仍显示实际检查结果，因为改变被覆盖的输入不改变实际命令。切换页面保留草稿和结果，后台断开清空检查状态，避免把旧进程结果带入新连接。

## 验证

`pnpm --filter @hive-desktop/runtime check:programs` 包括真实 Hive HTTP/SQLite、真实子进程和生产 JSONL：

- 文件名含空格/分号仍按字面执行，只收到 `--version`。
- 未保存路径不改设置、env 覆盖保留、严格拒绝缺失/非法参数、缺失命令、非零退出、stderr 版本、不可识别输出及输出上限。
- 真正运行超时进程并验证已退出；检查过程中关闭 runtime，RPC 返回 cancelled，相关进程也已退出。
- 本机原始 Codex 0.166.0、Pi 1.4.0、Claude 2.1.317 均实际执行 `--version`。这些是原始 CLI 版本检查，不是厂商模型调用。
- 既有 `desktop-settings.ts` 真实设置/原生外部 Pi 恢复检查，以及 `team-runtime.ts` 团队投递回归通过，无模型请求。

GPUI 浅色、深色与 760px 窄窗口：实际原生布局坐标的注入鼠标事件验证进入配置、旧 Codex 0.46.0 的版本差异、修改结果过期、缺失路径失败、切页草稿保留、改正为原始 0.166.0 后重试、env 覆盖、原始 Pi/Claude 版本、显式保存及中文聊天草稿保留。退出时仍有活动版本检查，应用退出后该进程已不存在。窗口输入数据为隔离夹具，不是系统物理输入或 Windows GUI 验收。

本地证据：`.artifacts/program-check/` 中的 core-receipt.json、wire-receipt.json、三种窗口 receipt/截图、最终日志及 fixture-source。首次测试误填不存在的 Pi 安装路径而失败，已改为实际 `/Users/tsk/Library/pnpm/pi`，没有为失败加产品兜底；首次夹具锁文件未适配临时依赖路径而失败，并误启动旧测试二进制，已停止那组隔离进程且未计入验收。仅临时夹具离线调整锁文件，生产 Cargo.lock 未改。窄窗首次夹具未先打开收起的导航，修正操作步骤后重跑通过，未改产品语义。

默认 Rust 检查 18 通过 / 1 fixture 忽略，Mac 生产构建与 Windows x64 MSVC 默认功能 cargo check 通过；未默认运行整个 Node suite，依据本次外部设置/新探针范围运行上述定向回归。Windows native-clis CI 已接线此检查和收据上传，但尚未取得该新增步骤的原生执行证据，不能外推 Windows 可用。

## 未完成范围

检查不是安装/升级，也不验证 CLI 协议完整兼容、账号、推理、完整协作或所有配置。正式跨平台安装与 Cindy 完整体验目标继续开放；生产包和 Windows CI 的本轮结果另按实际运行记录。

## 本轮开发包

冻结生产依赖重新部署，Mac ZIP 解压后通过真实 PTY/HTTP/SQLite smoke、原生窗口保存项目/恢复会话启动，以及包内自带 Node/runtime 的新版本检查 JSONL（含取消与进程退出）。二进制及所有 runtime JS 与当前构建逐文件比对一致，生产二进制无夹具入口。ZIP SHA-256：`82e5c90c1fb55165a2b88698950c3f394aca901a6e34ecb3fe4c8758c3a344ca`；二进制：`e684dd9450e1de8425e08230db77b25898d81f3c48a53d3780b47d15cbb0d731`。本地收据为 package-receipt.json / wire-receipt.json，仍是未签名/未公证的开发包。
