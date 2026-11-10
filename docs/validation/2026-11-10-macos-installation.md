# Mac 应用身份、DMG 与实际安装

源码 `db6fb209e5d4dda4a3b4cc4d5876e3aa020be8db`，Mac ARM64 优化 release；纳入此前 `fbf7d5c` 的欢迎账号反馈与严格/脱敏凭据读取修复。

## 用户可见变化

原先 ZIP 中应用没有图标和完整系统版本信息，安装依赖用户自行找出 `.app`。现在使用已捆绑 Hive 2.2.1 的原有 logo 生成原生 ICNS，写入系统应用名、开发版本、开发工具分类及真实 Mach-O 最低部署版本；新增只读压缩 DMG，内含应用及指向 `/Applications` 的快捷入口。保留原标识 `com.hive.desktop.dev`，不加入新图形依赖。

logo 原文件 SHA256 `530b2d330a451f93ddc13b5921a4f33bed8887fe1ffa01ac2b088671a93cd047`，512px 原始上限，未伪造1024px细节；包内 `hive-icon-source.txt` 指向实际随包保留的上游 NOTICE、LICENSE.BSL 与 TRADEMARK.md。最低版本11.0来自二进制 LC_BUILD_VERSION，不表示已实测 macOS 11。

## 实际安装验证

证据位于 `.artifacts/release-db6fb20/`。

1. 与上一3339ce1生产依赖、锁文件、package元数据和全部归档条目比对，只改原生程序、pi-auth.js/map、Info.plist，并新增图标/来源说明。112个runtime文件与生产dist相同；28,865条文件/模式/哈希与29条内部链接在ZIP解压后逐一匹配。
2. `package-macos-dmg.mjs --app … --output …` 实际生成UDZO映像，`hdiutil verify` 通过；以readonly/nobrowse挂载，映像中的全部28,865条应用内容与清单相同，Applications链接实际指向 `/Applications`。
3. 安装前确认 Applications 两种位置均没有Hive Desktop，默认桌面SQLite工作区/消息/派单均0。用Finder真实复制至 `/Applications/Hive Desktop.app`，没有覆盖既有应用。复制未完成时第一次清单校验只有20,909项，按未完成处理；复制完成后再次校验28,865项全部匹配，不以目录已存在代替安装完成。
4. Finder Get Info真实显示Hive图标、Apple芯片应用、版本 `Hive Desktop 0.1.0 development build`，安装体积457,919,035字节（磁盘约538.8MB）。见 `finder-installed-info.png` / `.ax.txt`。
5. 从Finder打开安装应用；实际原生PID68670，子进程77843的Node与runtime均来自 `/Applications/Hive Desktop.app/Contents/Resources`。没有源码路径环境覆盖。欢迎页由后台加载完后可操作；⌘,进入设置、Escape返回欢迎，账号卡显示实际保存元数据；未进行登录或修改凭据，未启动Agent或发起推理。见 `installed-welcome.png` / `.ax.txt`、`installed-settings.ax.txt`。
6. ⌘Q后系统应用列表显示isRunning=false，原生与Node PID均消失；默认SQLite工作区/消息/派单仍0。Finder启动没有可取exitCode，不写成exit0。映像已经正常卸载，应用保留在Applications供继续使用。

## 包检查

包内Node执行六组 `scripts/check-{package,packaged-models,packaged-dispatches,packaged-codex,packaged-remote,packaged-claude}.mjs --bundle …` 全部exit0。新增Mac身份检查使用系统plutil和iconutil真实解码512PNG；旧3339ce1包在缺少版本字段处先失败，证据 `.artifacts/macos-identity-before.log`。后台检查继续穿透真实PTY、HTTP/SQLite、模型配置、派单结算/队列、远程链路及原始厂商协议测试对端。

额外将生产 `packages/runtime/checks/pi-auth-wire.ts` 编译后置于临时目录，使用包内Node/runtime执行：真实JSONL凭据损坏→固定安全错误、无secret片段、文件未被改写→恢复文件→重新读取成功，并保留原生配置/取消/移除与默认值检查，全部通过（`auth-wire.log`）。本轮无模型请求；上版真实Codex Terra Medium成员80/4生命周期证据沿用[原记录](2026-11-10-member-lifecycle-package.md)，不冒称重跑。

## 当前产物与边界

- current/candidate ZIP同字节硬链接：143,751,899字节，SHA256 `e80ae94c7aaef85347bc1f2ed2fc78cc8adbccc22b87b9b3cf5c2472844d60d9`。
- DMG：`.artifacts/release-db6fb20/Hive-Desktop-macos-arm64.dmg`，234,078,454字节，SHA256 `0c0f0b7b84b38c5173ec36b6bd47899e6840bb878c1cc09a54d412b70ba98eee`。
- 原生程序SHA256 `aba3d943fb1507d5cc4f221660ee45967b5c1e3232942958f90e081ec7402e8c`。
- 元数据、图标与DMG脚本均通过Node语法检查、diff检查；无Rust/Windows业务代码变动，不重复无关全量suite。

这是未签名、未公证的本地开发分发包。未验证下载隔离标记下的Gatekeeper、正式签名/公证、自动更新或Windows原生安装；没有删除quarantine或绕过系统保护。DMG提供标准应用和Applications入口，尚无定制安装背景/图标布局。完整功能与UIUX验收仍按总目标推进。

## 存储清理

推广新包前后核验归档SHA256和lsof无打开引用，删除本轮重复base（538,775,552磁盘字节）及已经落后两版的ecad294解压bundle（538,427,392磁盘字节），合计1,077,202,944字节，约1.00GiB。其ZIP/收据、历史测试状态和旧聊天工作目录保留；上一3339ce1可运行解压应用保留。清单见 `cleanup.json`、`superseded-cleanup.json`。安装到Applications的应用供后续真实使用，不当作可随意清理的缓存。
