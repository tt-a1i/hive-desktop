# Hive GPUI 聊天选型实验

这是已经在 macOS 编译、启动并走查的独立原生 UI 原型，不是 Hive Desktop 成品，尚未接入 Agent 对话。macOS 验证证据与未验证项见 [VALIDATION.md](VALIDATION.md)。

## 运行

```sh
cd /Users/tsk/code/hive-desktop/experiments/gpui-chat
rustup toolchain install 1.98.0 --profile minimal
cargo +1.98.0 build --locked -j 3
HIVE_PROBE_URL=http://127.0.0.1:58591 cargo +1.98.0 run --locked -j 3
```

示例端口 `58591` 属于本次隔离 Hive 后台，后续运行需替换为实际隔离实例端口。根任务还创建了本地演示包 `.artifacts/Hive GPUI Probe.app`，并通过 CUA 实际打开和检查；它不是签名安装包。

`HIVE_PROBE_URL` 可在启动时设置，也可在窗口内编辑。应指向隔离 Hive 实例的 base URL；点击「真实 GET」才请求，启动不自动连接。只接受 `http://` 加 loopback IP 和显式端口，例如 `127.0.0.1:4173` 或 `[::1]:4173`。探针依次 GET `/api/ui/session` 获取 `hive_ui_token`，再带 cookie GET `/api/workspaces`；cookie 不写日志。它不支持 TLS、重定向或完整 HTTP 客户端语义，单次响应读取上限 64 KiB，界面展示 HTTP 状态与一小段响应内容；没有后台写操作。

## 依赖与构建

使用官方 `gpui-kit =0.9.0`，Cargo.lock 固定间接依赖，包括 `gpui-pre =0.3.11`。项目工具链锁定 Rust **1.98.0**：1.93.1 实际构建因 `std::hint::cold_path` 失败，1.95.0 又因 macOS 后端使用 `Option::map_or_default` 失败；1.98.0 构建成功。安装项目工具链没有改变全局 Rust 默认。

Kit 的标准 platform 依赖启用 `runtime_shaders`；本机仅有 Xcode CommandLineTools，没有安装完整 Xcode 或 Metal 编译器，没有修改第三方源码或使用不稳定编译绕过。构建使用 3 jobs、关闭 dev debug 信息以控制资源。`build.log` 保存本机编译输出；`run.log` 保存直接启动时的运行事件，bundle 运行证据由根任务记录。

## 原型行为

- 会话与成员侧栏可切换选中状态，当前共用演示消息，没有独立会话数据。
- 聊天使用原生 `Textarea` / `TextareaState`：初始 3 行，最多自动增长到 6 行，Enter 换行；后台地址继续使用单行 `Input`。发送只追加到本地演示，随后清空输入；演示流期间发送按钮禁用。
- 原生 `TextView` 展示可选择的 Markdown、代码块、增量文本与预置滚动内容。所有预置内容和定时器流均标记为 **UI 演示，非 Agent 输出**。
- 标准输出记录启动、成员选择、输入字符数与行数、演示发送、演示流开始/完成及真实 GET，不记录输入正文或 cookie。
- 入口已显式接线 macOS `Cmd+Q`、其他平台 `Ctrl+Q` 和最后窗口关闭时退出原型进程。它不关闭独立 Hive 后台；这些退出交互尚未通过 CUA 复验。

## 已验证与边界

macOS bundle 已通过 CUA 实际查看：真实 GET 显示 `session 200 → HTTP/1.1 200 OK · []`；成员选择、演示 Markdown/代码块流、中文多行粘贴、Enter 新增一行、发送清空和发送按钮禁用均已观察。中文 IME 候选窗组合输入**未验证**；CUA Unicode `typeText` 和剪贴板 `paste` 表现不同，不能据此前者断言 IME 失败。

预置滚动内容仅为数 KiB、8 次重复的演示文本，不能据此判断超长历史性能。最新 `ps` 快照中原型约 132.8 MiB RSS，隔离 Node 后台约 55.7 MiB，合计约 188.5 MiB；未包括 CLI 或模型进程。这是 debug 演示快照，不是技术栈性能比较。

退出接线已编译通过。后续 CUA 重抓屏在 ScreenCaptureKit `-3811` 处失败，阻止了 `Cmd+Q` 和关闭窗口后的进程退出复验；此前截图和 AX 获取均成功。这一限制不应写成退出行为验证通过。

Windows 本应用的 [CI workflow](../../.github/workflows/gpui-probe.yml) 已由根任务编写，**尚未运行**；Windows 编译和真机运行均未验证。应用使用跨平台 GPUI Kit 与 Rust 标准库，但平台后端的存在不等于本应用验证完成。尚无 Windows 安装包、签名、更新验证。

尚未实现：Agent SDK/RPC/ACP 接线、团队操作、工具执行视图、会话持久化、Agent 停止/恢复、产品后台生命周期管理、生产 HTTP 客户端、完整键盘/无障碍校准，以及超长会话虚拟化与性能预算。

## 后台独立自检

需要空隔离 Hive，复用 UI 的同一个请求 helper，断言认证后 HTTP 200 和空工作区列表：

```sh
rustc +1.98.0 --edition 2024 checks/backend.rs -o /tmp/hive-gpui-backend-check
HIVE_PROBE_URL=http://127.0.0.1:58591 /tmp/hive-gpui-backend-check
```

## API 依据

- [官方 hello_world](https://github.com/longbridge/gpui-kit/blob/v0.9.0/examples/hello_world/src/main.rs)
- [官方 input](https://github.com/longbridge/gpui-kit/blob/v0.9.0/examples/input/src/main.rs)
- [官方 stream-markdown](https://github.com/longbridge/gpui-kit/blob/v0.9.0/examples/stream-markdown/src/main.rs)
