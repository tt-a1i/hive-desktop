# 桌面外观设置

## 对照与场景

参考本地 Cindy 源码快照的 `appearance-settings-store.ts` 和设计规范亮暗双模式要求：应用级外观独立于 Agent 配置，写入本机用户数据，切换后即时应用。

原先 Hive Desktop 只能跟随系统，用户夜间希望固定深色时必须修改整个系统。现在设置增加外观页，可选跟随系统、浅色、深色，保存后立即更新共用主题及 Hive 调色板。系统外观事件尊重固定选择，重新启动恢复选择；不做此项则用户无法单独控制 Hive 外观。

## 实现

- 使用现有 GPUI Theme 和 Hive 调色板，未新增 UI 库、依赖或 Agent 权限。
- 配置在桌面数据目录的 `desktop-appearance.json`，同目录临时文件原子替换，保存成功后才修改当前选择。读取失败保留原文件并在外观页显示错误。
- 数据目录沿用后台 initialize 返回值，没有复制 macOS / Windows 路径规则。后台断线后的已加载外观设置仍可在本机保存；首次连接前尚不知道目录时禁用选择。
- 新启动窗口在后台返回前短暂采用系统外观，加载配置后应用保存值；本轮没有宣称解决首次绘制的主题闪烁。
- 设置默认打开外观页，其他设置入口和未保存内容沿用原逻辑。

## 验证

1. `cargo test --locked --manifest-path apps/desktop/Cargo.toml`：新增本机文件检查覆盖三种值的实际替换和重新读取、缺省系统模式、损坏文件报错且未覆盖原内容。
2. 真实 GPUI 原生窗口验证：临时复制应用源码，仅在 main 添加断言驱动，使用原版 GPUI 和真实 Node 后台。三次独立应用启动分别检查预存深色 → 选择浅色、重开恢复浅色 → 选择深色、重开恢复深色 → 选择系统。每次验证实际 Theme、磁盘 JSON、重新应用系统外观后的固定模式及重新 restore 不覆盖选择。三个进程均输出 `APPEARANCE_NATIVE_OK` 并正常关闭后台；没有模型请求。
3. `cargo build --locked`、`cargo fmt --check`、`pnpm check`。临时驱动没有进入生产代码，之后从正式工作区重建二进制。

## 待验收

Mac 锁屏使 CUA 无法访问原生窗口，尚未完成按钮鼠标/键盘操作、浅深主题视觉对照与窄窗口走查。上述原生断言验证实际应用状态，不等同于视觉验收。Windows 真机及最新构建结果也未在本轮取得。Cindy 的其他外观能力没有整体搬入；完整桌面体验目标保持开放。

正式工作区重建后的 macOS arm64 开发包位于 `.artifacts/hive-desktop-macos-arm64-appearance.zip`。包内 Node 的 `check-package.mjs`、`check-desktop-startup.mjs` 与 `check-pi-shell-owner.mjs --native` 通过，确保新 main 的外观初始化没有破坏之前的后台 / Shell 清理路径。包尚未签名公证。本轮未运行所有模型与后端 suite，因为没有更改 Node 协议或 Agent 逻辑；界面视觉和 Windows 验证仍保留上述边界。
