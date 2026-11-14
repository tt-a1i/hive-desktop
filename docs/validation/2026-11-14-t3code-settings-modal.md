# T3 Code设置浮层与实际配置流程

工作项[#36](https://github.com/tt-a1i/hive-desktop/issues/36)，属于完整产品目标[#1](https://github.com/tt-a1i/hive-desktop/issues/1)。用户最新要求UI/UX先复刻T3 Code，其他产品不再作为界面参考。本批先完成设置容器与导航，完整主窗口、聊天、具体设置和引导继续实施。

## 参考与改动

固定实际安装的T3 Code 0.1.9提交[`24e77473ecf41bc1f7142529c7911c1c1aae1d76`](https://github.com/pingdotgg/t3code/tree/24e77473ecf41bc1f7142529c7911c1aae1d76)。对应设置路由、AppRouteModal、SettingsSidebarNav、SettingsGroup和dialog样式已只读查阅，源码哈希保存在`.artifacts/t3code-24e77473ecf4/source.json`。MIT许可保留于`apps/desktop/licenses/LICENSE-T3CODE`，打包脚本复制并由包检查逐字节核对。

整页设置改为窗口管理的居中圆角浮层，保留真实工作台作为背景。最大1280×960，宽窗口四周40px，小窗口横向24px/纵向32px；16px圆角，240px分组侧栏，右栏48px分类面包屑和关闭。小于768px时侧栏改56px图标，展开搜索后占用浮层内容，选择分类返回。当前只有现有十个可用Hive页面，没有用无功能页面补齐T3的二十类导航。

原设置表单、异步保存、权限、后台接口与持久化归属保留。外部程序保存结果另在浮层右栏顶部显示，不随表单滚动；仅采用实际保存响应，输入变化及新请求清除旧提示。已有同文固定反馈时隐藏重复行内Status，其他读取/选择错误及独立调用保留原反馈。首次尝试的应用内toast被浮层遮挡，已撤回，不计为通过。

设置使用同一WindowState对话栈处理嵌套确认、Esc、焦点限制与恢复。GPUI vendor增加标准AccessKit `aria_hidden(bool)`，只控制辅助技术树，不改变背景绘制或输入命中；对话框另管理实际焦点和输入隔离。子确认拥有焦点时父设置控件也隐藏。后台重启关闭对话栈后，设置会按保留的分类和表单重建，断线/重启操作留在浮层内。

## 实际原生操作

所有操作使用独立`Hive Product QA`身份和复制的数据目录`.artifacts/t3code-ui/{state,agent}`，未替换或操作用户安装实例。没有新模型推理、CLI版本/账号探测或登录；Claude/Antigravity不验，Pi不重试。

| 场景 | 已观察的结果 |
| --- | --- |
| 首次打开快捷键 | 从聊天打开后立即Cmd-F搜索diagnostics、Enter打开About；Esc先清搜索再关闭；不依赖先点击输入框 |
| 背景与子确认隔离 | 背景可见但AX不暴露工作台；Discard确认时AX只保留Keep editing/Discard，三次Tab仍在子确认循环，Esc只关子层 |
| 草稿与返回焦点 | 未保存provider草稿跨分类、搜索、关闭/重开保留；关闭设置后不点击输入框，真实按x进入原两行聊天草稿，随后精确还原 |
| 窄窗与语言 | 700×503图标导航、展开搜索和Enter进入About；英文及中文20px实际观察 |
| 真实后台恢复 | 只结束owned Node后，模态内可见断线与Restart；点击后实际版本恢复，仍在About且表单草稿保留 |
| 真实程序保存 | 从原Codex路径改为指向同一程序的隔离alias，通过真实RPC写入；主线程独立读SQLite确认codexCommand及其余字段完全符合预期 |
| 保存反馈修复 | 在Codex/Claude表单中段保存后，无需滚顶即可看到浮层内部成功提示；后台实际保存与视觉反馈分别核对 |
| 实际失败 | 输入超过4096字符后，真实后台返回校验错误；固定红色反馈可见、原current路径保持、保存可重试，独立SQLite确认全字段仍为baseline；没有执行CLI |
| 原值恢复 | UI恢复原路径，切页/关闭重开一致；编辑恢复时旧错误消失、保存禁用；各轮正常退出后的独立SQLite全字段均匹配baseline |

精确保存基线及每次检查点保存在`.artifacts/t3code-ui/program-*.json`。五轮原生进程均正常退出0、owned Node及重启进程消失，记录位于`.artifacts/product-completeness-20261114/t3-settings-*-process.json`。最终固定反馈版PID33294/Node33314均退出，冷读匹配原值。此轮发现重复AX Status后已做条件去重并构建通过，实际single Status及英文窄窗反馈留待优化包复验。

另一个输入观察是CUA按x后Cmd-Z没有撤销，不打开设置的对照也相同。静态检查未发现设置切换重建输入或清除撤销历史，无法据此确定产品原因。已用Backspace精确恢复原草稿，不以盲改焦点或历史掩盖不确定性。

## 检查与独立复核

- 默认Mac构建、MSVC交叉检查及diff检查退出0。
- Rust主程序测试实际44通过、0失败、1忽略；不将这些单元测试说成模态交互覆盖。
- 1374个唯一英文键，直接中文`tr()`字面量无遗漏；两份打包脚本Node语法检查通过。
- Vendor新增测试只覆盖AccessKit元数据，不是GPUI渲染/焦点/平台适配测试；现有vendor harness缺上游依赖和字体，退出101前未执行，不记为通过。客户端实际编译和原生AX证据分别记录，未为运行该测试扩充生产依赖。
- 四个独立角度复核：架构A、边界B、验证A−、范围B+。审查发现的初开Cmd-F、子确认父AX泄漏、模态内重启、Pi登录后返回未关闭外层已修复；前面三项完成实际原生复验。Pi返回仅源码/编译核对，没有登录。

## 实际优化开发包

本批已进入Mac current/candidate `0e478c2627ba015ff9c529f16028dadec7636d72`，默认优化release、无ui-preview。ZIP144,154,624字节，SHA-256为`2f556693bf28d555f930715263baa8a715593fafa7e5b40e688061cdfc864276`；DMG226,431,967字节，SHA-256为`d1decba4bed3aabebe6a18487f9b3ec887400bb35b6cdd2a717ec04272891d46`。28,872项清单/29链接逐项匹配，DMG只读挂载清单及Applications快捷方式核对后卸载。相较8cd9c84仅原生二进制、T3许可和vendor变更说明三项不同；118份runtime文件、Node和冻结生产依赖完全保持。

四组正式包检查实际退出0，包含随包T3许可精确比对。包内Node冷读今日真实三会话36条完整历史、ID和冷态角色搜索一致；英文实际原生启动能恢复缺失的会话选择。没有新模型请求。

同一优化二进制和Resources、仅QA身份不同、无Node/runtime覆盖的实际窗口，完成搜索/Enter/Esc、嵌套AX/Tab隔离和草稿抽查。英文700×503/20px下，保存成功与实际4096字符校验失败都可读且AX同文Status均为1；主线程独立SQLite确认alias保存、失败不写、最终全字段还原。主线程另比对实际运行二进制SHA和包内Node路径。真实结束owned Node43470后，在浮层内重启为47184，关于页实际显示0e478c2627ba/优化版及Node22.23.3/Hive2.2.1，原表单和聊天草稿保留；正常退出0，两Node均消失，最终冷读匹配baseline。主线程只读截图/AX核对了中文20px窄窗的真实模态和背景隔离。

证据位于`.artifacts/release-0e478c2/{receipt,checks,wire-receipt,dmg-receipt,native-payload-identity,native-ui-verdict,program-*,sidecar-stop}.json`及实际日志。开发包已推广current，未替换用户安装实例，8cd9c84保留可运行回退。

## 尚未交付的范围

当前是遮罩/阴影，尚无T3的真实背景模糊。新模态Windows原生、完整辅助技术/输入法矩阵、后续聊天/输入区及所有页面的完整复刻继续；复制诊断的退出附近剪贴板边界仍为[#35](https://github.com/tt-a1i/hive-desktop/issues/35)，本批未重复退出剪贴板实验。不将这批有限原生/包内验收当成完整T3复刻或完整产品验收。
