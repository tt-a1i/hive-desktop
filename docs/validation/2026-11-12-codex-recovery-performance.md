# Codex长历史恢复查找成本

2026-11-12，基于3a35e07。聊天已有可变高度虚拟列表，本次沿真实恢复路径发现CodexProjection.recover为每个原生回合对全部显示记录filter。回合数和显示记录一起增长时重复扫描；这是恢复期间的后台成本，与Markdown可见行渲染分开。

## 变更

先用局部Map按turnId收集原显示记录，再按原生回合顺序读取。仅增加6行净代码，没有持久缓存、新抽象、依赖或协议。单个回合内部顺序、回合排序、缺口说明、legacy导入标记、未完成工具的原生终态替换及中断标记均沿用原逻辑。临时Map保留原对象引用及分组数组，生命周期仅限本次recover；不复制原始消息正文。

## 定量检查

`.artifacts/codex-recovery-performance-20261112/check.mjs`使用实际生产类和文件系统，准备完整历史后计时recover，包含序列化和原子文件替换，不包含constructor/原生thread读取/GPUI。每规模4次，分别记录全部样本，中位数为中间两个样本均值。

| 合成回合/记录 | 修改前中位ms | 修改后中位ms | 相对倍数 |
| --- | ---: | ---: | ---: |
| 1,000 / 2,000 | 10.930 | 3.621 | 3.02 |
| 5,000 / 10,000 | 312.099 | 68.009 | 4.59 |

两规模持久化JSON的SHA256修改前后分别精确一致，并在每次写后重新创建真实类断言全部记录。数据为合成普通文本，仅同机Node本地采样；不是用户真实5,000轮、整体开会话耗时、帧率或所有后端性能结论。保留completedTurns的原有数组语义，因此也不声称整个恢复已变为严格线性算法。初次日志采用上中位，计算定义已统一改为两中间样本均值，原采样数值没有重跑或丢弃。

## 正确性与真实会话

`parity.mjs`用git固定3a35e07版本与当前实现直接对照，8组组合（原生/legacy × completed/interrupted/failed/inProgress），总32次recover调用。含交错存储回合、缓存工具与原生完成态差异、新回合导入、缺口提示、重复恢复及冷读；逐对象比较旧/新首次及重复写盘结果和重新打开结果，不只比较长度。输入夹具保持原值。

以下现有检查各exit0：

- `pnpm check`、`pnpm build:runtime`。
- `pnpm --filter @hive-desktop/runtime exec tsx checks/codex-session-normalization.ts`。
- 同命令前缀下的`codex-paginated-history.ts`、`codex-session-failures.ts`、`codex-interrupted-tools.ts`、`codex-exit-cleanup.ts`。

这些包含真实Hive HTTP/SQLite及显示journal/退出/队列恢复，Codex协议对端为本地合成进程，不冒称厂商推理。另以Node22运行当前dist的`cold-team.mjs`，只读打开此前645c9a5真实Terra协作的三会话，核对原模型、完成投递/汇报来源、pending=0及pendingReports空；没有发prompt或resume执行，原推理证据仍归原提交。

原始日志、四次样本、旧实现、可运行差分程序、三会话快照和receipt均保留在上述目录。无正式测试增改删，无新模型或面板操作。按修改风险运行五个定向检查，没有跑无关全量suite。当前3ed07c9安装包尚未含本次优化及上一批协作复制入口；Windows实际性能、图片/大表格/长工具记录、完整可访问性和产品目标继续开放。
