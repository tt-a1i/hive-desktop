# 限制图片预览的并发解码

2026-11-12，基于1274d11。重开含多张图片的历史时，原实现为每条消息提交独立blocking解码任务；单图已有64MiB分配限制，但多张会同时分配大像素缓冲。实际16张2400×2400合成PNG历史的Mac debug窗口启动采样，原生父进程峰值RSS约509MiB。

## 实现

草稿导入、草稿恢复和历史预览改走同一个prepare_preview入口，用已安装smol::lock::Semaphore最多同时解码两张。先异步等待许可再提交blocking工作，等待者不占blocking线程；许可移动到真正的blocking闭包，调用者离开会话也不会提前释放仍在工作的解码名额。复用原preview/decode_image，原格式、尺寸/分配限制、144×112缩略图、错误传播及原图字节保持。没有新依赖、缓存、store、协议或权限。

查看完整原图继续走原独立入口，不排在整段历史的缩略图队列后。此改动降低同时分配，不宣称缩短所有缩略图准备时间或取消已经排队的旧会话工作。

## 实测及范围

`.artifacts/image-preview-pressure-20261112/seed.mjs`用原生SessionManager准备16条user图片消息，图片为本地生成的均匀RGBA PNG，不是外部照片或模型结果。真实Hive工作区禁用autostart，无Worker；before/after使用同一原始历史、两个独立数据副本、同一Mac debug构建方式和空Pi配置。仅打开恢复，不发送消息。run.py每约50ms通过ps采样原生父进程RSS，不含Node子进程/GPU总占用。

| 范围 | 修改前RSS峰值MiB | 修改后RSS峰值MiB |
| --- | ---: | ---: |
| 共同启动前5秒 | 506.81 | 195.02 |
| 各自完整运行 | 508.89 | 196.17 |

前窗口采样约11.9秒，后窗口约43.6秒；因此主要对照固定前5秒，完整运行数值只作旁证。每版只有一次启动，不声称统计显著、所有格式/图片历史或正式release均有同等改进，也没有加载速度数据。后窗口末尾13/14/15图片已成为可查看缩略图，两窗口均Cmd+Q exit0；没有为了降内存丢弃历史图片。

`check.py`逐条核对原生16个消息ID、文本、图片字节与原始PNG完全相同，before/after两个Hive数据均workspaces1、workers/messages/dispatches/dispatch_messages/agent_runs全0。收据、原始RSS样本、AX、native日志、准备及断言式脚本均保留；检查覆盖数据与可见末尾缩略图，未声称逐一原生打开16张图。

Mac `cargo build --locked --manifest-path apps/desktop/Cargo.toml --bin hive-desktop`、Windows `cargo check --locked --manifest-path apps/desktop/Cargo.toml --target x86_64-pc-windows-msvc`及`git diff --check`通过。纯前端调度改动，无正式测试增改删，按风险做构建和真实原生/数据检查，未跑无关全量suite。没有新模型请求或账号操作。

其他格式/坏图、草稿导入与恢复的完整原生矩阵、原图并发查看、切换/退出时序、Windows真机及release内存仍待。当前d644db9安装包尚未含本次调度及上一批汇报布局，完整产品目标继续。
