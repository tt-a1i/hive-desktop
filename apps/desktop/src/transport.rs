use crate::{
    process_supervisor::{CleanupReport, OwnedProcess},
    protocol::{PiEvent, TerminalWireEvent},
    runtime_location::locate_runtime,
};
use serde_json::{Value, json};
use smol::channel::{Receiver, Sender, bounded, unbounded};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

type Reply = Result<Value, String>;
type WireReply = Result<Value, WireFailure>;
type Pending = Arc<Mutex<HashMap<u64, PendingReply>>>;

struct PendingReply {
    sender: Sender<WireReply>,
    interaction: bool,
}

enum WireFailure {
    Message(String),
    Interaction(InteractionRequestError),
    HistorySnapshotStale,
}

pub(crate) enum HistoryPageRequestError {
    StaleSnapshot,
    Other(String),
}

#[derive(Debug)]
pub(crate) enum InteractionRequestError {
    AttachmentCopyUnavailable(String),
    Other,
}

impl WireFailure {
    fn from_frame(error: &Value, interaction: bool) -> Self {
        if interaction {
            let kind = match (error["code"].as_str(), error["attachmentId"].as_str()) {
                (Some("QUESTION_ATTACHMENT_COPY_UNAVAILABLE"), Some(id)) => {
                    InteractionRequestError::AttachmentCopyUnavailable(id.to_owned())
                }
                _ => InteractionRequestError::Other,
            };
            return Self::Interaction(kind);
        }
        if error["code"] == "HISTORY_SNAPSHOT_STALE" {
            return Self::HistorySnapshotStale;
        }
        Self::Message(
            error["message"]
                .as_str()
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("后台请求失败，请查看后台日志。")
                .to_owned(),
        )
    }

    fn into_message(self) -> String {
        match self {
            Self::Message(message) => message,
            Self::Interaction(_) => "后台请求失败，请查看后台日志。".into(),
            Self::HistorySnapshotStale => crate::language::tr("已保存的历史发生变化，请重新读取会话历史。").into(),
        }
    }
}

struct CancelSearch<'a> {
    runtime: &'a Runtime,
    request_id: u64,
    method: &'static str,
}

impl Drop for CancelSearch<'_> {
    fn drop(&mut self) {
        let runtime = self.runtime;
        let pending = runtime.pending.lock().unwrap().remove(&self.request_id);
        if pending.is_some() && !runtime.closing.load(Ordering::Acquire) {
            let id = runtime.next.fetch_add(1, Ordering::Relaxed);
            let _ = runtime.tx.send(json!({
                "id": id,
                "method": self.method,
                "params": {"requestId": self.request_id},
            }));
        }
    }
}
pub enum RuntimeEvent {
    Agent(PiEvent),
    Terminal(TerminalWireEvent),
    WorktreeInitializationChanged { workspace: String, session: String },
    Failed(String),
}

fn worktree_initialization_changed(data: &Value) -> Option<RuntimeEvent> {
    Some(RuntimeEvent::WorktreeInitializationChanged {
        workspace: data["workspaceId"].as_str()?.to_owned(),
        session: data["sessionId"].as_str()?.to_owned(),
    })
}

pub struct Runtime {
    tx: mpsc::Sender<Value>,
    pending: Pending,
    next: AtomicU64,
    pub events: Receiver<RuntimeEvent>,
    owned: Arc<Mutex<OwnedProcess>>,
    closing: Arc<AtomicBool>,
    events_tx: Sender<RuntimeEvent>,
    failure: Arc<Mutex<Option<String>>>,
}
fn fail(
    pending: &Pending,
    events: &Sender<RuntimeEvent>,
    failure: &Mutex<Option<String>>,
    message: String,
) {
    *failure.lock().unwrap() = Some(message.clone());
    for (_, reply) in pending.lock().unwrap().drain() {
        let _ = reply.sender.try_send(Err(WireFailure::Message(message.clone())));
    }
    let _ = events.try_send(RuntimeEvent::Failed(message));
}

fn read_frame(reader: &mut impl BufRead, scratch: &mut Vec<u8>) -> Result<Option<Value>, String> {
    scratch.clear();
    if reader.read_until(b'\n', scratch).map_err(|error| error.to_string())? == 0 {
        return Ok(None);
    }
    let result = serde_json::from_slice(scratch).map_err(|error| error.to_string());
    // Keep the buffer for frequent small RPCs, but release an entire history
    // or attachment frame as soon as the parsed value owns its contents.
    if scratch.capacity() > 256 * 1024 {
        *scratch = Vec::new();
    }
    result.map(Some)
}

const SHELL_PATH_MARKER: &str = "__HIVE_PATH__";

fn is_macos_launchservices_path(paths: &[PathBuf]) -> bool {
    paths.iter().all(|path| {
        let s = path.to_string_lossy();
        matches!(
            s.as_ref(),
            "" | "/usr/bin"
                | "/bin"
                | "/usr/sbin"
                | "/sbin"
                | "/usr/local/bin"
                | "/Library/Apple/usr/bin"
        ) || s.starts_with("/System/Cryptexes/")
            || s.starts_with("/var/run/com.apple.security.cryptexd/")
    })
}

fn parse_shell_path_output(raw: &str) -> Option<&str> {
    let start = raw.find(SHELL_PATH_MARKER)? + SHELL_PATH_MARKER.len();
    let rest = &raw[start..];
    let end = rest.find(SHELL_PATH_MARKER)?;
    let path = rest[..end].trim();
    (!path.is_empty()).then_some(path)
}

fn merge_runtime_paths(
    node_directory: Option<PathBuf>,
    current: Vec<PathBuf>,
    hydrated: Vec<PathBuf>,
    fallbacks: Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |dir: PathBuf| {
        if !dir.as_os_str().is_empty() && seen.insert(dir.clone()) {
            merged.push(dir);
        }
    };
    if let Some(dir) = node_directory {
        push(dir);
    }
    for dir in current {
        push(dir);
    }
    for dir in hydrated {
        push(dir);
    }
    for dir in fallbacks {
        push(dir);
    }
    merged
}

#[cfg(target_os = "macos")]
fn probe_macos_login_shell_path() -> Vec<PathBuf> {
    let shell = std::env::var_os("SHELL")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OsString::from("/bin/zsh"));
    let script = format!("printf '{SHELL_PATH_MARKER}%s{SHELL_PATH_MARKER}' \"$PATH\"");
    let Ok(mut child) = Command::new(&shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let reader = child.stdout.take().map(|mut stdout| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    });
    let deadline = Instant::now() + Duration::from_millis(1500);
    let exited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    if !exited {
        return Vec::new();
    }
    let output = reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    parse_shell_path_output(&output)
        .map(|raw| std::env::split_paths(raw).collect())
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn macos_fallback_user_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for rel in [
            ".npm-global/bin",
            "Library/pnpm",
            "Library/pnpm/bin",
            ".local/bin",
            ".bun/bin",
            ".volta/bin",
            ".mise/shims",
            ".asdf/shims",
            ".cargo/bin",
        ] {
            let candidate = home.join(rel);
            if candidate.is_dir() {
                dirs.push(candidate);
            }
        }
    }
    for system in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"] {
        let candidate = Path::new(system);
        if candidate.is_dir() {
            dirs.push(candidate.to_path_buf());
        }
    }
    dirs
}

fn resolve_runtime_path(node_directory: Option<PathBuf>) -> Option<OsString> {
    let current: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    #[cfg(target_os = "macos")]
    let (hydrated, fallbacks) = if is_macos_launchservices_path(&current)
        && std::env::var_os("HIVE_DESKTOP_NO_SHELL_PATH").is_none()
    {
        (probe_macos_login_shell_path(), macos_fallback_user_bin_dirs())
    } else {
        (Vec::new(), Vec::new())
    };
    #[cfg(not(target_os = "macos"))]
    let (hydrated, fallbacks): (Vec<PathBuf>, Vec<PathBuf>) = (Vec::new(), Vec::new());

    if node_directory.is_none() && hydrated.is_empty() && fallbacks.is_empty() {
        return None;
    }
    let merged = merge_runtime_paths(node_directory, current, hydrated, fallbacks);
    std::env::join_paths(merged).ok()
}

impl Runtime {
    pub fn launch() -> Result<Arc<Self>, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let executable = std::env::current_exe().map_err(|e| format!("无法读取应用路径：{e}"))?;
        let location = locate_runtime(
            &executable,
            &root,
            std::env::var_os("HIVE_DESKTOP_RUNTIME"),
            std::env::var_os("HIVE_DESKTOP_NODE"),
        )?;
        let mut command = Command::new(location.node);
        #[cfg(target_os = "macos")]
        command.env("HIVE_DESKTOP_SHELL_OWNER", &executable);
        command
            .arg(location.entry)
            .current_dir(location.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(path) = resolve_runtime_path(location.node_directory) {
            command.env("PATH", path);
        }
        Self::launch_command(command)
    }
    fn launch_command(mut command: Command) -> Result<Arc<Self>, String> {
        let mut owned = OwnedProcess::spawn(&mut command)
            .map_err(|e| format!("启动/监督 Node 后台失败：{e}"))?;
        let mut stdin = owned.take_stdin().ok_or("后台没有 stdin")?;
        let stdout = owned.take_stdout().ok_or("后台没有 stdout")?;
        let owned = Arc::new(Mutex::new(owned));
        let closing = Arc::new(AtomicBool::new(false));
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let failure = Arc::new(Mutex::new(None));
        let (events_tx, events) = unbounded();
        let (tx, rx) = mpsc::channel::<Value>();
        let trace = std::env::var_os("HIVE_DESKTOP_TRACE_RPC").is_some();
        let (p, e, f) = (pending.clone(), events_tx.clone(), failure.clone());
        thread::spawn(move || {
            for value in rx {
                if trace {
                    eprintln!("[hive rpc] send {} {}", value["id"], value["method"]);
                }
                let result = serde_json::to_writer(&mut stdin, &value)
                    .map_err(std::io::Error::other)
                    .and_then(|_| stdin.write_all(b"\n"))
                    .and_then(|_| stdin.flush());
                if let Err(error) = result {
                    fail(&p, &e, &f, format!("写入后台失败：{error}"));
                    break;
                }
            }
        });
        let (p, e, f) = (pending.clone(), events_tx.clone(), failure.clone());
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut scratch = Vec::new();
            loop {
                let mut value = match read_frame(&mut reader, &mut scratch) {
                    Ok(Some(value)) => value,
                    Ok(None) => break,
                    Err(error) => {
                        fail(&p, &e, &f, format!("后台协议错误：{error}"));
                        return;
                    }
                };
                if let Some(id) = value["id"].as_u64() {
                    let pending_reply = p.lock().unwrap().remove(&id);
                    if trace {
                        if let Some(error) = value.get("error") {
                            // Provider interaction errors can contain a submitted secret.
                            if pending_reply.as_ref().is_some_and(|reply| !reply.interaction) {
                                eprintln!("[hive rpc] reply {id} {error}");
                            } else {
                                eprintln!("[hive rpc] reply {id} failed");
                            }
                        } else {
                            eprintln!("[hive rpc] reply {id} ok");
                        }
                    }
                    if let Some(reply) = pending_reply {
                        let result = if let Some(error) = value.get("error") {
                            Err(WireFailure::from_frame(error, reply.interaction))
                        } else {
                            Ok(value.get_mut("result").map(std::mem::take).unwrap_or_default())
                        };
                        let _ = reply.sender.try_send(result);
                    }
                } else if value["event"] == "terminal" {
                    match serde_json::from_value(value.get_mut("data").map(std::mem::take).unwrap_or_default()) {
                        Ok(event) => {
                            let _ = e.try_send(RuntimeEvent::Terminal(event));
                        }
                        Err(error) => {
                            fail(&p, &e, &f, format!("终端事件格式错误：{error}"));
                            return;
                        }
                    }
                } else if value["event"] == "agent" {
                    match serde_json::from_value(value.get_mut("data").map(std::mem::take).unwrap_or_default()) {
                        Ok(event) => {
                            let _ = e.try_send(RuntimeEvent::Agent(event));
                        }
                        Err(error) => {
                            fail(&p, &e, &f, format!("Agent 事件格式错误：{error}"));
                            return;
                        }
                    }
                } else if value["event"] == "worktreeInitializationChanged"
                    && let Some(event) = worktree_initialization_changed(&value["data"])
                {
                    let _ = e.try_send(event);
                }
            }
            fail(
                &p,
                &e,
                &f,
                "后台连接已关闭；请重新启动 Hive。详细错误见后台日志。".into(),
            );
        });
        // stdout may stay open in a descendant after Node exits. Observe the
        // leader independently, without reaping/releasing its PID on macOS.
        let (scope, stopping, p, e, f) = (
            owned.clone(),
            closing.clone(),
            pending.clone(),
            events_tx.clone(),
            failure.clone(),
        );
        thread::spawn(move || {
            loop {
                if stopping.load(Ordering::Acquire) {
                    return;
                }
                let exited = {
                    let owner = scope.lock().unwrap();
                    if owner.is_cleaned() {
                        return;
                    }
                    owner.leader_exited()
                };
                match exited {
                    Ok(false) => thread::sleep(Duration::from_millis(20)),
                    outcome => {
                        if stopping.swap(true, Ordering::AcqRel) {
                            return;
                        }
                        let message = match outcome {
                            Ok(true) => {
                                "后台已退出，请重新打开 Hive；未收到结果的操作请先核对执行情况。"
                                    .into()
                            }
                            Err(error) => format!("无法检查后台进程状态：{error}"),
                            _ => unreachable!(),
                        };
                        fail(&p, &e, &f, message);
                        match scope.lock().unwrap().cleanup(Duration::ZERO) {
                            Ok(report) => eprintln!(
                                "[hive-desktop] sidecar scope exited: forced={}, scope={}",
                                report.forced, report.scope
                            ),
                            Err(error) => {
                                eprintln!("[hive-desktop] sidecar cleanup not confirmed: {error}");
                                let _ = e.try_send(RuntimeEvent::Failed(format!(
                                    "后台进程清理未确认：{error}"
                                )));
                            }
                        }
                        return;
                    }
                }
            }
        });
        Ok(Arc::new(Self {
            tx,
            pending,
            next: AtomicU64::new(1),
            events,
            owned,
            closing,
            events_tx,
            failure,
        }))
    }
    pub async fn shutdown(&self) {
        // Last-chance, non-cancellable GPUI quit observer has only 200 ms.
        // Normal window/shortcut exit performs its 3 s pre-cleanup separately.
        // Issue scope termination immediately here, never spend that window in grace.
        self.shutdown_with_grace(Duration::ZERO).await;
    }
    pub async fn shutdown_with_grace(&self, grace: Duration) {
        if let Err(error) = self.shutdown_checked(grace).await {
            eprintln!("[hive-desktop] sidecar cleanup not confirmed: {error}");
            let _ = self
                .events_tx
                .try_send(RuntimeEvent::Failed(format!("后台进程清理未确认：{error}")));
        }
    }
    /// Future reconnect must await this barrier and stop on Err. Success is scoped
    /// to the owned group/Job; it does not attest to arbitrary detached processes.
    pub async fn shutdown_checked(&self, grace: Duration) -> Result<CleanupReport, String> {
        self.closing.store(true, Ordering::Release);
        let _ = self
            .tx
            .send(json!({"id":0,"method":"shutdown","params":{}}));
        self.finish_shutdown(grace).await
    }
    // Reaping the sidecar and authorizing the installer are separate results.
    pub async fn shutdown_for_install(
        &self,
        grace: Duration,
        installer_id: &str,
    ) -> Result<Result<(), String>, String> {
        let reply = self.begin_request("shutdown", json!({"desktopInstallerId": installer_id}));
        self.closing.store(true, Ordering::Release);
        self.finish_shutdown(grace).await?;
        let confirmation = smol::future::race(reply, async {
            smol::Timer::after(Duration::from_secs(1)).await;
            Err("未收到后台安装交接确认".into())
        })
        .await
        .and_then(|receipt| {
            if receipt["desktopInstallerId"].as_str() == Some(installer_id) {
                Ok(())
            } else {
                Err("后台安装交接确认不匹配".into())
            }
        });
        Ok(confirmation)
    }
    async fn finish_shutdown(&self, grace: Duration) -> Result<CleanupReport, String> {
        let owned = self.owned.clone();
        // Serialize concurrent cleanup callers. A second caller receives the
        // same completed report, never "done" merely because Child was taken.
        smol::unblock(move || owned.lock().unwrap().cleanup(grace)).await
    }
    pub async fn request(&self, method: &str, params: Value) -> Reply {
        self.begin_request(method, params).await
    }
    pub(crate) async fn interaction_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, InteractionRequestError> {
        let (_, reply) = self.enqueue_request(method, params)
            .map_err(|_| InteractionRequestError::Other)?;
        await_wire_reply(reply).await.map_err(|error| match error {
            WireFailure::Interaction(kind) => kind,
            _ => InteractionRequestError::Other,
        })
    }
    pub(crate) async fn history_page(&self, params: Value) -> Result<Value, HistoryPageRequestError> {
        let (_, reply) = self.enqueue_request("chat.historyPage", params)
            .map_err(HistoryPageRequestError::Other)?;
        await_wire_reply(reply).await.map_err(|error| match error {
            WireFailure::HistorySnapshotStale => HistoryPageRequestError::StaleSnapshot,
            other => HistoryPageRequestError::Other(other.into_message()),
        })
    }
    pub async fn search(&self, params: Value) -> Reply {
        self.cancellable_search("chat.search", "chat.search.cancel", params).await
    }
    pub(crate) async fn search_conversation_messages(&self, params: Value) -> Reply {
        self.cancellable_search("chat.searchMessages", "chat.search.cancel", params).await
    }
    pub(crate) async fn search_project_files(&self, contents: bool, params: Value) -> Reply {
        let method = if contents { "project.files.searchContent" } else { "project.files.search" };
        self.cancellable_search(method, "project.files.search.cancel", params).await
    }
    async fn cancellable_search(&self, method: &str, cancel: &'static str, params: Value) -> Reply {
        let (request_id, reply) = self.enqueue_request(method, params)?;
        let _cancel = CancelSearch { runtime: self, request_id, method: cancel };
        await_reply(reply).await
    }
    /// Enqueue in caller order; the existing writer remains the only wire writer.
    /// Terminal input/ACK must not depend on async task polling order.
    pub fn begin_request(
        &self,
        method: &str,
        params: Value,
    ) -> impl std::future::Future<Output = Reply> + use<> {
        let pending = self.enqueue_request(method, params);
        async move { await_reply(pending?.1).await }
    }

    fn enqueue_request(&self, method: &str, params: Value) -> Result<(u64, Receiver<WireReply>), String> {
        if self.closing.load(Ordering::Acquire) {
            return Err("后台正在关闭或已退出".into());
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = bounded(1);
        // Lock failure before pending so an EOF cannot strand a newly inserted request.
        {
            let failure = self.failure.lock().unwrap();
            if let Some(error) = &*failure {
                return Err(error.clone());
            }
            self.pending.lock().unwrap().insert(id, PendingReply {
                sender: tx,
                interaction: matches!(method, "chat.interactionAttachments.save" | "chat.respondInteraction"),
            });
        }
        if self
            .tx
            .send(json!({"id":id,"method":method,"params":params}))
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            return Err("后台写入线程已停止".into());
        }
        Ok((id, rx))
    }
}
async fn await_reply(reply: Receiver<WireReply>) -> Reply {
    await_wire_reply(reply).await.map_err(WireFailure::into_message)
}
async fn await_wire_reply(reply: Receiver<WireReply>) -> WireReply {
    reply.recv().await.unwrap_or_else(|_| Err(WireFailure::Message("后台响应通道已关闭".into())))
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Release);
        let _ = self
            .tx
            .send(json!({"id":0,"method":"shutdown","params":{}}));
        let owned = self.owned.clone();
        thread::spawn(move || {
            if let Err(error) = owned.lock().unwrap().cleanup(Duration::from_secs(3)) {
                eprintln!("[hive-desktop] dropped sidecar cleanup not confirmed: {error}");
            }
        });
    }
}

#[cfg(test)]
#[path = "process_supervisor/transport_tests.rs"]
mod tests;
