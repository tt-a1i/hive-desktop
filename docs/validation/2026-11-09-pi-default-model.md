# 内置Pi默认模型优先级核对

2026-11-09，源码基线c918ae0；本轮不改生产模型选择逻辑。

## 发现与判定

原始补丁UI夹具设置defaultProvider=openai/defaultModel=gpt-6.1-terra，但聊天栏显示其他模型。独立读取隔离目录的SettingsManager与ModelRuntime确认：目录匹配、默认字段正确；该夹具没有注册请求的Terra定义，也没有对应认证。查Pi SDK原生findInitialModel：默认项必须存在且可用，否则进入原生候选选择。该现象不足以证明桌面丢失默认配置，没有为错误夹具给产品增加fallback或强制选模。

只检查模型/认证是否存在的布尔元数据，没有打印认证值或修改用户全局配置。原夹具仅用于本地工具/历史显示，没有模型推理；其聊天栏元数据不当作Terra实际推理证据。

## 新定向检查

`packages/runtime/checks/pi-default-model.ts`使用真实desktop runtime、Hive HTTP/SQLite、Pi SDK和两份隔离提供商定义，模型ID统一gpt-6.1-terra；地址为拒绝请求的owned loopback，凭据为仅此夹具的占位字符串，不访问个人认证。实际断言：

- 新聊天读取全局默认；新建项目配置后，后续聊天采用项目默认。
- 聊天内明确选模不写回全局或项目配置，后续聊天仍遵守项目默认；两个设置文件逐字保持。
- 完整runtime重开后，先前聊天保存的选择优先于新默认，pending0。
- 删除保存选择的提供商后，旧聊天model为空且unavailableModel保持原provider/id；其他聊天仍用有效项目模型，不静默覆盖旧选择。
- owned端点请求数严格为0，正常关闭runtime与server并删除临时目录。

源码检查已通过，注册`check:pi-default-model`，并纳入`check:pi-models`和包内模型检查。没有根据测试改动生产语义。

## 候选包补验

独立解压原canonical候选1534986（ZIP SHA256 e353c74c0a0343f651f41bfe039b47f9c644872f54888e7612277d54d0190529），运行更新的`node scripts/check-packaged-models.mjs --bundle …`。已有JSONC/override/wire/warm-reload及新增defaults全部exit0；新增检查的runtime依赖绑定解压包dist，用包内Node执行，不以源码tsx结果替代。该命令从当前仓库读取检查脚本，不声称旧ZIP带有后来新增的开发检查文件。

`pnpm check`、脚本语法及git diff --check通过；无生产TS/Rust改动，不重编译或重复全套。证据`.artifacts/pi-default-audit-c918ae0/`包含只读核对receipt、源码/包检查日志及package-receipt；lsof无占用后移除解压副本，canonical ZIP/current均未替换，候选收据追加本轮模型gate。Windows原生、实际重新登录后的Pi推理及完整默认配置UI仍按原缺口继续。
