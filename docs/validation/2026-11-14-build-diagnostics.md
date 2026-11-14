# 桌面构建与后台版本诊断

2026-11-14，工作项 [#34](https://github.com/tt-a1i/hive-desktop/issues/34)。这是完整设置体验的一项补齐，完整产品目标继续。

## 行为与来源

“设置 → 关于与诊断”可从导航或中英文关键词搜索进入，显示桌面版本、构建标识、构建类型、平台，以及实际后台的运行时、Node.js、Hive 包版本和平台。“复制诊断信息”保留完整提交标识和限定字段，给出实际复制成功提示。

- 桌面标识在构建时从本项目 Git 仓库读取；带“含已跟踪文件修改”的构建只声明已跟踪文件的状态，不声称整个目录干净。复制字段明确命名 `trackedFilesModified`。源码导出或非 Git 构建显示无法获取，不借用父仓库提交；保留现有 Windows 主线程栈设置。
- 后台在自身启动时读取实际模块所属的 package.json，并核对包名；Node 版本和平台来自当前进程。部署目录、启动 cwd 和桌面所在的另外一个源码仓库不会替代这些来源。Hive 包没有可验证的 Git 提交，不伪造来源 revision，也不调用联网更新服务。
- 原 `initialize.version` 与既有字段保留，新增 `runtimeInfo`。旧后台响应仍可读取已报告的版本，缺失字段显示无法获取；元数据损坏不阻断后台初始化。
- 连接失败或重启清除旧后台身份，只采用当前连接代数的初始化结果。读取会话时共享的加载状态不会把已连接后台误报为正在启动。
- 复制只包含固定版本、构建和连接字段；不复制整个初始化响应、错误正文、配置目录、环境变量、账号凭据或聊天内容。

## 已运行的检查

| 检查 | 本轮结果 |
| --- | --- |
| 类型检查与 `check:runtime-info` | 退出0；实际编译 JSONL 子进程在无关 cwd、隔离数据/Pi目录中初始化和关闭，精确核对原字段及运行进程/包版本 |
| 损坏/缺失元数据 | 实际编译 reader 在隔离包中执行；无效 JSON、错包名和空版本返回缺失，不致启动异常 |
| `scripts/check-build-identity.mjs` | 实际 Cargo/Git 构建：稳定缓存、未跟踪增删、暂存、提交、已跟踪编辑、嵌套导出及非Git来源均核对通过 |
| Rust 主程序测试 | 44通过、0失败、1忽略；含旧初始化/白名单/超长及异常字段处理 |
| Mac debug与MSVC交叉检查 | 退出0；不等于Windows原生或release验收 |
| 英文词条闭合 | 1369键，无遗漏的直接中文 `tr()` 字面量 |

构建身份回归将未跟踪增删保持为“已跟踪文件未修改”，暂存新文件为修改，提交后恢复并更新提交；不以只观察一次 Cargo 缓存的偶然刷新判断成功。日志位于 `.artifacts/product-completeness-20261114/version-*.log`。

## 已完成的原生调试操作

使用独立 Hive Product QA 身份与复制的 `version-ui-state`，未替换运行中的用户安装实例，未启动模型、登录或账号检查。

中英文20px、822×503及宽窗中，导航/搜索Enter、所有桌面与后台字段、滚动到页脚及从底部Tab显露复制均实际通过。初次对滚轮无响应的判断已撤回：CUA的方向与预期相反，实际滚动可用；不将其记录成已修复的产品故障。

在实际窗口复制后，主进程独立从剪贴板核对完整桌面提交、已跟踪修改标记、开发构建类型/目标和后台 `0.1.0 / v22.23.3 / 2.2.1 / darwin / arm64` 及精确白名单。只强制结束该QA实例拥有的Node子进程后，窗口显示断开，独立复制结果中后台为null、桌面身份保持。实际点击重新启动，版本恢复且旧复制提示清除；再次冷启动后复制内容独立核对通过。最后一次复制后不再进行搜索/粘贴，正常退出后剪贴板内容仍完整一致。

两轮QA正常退出码0，原Node及重启后的子进程均消失。原生证据为 `version-native*-process.json`、`version-sidecar-stop.json`、`version-clipboard-{connected,disconnected,cold-connected}.json` 与 `version-clipboard-after-quit.json`。另一次延迟读取没有取得有效JSON，未计为成功，之后按立即读取/直接退出重新验证；不把该工具时序归因为应用故障。

## Self-Review

四个独立角度审查发现构建缓存与未跟踪状态范围不一致，以及会话加载可能误报后台启动。已明确收窄为已跟踪文件、同步UI/复制字段及稳定缓存回归，连接优先级也已修复。复核架构、边界和测试均A，范围B+（当时等待原生窄窗验证）；后续原生检查按上文通过。没有将静态复核称为独立完整实机重跑。

## 优化开发包与实际运行

本批已进入 current/candidate Mac 开发包 `8cd9c8453ff3d4708c946994691725c259a796cc`，默认优化 release、无 ui-preview。ZIP 为144,147,038字节，SHA-256为`2ac357038c8fda35331798deb287b92d036dd49715a059cdacecf30657ac2618`；DMG为249,219,834字节，SHA-256为`574a8442986234c8732a77923ce5ce72d4b5514d9f11484f188d5176890a5994`。28,871项清单、29个链接和118个runtime文件逐项匹配，DMG只读挂载清单与Applications快捷方式核对后已卸载。冻结生产依赖与Node经源码manifest/lock/patch一致性核对后复用，没有把开发目录依赖作为随包依赖。

`check-package`、`check-packaged-codex`、`check-packaged-dispatches`、`check-packaged-queue`均实际退出0。新增包检查与随包Node进程的`process.version/platform/arch`、runtime/Hive包元数据精确比较通过。包内Node实际冷读今日三会话36条完整历史、原ID和冷态角色别名搜索一致，无新模型请求。

使用同一原生二进制与Resources、仅QA身份不同的隔离应用，未设置Node/runtime覆盖。窗口显示实际`8cd9c8453ff3`、Release、已跟踪文件未修改、`aarch64-apple-darwin`，后台为`v22.23.3 / 0.1.0 / 2.2.1 / darwin / arm64`。中英文搜索、窄窗20px、键盘显露和复制、外部程序页16/20px、实际结束owned Node并从窗口重启、连接/断线/重连时白名单复制均通过。主线程独立比对二进制SHA和诊断内容。正常退出码0，原子进程及重启进程均消失。未替换用户安装实例，c24069c保留可运行回退。

优化包退出后剪贴板保持**未通过验收**。一次受控观察在窗口退出附近发现剪贴板变化；源码尚未发现退出时写入，不能确定来自应用、工具时序或外部写入。unexpected正文未保存；单独跟踪[#35](https://github.com/tt-a1i/hive-desktop/issues/35)。上文调试包单次立即读取成功不覆盖这个发布包边界。证据保留于`.artifacts/release-8cd9c84/{receipt,checks,wire-receipt,dmg-receipt,native-ui-verdict,clipboard-close-observer}.json`及对应日志。

归档及未占用检查后已清理本包组装base、已替代f9f3c2b解压副本和旧DMG，合计1,156,597,057字节逻辑大小。全部ZIP、原证据/状态、当前8cd解压包及DMG、c24069c可运行回退和仍在使用的Rust缓存保留；未测量物理空间回收。操作收据为`.artifacts/release-8cd9c84/cleanup-unused.json`。

## 尚待验证

Windows About原生交互、系统输入法/辅助技术完整矩阵和正式签名/公证继续开放。新T3设置浮层尚不在8cd9c84中。Claude/Antigravity本轮不验收，Pi账号不重试，不以这些账号边界阻断其他实施。此开发包与本页的有限验收不代表完整产品已交付。
