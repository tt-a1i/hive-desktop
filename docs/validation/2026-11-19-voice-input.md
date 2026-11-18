# 本机听写与草稿

`58050b7` 在聊天输入框加入开始、完成和取消听写。启动后用系统本机语言识别，单次最多采集 60 秒。识别文字追加到原草稿，发送仍由用户触发。应用输入法正在组字时，会保留已经识别的结果并显示插入按钮。切换会话、取消听写、发送原草稿、后台断开及退出时结束听写，避免迟到结果进入另一份草稿。

macOS 使用系统 Speech 和 AVFoundation。麦克风和语音识别的系统授权由用户触发，仅在本机语言支持离线识别时启动。Windows 使用系统已安装的桌面识别器。两个平台的听写进程由应用按需启动和回收。进程间只返回识别文字及状态，采集期间无需写入音频文件。[Apple 本机识别要求](https://developer.apple.com/documentation/speech/sfspeechrecognitionrequest/requiresondevicerecognition?changes=_3) · [Windows 桌面识别 API](https://learn.microsoft.com/en-us/dotnet/api/system.speech.recognition.speechrecognitionengine.recognizeasync?view=netframework-4.8.1)

## macOS 验收

- 普通 arm64 开发包带有麦克风和语音识别的系统用途说明。包内 `--voice-check-file` 用本机引擎从 WAV 识别出预期的英文句子。实际原生窗口打开隔离草稿，显示听写入口，空闲时没有启动听写进程。该包通过真实 PTY、Hive HTTP/SQLite 和退出冒烟。
- 使用带 `ui-preview` 特性的原生验收包，将录音来源换成实际 WAV，并通过正式的原生识别、进程、输入和保存流程。文字准确追加在含 emoji 的原草稿之后，冷重开保持只追加一次。长音频覆盖取消、切换到另一会话及听写中退出。两个会话的预期草稿逐字核对，三个主程序以 0 退出，观测到的三个听写进程全部回收。
- 听写包、系统状态、产物和日志保存在 `.artifacts/voice-mac-4abIQt/`。主要证据为 `production-audio.json`、`production-ui.log`、`packaged-entry/receipt.json`、`production-smoke.log`、`ui-fixed/receipt.json` 及 `ui-fixed/inserted.ax.txt`。测试使用本机合成音频，未开启麦克风或改变语音授权。

Rust 测试 114 项通过、1 项忽略。macOS 构建和 Windows 交叉编译通过，本地编译限单作业。Windows `aca54bb` 作业从解压的便携包调用系统英语识别器，离线将 WAV 识别为 “The garden has seven bright red flowers”。同一包通过 50 项运行时检查，原生进程监督作业也通过。Windows 从麦克风听写并写入草稿的窗口流程，以及两个平台的物理麦克风和首次授权弹窗，仍需实测。本轮没有发布。[Windows CI](https://github.com/tt-a1i/hive-desktop/actions/runs/44363997413) · [语音收据](https://github.com/tt-a1i/hive-desktop/actions/runs/44363997413/artifacts/14457827224)

## Windows 进程监督夹具

HTML 版本的 Windows 作业在 `leader_exit_is_detected_while_descendants_hold_stdout` 的 `result.forced` 断言失败。Node 的 libuv 在 Windows 会把普通子进程纳入父进程自有的 Job，父进程退出可能先于 Hive 回收这些子进程。夹具现在只在 Windows 对该子进程使用 `detached`，使其避开 Node 自己的 Job，仍处于 Hive 的外层 Job。测试现在明确核对独立的进程退出观察结果，再核对 Hive 强制回收及全部子进程退出。该改动只涉及夹具。[Node v22.23.3 进程源码](https://raw.githubusercontent.com/nodejs/node/v22.23.3/deps/uv/src/win/process.c)
