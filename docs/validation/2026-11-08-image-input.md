# 图片粘贴、拖放与原图查看

日期：2026-11-08。前置用户消息与恢复链路见 [图片附件](2026-11-08-image-attachments.md)。本轮继续桌面体验，没有改变 Hive 协作协议、数据库 schema 或审批规则。

## 使用场景和变化

用户截取屏幕后，希望在聊天框直接粘贴，或从文件管理器拖入图片；发送后需要看清截图细节。原先必须先保存文件再点选择器，历史只能看缩略图。现在剪贴板图片/复制的图片文件及拖入的本地图片均进入现有附件草稿，缩略图可打开原图查看器。不做这些，截图输入和核对仍需频繁切换应用。

## 实现

- Textarea 使用 gpui-kit 原生 on_paste，纯文本继续交给原输入组件，文件附带的路径字符串不混入消息正文；图片与 ExternalPaths 进入和选择器相同的异步导入队列。文件拖放在聊天区域接收 GPUI 原生 FileDrop。未连接、移除项目、导入中或执行中的会话拒绝添加，并保留原草稿。
- 新增私有 JSONL `chat.attachments.clipboard`，使用现有 promptImages 校验和 Hive upload 存储。PNG/JPEG/GIF/WebP 剪贴板字节不转换；BMP/TIFF 先用现有 image 库解码成 PNG，以兼容 Windows DIB 和部分 macOS 图片源。不向磁盘写剪贴板临时文件。
- 解码沿用宽高各 8000、分配上限 64 MiB；BMP/TIFF 压缩/原始输入额外上限 64 MiB。转换结果仍须满足编码后每张 5 MB、合计 20 MB、最多 10 张。上述是单张/单次限制，不是整个应用或长图片历史的内存承诺。
- 草稿及历史同时保留缩略图与共享的压缩原始图片。查看器异步解码首帧，提供适应窗口、按屏幕物理像素 1:1 的原尺寸及滚动、复制原图、另存为。GIF 不播放动画，复制/保存仍使用原始字节。原图关闭后释放查看器的大尺寸渲染，压缩来源仍随对应消息/草稿保留。
- 另存为使用原生路径对话框和同目录临时文件原子替换；失败保留原文件、清理临时文件并显示错误。该写入检查与操作系统保存对话框的真机验收分开。
- 图片单独消息的空文字复制按钮禁用；pending 文案改为“待处理消息”，不将用户图片也称作团队汇报。

## 复现并修复的 GPUI 问题

图片 Cmd-V 后直接注入真实 GPUI FileDrop::Entered/Submit，没有额外普通 MouseMove，原有 on_drop 完全不触发，草稿仍只有一项。源码确认 Window::dispatch_event 在事件转换前只将普通 MouseMove/MouseDown 归为鼠标模式；FileDrop 虽转换成鼠标事件，仍保留 Keyboard 模式。HitboxId::is_hovered 在该模式返回 false，拖放高亮和回调均被忽略。

将 FileDrop 归入鼠标模式后，同一流程立即得到第二项上传，正文、归属及恢复均保持。没有通过在应用或测试补一条普通鼠标移动、延时、特殊 PTY 或模型 fallback 规避问题。

生产使用 Cargo path override 固定完整已发布 gpui-pre 0.3.11 到 `vendor/gpui-pre`；原发布包 SHA-256 与此前 lockfile 一致。审计确认仅 src/window.rs 的输入模式分支改变；保留全部原发布文件与 Apache 许可，新增 HIVE-PATCH.md 记录差异。未引入另一套 GPUI，也未把临时显式渲染助手带进生产依赖。未来上游固定版本包含此修复时可移除 override。Mac 包同步携带许可与变更说明；Windows CI 路径包含 vendor。

## 验证与限制

- `check:attachments`：真实 Hive HTTP/SQLite、实际 Pi SDK、本机拒绝对端及生产 JSONL；补验剪贴板上传、名称、原字节、非法 base64/MIME 拒绝与纯文本模型发送前拒绝。未发生厂商模型推理。
- Rust 应用检查：18 通过、1 子进程入口按设计忽略。新增 BMP/TIFF 的实际编码→转换→逐像素一致性、PNG 字节不变、非法/过大/不支持格式；另存为验证替换后的原字节、失败保护及临时文件清理。最初测试的 glob import 将 gpui 的 test 宏引入，造成递归错误；已改显式导入，未提高递归上限或改生产行为迁就测试。
- 实际 GPUI 应用夹具：浅色宽窗、深色宽窗和 760×820 窄窗。实际 Cmd-V 图片、Finder 风格文件列表（附带路径文本）及中文文本、FileDrop 事件、HTTP 上传、切换草稿、退出重开、文件缺失发送失败与恢复、图片单独发送和历史重开均检查。每组从三个独立应用进程完成，最终本机模型对端只收到一次请求，图片 SHA-256 与原始 PNG 相同。
- 原图从恢复后的历史加载，检查 320×180 尺寸；用实际 GPUI MouseDown/MouseUp 点击复制与原尺寸按钮，校验系统剪贴板原图字节和布局状态，再用 Escape 关闭。Retina 校准后原尺寸按 scale_factor 换算逻辑尺寸，以截图的实际蓝/粉像素范围验证 320×180 图片不再被放大成 640×360。截图使用减少动态效果取得稳定最终帧，未将动画中间帧当作视觉完成。测试用原生 AppKit 逐类型暂存并恢复剪贴板数据，进程异常时驱动也会恢复并删除私有备份，不打印用户剪贴板。
- 复制文件的首轮夹具误用 GPUI 的文件剪贴板写入，该 macOS 实现是 no-op；随后只写现代 file URL 也未进入 GPUI 的旧 filenames 读取路径。最终夹具用原生 AppKit 同时写 Finder 风格 filenames、file URL 与路径文本，并验证文件导入且正文不含路径。没有为夹具给生产代码添加特殊文件路径。

- 这些是实际应用、原生组件、系统剪贴板和 GPUI 事件的检查，但事件由夹具注入；没有物理 Finder 拖拽、Windows DIB 系统剪贴板、操作系统保存对话框或 Windows GUI 验收。BMP/TIFF 的转换是实际编解码检查，不是 Windows 真机证据。
- `check:images`、`check:team`、Mac 默认应用构建与完整 Windows MSVC 默认功能类型检查通过。新 Windows CI 仍需原生运行，Mac 交叉检查不证明 Windows exe 链接、窗口或剪贴板。

本地日志、稳定截图和驱动源保存在 `.artifacts/image-input/`。GPUI 模式失败记录保存在 `drag-after-keyboard-failure.log`；此前截图曾捕获到对话框入场中间帧，最终夹具显式启用减少动态效果后重新检查。

真实 Terra 图片推理仍受上一轮 OpenAI OAuth invalid_grant 阻塞，没有重新尝试其他模型或 Magpie。只有现代 file URL、没有 filenames 的 macOS 文件剪贴板仍受 GPUI 读取限制；尚未扩展此上游能力。一般文件附件、图片编辑/动画、真实大型图片历史的资源性能、系统交互和正式发行继续待完成；完整目标保持开放。

## 本轮开发包

冻结生产 Node 依赖、默认功能 Mac 二进制重新打包，再从最终 ZIP 解压验证。包内 bundled Node 22.23.3 实际运行生产 JSONL 剪贴板上传、原图读回及拒绝路径；真实 PTY/Hive HTTP/SQLite smoke 和原生应用启动恢复均通过。确认包内原生二进制与构建 hash 相同，并携带 GPUI 原许可/差异说明。未签名、未公证，不是正式发行。

- ZIP：`.artifacts/hive-desktop-macos-arm64-image-input.zip`。
- ZIP SHA-256：`96a19e2a71b671396a117f0150d0f4a8d1faf2b5d945608f335a610bf0f12abd`。
- 二进制 SHA-256：`0884fd58a96f642787754ff580afcd01abd4232534dbd836638fe81126e46e1d`。
- 包内图片上传模块 SHA-256：`c599e4149c844b63d5de137d112f5c3806e2aefb80a79e9d4a081c1e369d5d9f`。

没有删除既有测试。采用上传/图片/团队定向回归、Rust 应用检查、平台类型检查、原生输入/渲染和解压包运行验证，没有运行完整 Node suite。Hive schema、团队协议、PTY 及工作流业务本轮未改。厂商账号和两个平台的系统交互边界仍未满足最终验收，未将本轮记作完整产品交付。

源码差异空白检查对自有改动通过。完整快照检查会指出 vendor 原始 _accessibility.rs 的两行注释尾空格（236、240）；为保留发布源码原样而未清洗，也未添加全局忽略规则。框架实际源差异已独立核对为仅 Window 的 FileDrop 模式分支。
