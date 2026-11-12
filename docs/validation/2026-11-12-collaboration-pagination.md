# 协作列表的整页分页边界

2026-11-12，基于e03c5b5。协作主页每页20条，原来请求20条并在返回数量达到20时启用下一页；当总数恰好20、40等整页数量时，会出现可点击但没有内容的下一页。

现复用原有limit/offset接口：初次加载和自动刷新均请求PAGE+1（21条），只渲染前PAGE（20条），存在多出的一条才启用下一页。页码及翻页步长统一引用PAGE。没有新增has_more字段、后台接口、store或游标协议；继续保留100,000的offset上限、筛选与旧请求序号保护。

## 主要通过代码验证

`.artifacts/collaboration-pagination-20261112/check.mjs`使用真实Hive HTTP/SQLite。通过真实后台创建隔离项目和停止状态成员后，仅在自有SQLite准备queued派单夹具，不启动Agent或模型。该检查证明读取/分页契约，不冒称这些fixture任务由模型真实派发。

- 20条：旧limit=20返回20条，offset=20实际返回空；新limit=21仍只返回20条，可明确没有下一页。
- 21条：新首批返回21，前20条加offset=20的1条完整覆盖，身份不重复。
- 40条：首批21，offset=20返回20，offset=40为空；两页展示项按完整sequence ASC身份序列覆盖40条，无重复/遗漏。
- reported筛选为空，不把其他状态计入下一页。

初次fixture错误地假设顺序为倒序；核对拥有实现DispatchLedgerStore的sequence ASC契约后，将检查改为断言完整升序ID序列。没有为检查改变产品排序。此准备错误说明保留于initial-fixture-ordering-note.txt。

## 一次原生流程抽查

源码独立App/data/agent，初始化20条queued记录。真实窗口的完整AX显示20张卡且Next page禁用。随后只在自有DB追加第21条，未手动刷新：既有自动更新使Next page启用，第一页仍仅20张卡；显露并点击下一页后，仅显示任务21，Previous启用/Next禁用。正常Cmd+Q退出0。完整AX和receipt保留在上述目录。

首次点击离屏分页按钮没有翻页，随后显露按钮并点击才成功；系统滚轮方向经过实测校准。期间ScreenCaptureKit捕捉错误在重读同一窗口后恢复，没有重启应用或修改生产滚轮语义，不把这些工具限制当应用故障。

通过Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows MSVC目标`cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`及`git diff --check`。编写过程中缺少PAGE导入和一处括号错误均由编译器发现并修正，原失败日志保留，最终两个目标通过。纯前端读取/展示修复，无正式测试增改删、无无关全量suite，无模型请求。40条的窗口翻页、所有筛选/在途变更及Windows原生未扩张为已验收。
