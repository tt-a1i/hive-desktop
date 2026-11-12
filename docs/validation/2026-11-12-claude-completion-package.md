# Claude 回执优化的开发包验证

2026-11-12，源码 `7b7940ce3f2f0966f6357a37d61dc0f7b951bb18`，包含 d43f55e 的 [Claude UUID 批量核对](2026-11-12-claude-completion-uuid.md)。current/candidate ZIP、DMG 和 `/Applications/Hive Desktop.app` 已更新。本批无新厂商推理或面板操作。

## 包与安装

相对 78fef23，仅更新 `agents/claude.js`、`claude.js.map`、`claude-projection.js`、`claude-projection.js.map`。TS 配置不生成声明文件，初次组装对多一份 d.ts 的预期被差分闸门拒绝；核对配置和完整实际差异后，从已核验的自有 staging 继续，保留失败及继续日志，没有改变生产源码来迎合打包预期。

原生源码、锁文件、依赖补丁和 package metadata 无变化，复用 3a18b94 冻结生产依赖及原 Node、应用身份。116 个 dist 文件全部与当前源码构建相同；ZIP 解压后的 28,869 项文件/权限、29 个内部链接与组装清单一致。原生程序 SHA-256 保持 `d9572c345976ebc0855f6ca5e7b24235b1c9da8f308f87790b61eae6dfcbcf6a`。

新 ZIP 为 143,936,343 bytes，SHA-256：

```text
331196b5db09932eaacb4f785b8cafcb7fae840b8b0fd51d25de91967245ed51
```

DMG 通过 hdiutil verify，随后只读挂载核对完整应用清单及 Applications 快捷链接并卸载。安装前核对旧完整清单和 lsof 无占用，经 staging 与备份更换 Applications，安装后完整清单相同。current/candidate 是同字节硬链接，仍为未签名、未公证开发包，不是正式发行。

## 包内实际验证

使用解压包内 Node 22.23.3、实际包内 dist / 生产依赖运行：

- **Claude Projection**：2,000 / 10,000 行合成原生形状历史及 10 个替换、合并、clear、流式别名、嵌套工具阶段；完整显示快照哈希及各阶段完整 JSON 与已验证源码一致。
- **Claude 官方 SDK / 受控 CLI**：1,000 行历史、500 条完成回执和两个缺失 UUID 记录，过滤、顺序、新回执只增加一次及完整冷恢复通过。另一组保留同样历史而完成标记为空，未凭成功文本猜完成；新输入后只有新 ID，冷读保持。明确没有 Claude 厂商调用，夹具里的 Terra 字符串是合成元数据。
- **最新真实 Terra 历史**：只读打开 [d43f55e 真实主控与两成员](2026-11-12-latest-terra-team.md)，由包内正式 main.js 经 JSONL 读取，40 条完整历史、27 条工具结果、消息 IDs、模型和待处理列表与真实运行结束时深比较一致，pending 0 / busy false，正式后台 exit 0。
- **桌面消息格式**：上述三条实际 stdout 帧通过与原生同版 serde_json 1.0.154 解析和模型、历史、完成回执、状态断言。是消息边界验证，没有扩大为 GPUI 渲染验收。

本轮包内恢复没有 prompt / Hive resume，也未重新运行真实模型驱动。最新真实推理仍指向 d43f55e，一次输入和三条原生 Terra / medium 回合；不把包内冷读当作再次推理。

四组包检查全部 exit 0：

```sh
node scripts/check-package.mjs --bundle .artifacts/release-7b7940c/extracted/bundle
node scripts/check-packaged-codex.mjs --bundle .artifacts/release-7b7940c/extracted/bundle
node scripts/check-packaged-dispatches.mjs --bundle .artifacts/release-7b7940c/extracted/bundle
node scripts/check-packaged-queue.mjs --bundle .artifacts/release-7b7940c/extracted/bundle
```

另通过 `pnpm check`、`pnpm build:runtime`、`git diff --check`。无新增、修改或删除正式测试；只刷新两个后台模块，采用相关完整差分、官方 SDK / 文件恢复与包回归，没有跑全量测试。未改原生程序，本轮未重复启动窗口或 UI 走查；Windows 原生、通知送达/点击、Pi 账号及 Claude 真实推理边界保持开放，完整产品目标尚未完成。

## 收据与存储

`.artifacts/release-7b7940c/` 保留 `assembly.json`、`manifest.json`、`dependency-diff.json`、`actual-gates.json`、`backend-receipt.json`、Projection/SDK 收据、`wire-receipt.json` / 原始帧 / Rust 日志、`dmg-receipt.json`、`installed-manifest-receipt.json` 与 `receipt.json`。生产依赖树没有新部署副本。

清理前先核对清单、回退 ZIP 哈希与无占用；仅删除本批重复 base 和旧版本临时安装备份。78fef23 解压应用、归档 ZIP、真实协作数据及所有验证收据保留供回退/冷读；本次删除 916,553,282 逻辑字节（约 874 MiB），收据见 `cleanup.json`，不将逻辑字节量等同于 APFS 实际释放空间。
