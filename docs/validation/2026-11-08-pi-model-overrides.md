# Pi 模型覆盖配置

任务：[#5](https://github.com/tt-a1i/hive-desktop/issues/5)。范围为模型配置页的实际配置完整度，完整产品目标继续实施。

## 行为

配置页现在同时枚举 models 数组和 modelOverrides 的模型 ID，同名项只显示一次。只有 modelOverrides 的项标明“模型覆盖”，可编辑公共字段或移除；不会把覆盖项误写成新的自定义模型。

覆盖项的思考与图片能力明确区分“继承内置 / 支持 / 不支持”，继承对应配置字段缺失与 RPC null，不把未配置的能力误显示为 false。恢复继承会删除对应覆盖字段，实际能力仍由 Pi 原生目录与配置决定；列表不冒充有效模型目录，未匹配的配置同样可清除。普通自定义模型仍沿用开关。

保存复用现有 JSONC 局部修改、原 revision 冲突检查与原子替换，保留 headers、sampling、其他模型、提供商和凭据。删除最后一条覆盖时，如果提供商已无其他有效配置，会同时移除这个空提供商覆盖；已有接口、凭据、其他覆盖或自定义模型的提供商保留。Pi 1.4.0 会拒绝只有空映射的提供商，此路径由真实 RPC 发现并修复，没有绕过原生校验。

三态与覆盖类型随现有本地表单草稿保存、跨页与重启恢复。旧草稿的布尔值仍可反序列化，新增类型字段默认 false。不新增依赖、权限流程或全局模型默认修改。

## 验证

- `pnpm --filter @hive-desktop/runtime check:pi-models`：已有实际 SDK/JSONC/只读文件/高级字段/冲突/符号链接/孤立凭据检查、常驻会话刷新通过；新增 override-only 枚举/编辑/恢复继承/删除、未知条目清除与生产 JSONL 检查通过。
- 新 JSONL 检查实际启动 Hive HTTP/SQLite，经既有有认证代理读取工作区，再通过生产配置 RPC 保存 null/false、保留高级字段、拒绝过期版本、移除最后条目及正常关闭。直接未认证访问返回 403；检查改走既有私有代理，没有改生产授权。
- SDK 检查使用真实安装目录的静态模型元数据验证继承与覆盖，不执行模型；该固定 SDK 的静态目录没有 gpt-6.1-terra。实际 RPC 和原生窗口的表单 ID 使用 gpt-6.1-terra，不声称其在静态目录中可用。全程没有厂商请求、模型执行或真实用户凭据，也不执行配置中的凭据命令。
- 上一 Mac 模型草稿包被新增实际 override 枚举断言拒绝（旧模块返回 0 项）；不是源码字符串断言。收据 `.artifacts/pi-model-overrides/before.log`。
- 浅色 760 宽窗口及深色常规窗口，实际 GPUI 布局/按钮事件与真实 runtime/config 检查均通过：覆盖项初始继承、改名/数字/能力、切页保留、正常退出、新进程恢复、保存仅改覆盖、恢复继承、删除确认取消/确认、其他覆盖保留和最后空提供商清除。共 6 个进程正常零退出，截图和收据在 `.artifacts/pi-model-overrides/`。
- `check:pi-auth`、`tsx checks/team-runtime.ts` 定向回归通过；TypeScript check、Mac cargo check/build 和现有模型草稿 3 项检查通过。草稿样本改为实际覆盖项的 null/false，用于验证三态真实文件往返。
- 最后重新构建生产源码，替换验证专用二进制。

## 边界

GPUI 检查是复制当前源码的显式帧驱动，没有冒认系统输入法/物理鼠标验收。新 Mac 开发 ZIP 与尚未完成的系统操作见下文；构建和包内行为检查不代替这部分验收。Windows 原生与 ACL、完整配置的高级字段图形编辑、正式发行继续开放。依用户要求不处理 CI。

本次未改 Hive 协作协议或 schema，不跑无关全量 runtime 测试。配置服务及桌面字段有改动，因此验证集中于实际 SDK、生产 JSONL、原生窗口、草稿与团队回归。


## Mac 开发包与系统操作进度

开发 ZIP：`.artifacts/hive-desktop-macos-arm64-model-overrides.zip`，源 `e6a48ca54527b7c98f9ff398a5bae891cf9ad991`（实现 f21d0f9 加包内检查）。ZIP SHA256：`9b793cdeb719713a7d9992dc3d7b9cadaf46754ae7a185ea3653ffeb614bf9b2`；程序 SHA256：`3a4dd4644c4c3bb37537cb488f69aa250de2bf78e2d9ad8f2048ebdf9eaf3c66`。仍未签名/未公证，Windows 未验收。

复用已核验的同平台冻结依赖包前检查 lockfile/patch/workspace 与依赖元数据，完整刷新 runtime 与原生程序。新 ZIP 独立解压后全部 106 个 runtime 文件和程序与本轮构建匹配；相对前包只有 pi-model-config.js 及 map 改变。

新增 `scripts/check-packaged-models.mjs --bundle <解压包>`，把既有四项实际 SDK/JSONC/覆盖配置/生产 JSONL/常驻刷新检查绑定到包内模块、SDK 和 Node；wire 子进程从包内 runtime 目录启动 dist/main.js。旧 05e0dbf 包通过既有配置检查，但被覆盖枚举的 0 != 2 断言拒绝；新包四项均通过。真实 PTY/Hive HTTP/SQLite smoke 与生产窗口启动恢复也通过。无厂商推理或用户认证变更。

系统界面从新包打开隔离配置，进入模型覆盖编辑页，确认原生按钮的“思考能力：继承内置”和“图片输入：继承内置”均选中、保存按钮禁用。开始输入时系统报告 Mac 锁屏且无法自动解锁，本轮没有完成编辑、退出重开、保存、恢复继承或删除的系统操作。已向用户请求解锁，保留这一个 owned 应用及其独立状态等待接续，不因观察超时重启。原生 GPUI 前轮检查已覆盖这些功能，不能混作本次系统操作完成。

收据、逐文件哈希、旧包负向与新包检查日志：`.artifacts/pi-model-overrides-package/`。以上是锁屏时的历史状态；后续系统操作已完成，见下文。完整产品目标继续开放。


## 解锁后的实际系统操作

用户解锁后，使用系统 CUA 实际编辑、退出并以新进程恢复覆盖草稿，保存后磁盘仅写 modelOverrides，没有新增 models；恢复两项能力的“继承内置”后对应字段被删除。删除确认的取消保留条目，确认清除最后覆盖及空提供商，完全退出重开仍无覆盖或遗留草稿。未执行模型或修改真实用户认证。

同机多个开发包使用同一应用标识，曾出现辅助功能树与可见窗口不一致，导致一次“设置页盖住确认框”的判断；该判断已撤回，没有据此修改产品代码。为隔离窗口身份，复制已解压开发包，仅修改 Info.plist 的应用名称与标识为 Hive Override Check / com.hive.desktop.overridecheck.e6a48ca，程序及全部 106 个 runtime 文件哈希保持一致，原 ZIP 未变。独立副本的确认框正常显示并可取消。该验证不代表原标识多实例的窗口定位已验收。

独立副本三个进程 32348、36890、37089 均通过系统退出正常结束；原始验证进程 3265 正常退出，定位混淆的 owned 进程 9754 使用 SIGTERM 结束，其 driver 非零退出，不计入正常退出证据。其他已有应用未终止。完整收据：.artifacts/pi-model-overrides-package/physical-ui-receipt.json，日志 physical-unique*.log。

本票据的模型覆盖行为与 Mac 开发包系统操作已完成。Windows 原生、正式签名公证与完整桌面目标仍由 #1 持续推进；本包不包含之后的排队输入功能。
