# 自动标题的Unicode截断与原生JSON兼容

2026-11-12，基于354ce96。真实prompt路径把首条消息text.trim().slice(0,60)作为自动标题；JavaScript按UTF-16 code unit截取。当第59个ASCII之后是🐝时，旧标题只保留高代理项\ud83d。JSON.stringify合法地转义它，但原生transport使用的serde_json::Value无法解析不完整代理项，进入既有fail路径；坏标题持久化后，即使聊天正文正确，后续列表/恢复响应仍会携带它。

## 修复

- 共享chat.prompt改为Array.from(text.trim()).slice(0,60).join('')，按Unicode code point截取。只处理标题，完整输入、附件、来源和投递语义不变。
- ChatIndex复用既有读取归一化，旧自动标题移除单独高/低代理项；Unicode模式正则不匹配有效代理对，正常emoji保留。用户自定义标题不改，原记录其他字段不改，读列表不会自动改写原文件；后续正常保存沿既有原子commit。
- 不从模型/正文重建未知标题、不新增store/migration/依赖或协议fallback。限制按code point，与现有重命名校验一致；不是新增完整grapheme断句或emoji组合分词。

## 可运行证据

`.artifacts/unicode-auto-title-20261112/prepare.mjs`复用原queued-user-inputs检查生成独立程序，把第一条真实原生输入换为59个x、🐝和后续文本。真实Pi SDK、Hive HTTP/SQLite、原生JSONL与本地HTTP回复继续执行完整排队/附件/停止/冷开/继续断言，保存实际后台记录按生产JSON序列化方式封装的响应。不是mock出一个坏标题。

- 旧真实自动标题isWellFormed=false，保存为\ud83d结尾；后续本地回复和队列流程仍能跑通，说明只有Node检查容易漏掉原生兼容问题。
- 独立Rust程序使用与桌面Cargo.lock相同serde_json=1.0.154，读取该响应，实际报unexpected end of hex escape；旧自动标题及旧持久化fixture两份均拒绝。
- 修改后新标题精确为59个x加完整🐝、60 code points；Rust实际接受并逐字断言。
- legacy.mjs通过真实runtime创建四后端空记录，植入两条旧残缺自动标题、30个完整🐝及一个自定义标题。读取后仅残片移除，正常emoji和自定义记录精确保持；raw index字节完全不变，搜索正常。新legacy响应被Rust完整解析，四条记录保持。

Rust probe只运行原生同版本JSON解析器，没有实际打开桌面或注入损坏响应使窗口断开；这里证明的是生产数据与原生解析器的兼容性，结合拥有实现的fail分支说明影响，不冒称已重现所有窗口症状。

## 检查及边界

pnpm check、pnpm build:runtime、git diff --check通过。四个现有检查均exit0：`pnpm --filter @hive-desktop/runtime exec tsx checks/session-titles.ts`、session-search.ts、session-archives.ts、empty-session-recovery.ts。真实本地输入/排队流程前后各运行一次；最后去除无必要的空标题fallback后，重新构建并复验旧数据及两个Rust接受路径。没有修改正式测试，临时断言式程序/原样JSON、Rust源码/锁文件、日志和receipt保留在上述目录。

本轮无面板操作或厂商推理，不改认证/全局模型。未跑无关全量suite；当前3d4422f安装包尚未含该修复。任意外部编辑造成的损坏自定义标题、所有组合emoji显示和平台完整体验不扩张为本批验收；原始聊天消息不因标题修复重写，完整目标继续。
