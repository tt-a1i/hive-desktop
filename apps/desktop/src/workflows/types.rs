use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Deserialize)]
pub struct Run {
    pub id: String,
    pub name: String,
    pub script_path: String,
    pub script_hash: Option<String>,
    pub status: String,
    pub phase: Option<String>,
    pub parent_run_id: Option<String>,
    pub agent_count: u64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub args: Value,
    pub result: Value,
    pub error: Option<String>,
}
#[derive(Clone, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub script_path: String,
    pub cron: String,
    pub args: Value,
    pub enabled: bool,
    pub next_run_at: i64,
    pub last_run_at: Option<i64>,
}
#[derive(Clone, Deserialize)]
pub struct Dispatch {
    pub id: String,
    pub to_agent_id: String,
    pub from_agent_id: Option<String>,
    pub status: String,
    pub label: Option<String>,
    pub phase: Option<String>,
    pub step_index: Option<u64>,
    pub text: String,
    pub report_text: Option<String>,
    pub last_pty_line: Option<String>,
    pub artifacts: Vec<String>,
}
#[derive(Clone, Deserialize)]
pub struct Log {
    pub id: u64,
    pub ts: i64,
    pub message: String,
}
#[derive(Clone, Deserialize)]
pub struct Policy {
    pub default: String,
    pub allowed: Vec<String>,
    pub supported: Vec<String>,
}
#[derive(Deserialize)]
pub struct Runs {
    pub runs: Vec<Run>,
}
#[derive(Deserialize)]
pub struct Schedules {
    pub schedules: Vec<Schedule>,
}
#[derive(Deserialize)]
pub struct Feature {
    pub enabled: bool,
}
#[derive(Deserialize)]
pub struct RunDetail {
    pub run: Run,
}
#[derive(Deserialize)]
pub struct Dispatches {
    pub dispatches: Vec<Dispatch>,
}
#[derive(Deserialize)]
pub struct Logs {
    pub logs: Vec<Log>,
}

pub fn status(value: &str) -> &str {
    match value {
        "running" => "运行中",
        "completed" => "已完成",
        "failed" => "失败",
        "stopped" => "已停止",
        "interrupted" => "已中断",
        other => other,
    }
}
pub fn json(value: &Value) -> String {
    if let Value::String(text) = value {
        text.clone()
    } else if value.is_null() {
        "无".into()
    } else {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    }
}
pub fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}
pub fn relative_time(time: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let seconds = time.saturating_sub(now) / 1000;
    let count = seconds.unsigned_abs();
    let age = if count < 60 {
        format!("{count} 秒")
    } else if count < 3600 {
        format!("{} 分钟", count / 60)
    } else if count < 86400 {
        format!("{} 小时", count / 3600)
    } else {
        format!("{} 天", count / 86400)
    };
    format!("{age}{}", if seconds > 0 { "后" } else { "前" })
}
