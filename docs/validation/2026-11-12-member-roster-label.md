# 成员列表刷新与历史会话标签

后续已恢复原生工具并补失败/延迟/删除后标签检查，另发现并修复成员页读取失败的空团队误导；实际范围和未覆盖的活动Worker场景见[原生复验](2026-11-12-member-roster-native.md)。下文保留本次源码修复时的历史验证边界。

基于2c5a4a6。前轮293afe3安装包真实Terra一对二协作时，协作概览短暂显示正在工作的proof-alpha/beta为Historical conversation，随后恢复。原始观察及限制见2026-11-10-installed-293afe3-team.md，不视为已完成动态UI验收。

源码链路：reload_collaboration立即请求任务与派单，但不请求成员；start_workspace_updates仅在协作/成员页显示时每三秒刷新成员。会话列表先收到新成员，agent_name在旧成员列表未命中时从会话标题回退，并无条件追加历史标签。因此“缓存里暂时没有”被误当成“已经不是成员”。

修正共五行：打开/显式刷新协作页时复用refresh_members（原有member_loading去重仍生效）；共享agent_name回退在成员正在加载、读取失败或关联会话仍busy时只显示会话名称，不推断历史身份。读取成功且非busy的原有历史标签保留。影响派单卡、详情、消息来源和工作流中的同一名字读取，不加定时器、请求接口、字段或依赖，不改历史数据或删除成员流程。

实际通过：cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop；cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc；git diff --check。日志.artifacts/member-roster-label。纯UI，无新增/修改测试或全量suite。

原生界面控制工具当前不可用，本轮未运行窗口、未做模型请求，不能将构建通过写成原生复验。新成员加载/错误/忙碌及已删除成员的实际标签矩阵仍待复验；293afe3安装包/current/candidate未更新，完整目标继续。
