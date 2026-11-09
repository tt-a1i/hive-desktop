use crate::language::tr;

// Only application-owned usage metadata from agent-usage.ts; preserve unknown/model labels.
pub(super) fn native_label(text: &str) -> String {
    if let Some(model) = text.strip_prefix("模型累计 · ") {
        return format!("{} · {model}", tr("模型累计"));
    }
    const LABELS: &[&str] = &[
        "Codex 原生线程",
        "Claude 官方 SDK",
        "原生会话累计",
        "原生线程累计",
        "原生最近一次用量",
        "最近报告的模型容量",
        "最近一轮主循环",
        "模型累计",
        "输入（不含缓存）",
        "输入（含缓存）",
        "输出",
        "缓存读取",
        "缓存写入",
        "原生总计",
        "缓存输入（输入子集）",
        "推理输出（输出子集）",
        "思考（输出子集）",
        "上下文窗口容量",
        "最大输出容量",
        "窗口容量（最近报告）",
        "本次原生连接尚未返回线程用量；未提供的数据不会显示为零。",
        "上下文为 Pi 原生估算；压缩后尚无新回复时可能暂不可用。累计用量不等于当前上下文大小。",
        "最近一次用量不是实时上下文占用；缓存输入和推理输出分别属于输入、输出的子集。重新连接后以原生通知为准。",
        "本次连接尚未收到原生回合统计；历史不会被重新计为零。",
        "主循环统计不含子 Agent 和辅助调用；模型累计取最新原生结果，不逐轮相加。原生报告有自身的恢复及重置边界，不是订阅额度或账单。",
        "该原生后端未提供用量读取接口。",
    ];
    LABELS.iter().find(|key| **key == text).map(|key| tr(key).to_owned()).unwrap_or_else(|| text.to_owned())
}
