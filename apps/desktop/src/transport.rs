use crate::{
    process_supervisor::{CleanupReport, OwnedProcess},
    protocol::PiEvent,
    runtime_location::locate_runtime,
};
use serde_json::{Value, json};
use smol::channel::{Receiver, Sender, bounded, unbounded};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

type Reply = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<u64, Sender<Reply>>>>;
pub enum RuntimeEvent {
    Agent(PiEvent),
    Failed(String),
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
        let _ = reply.try_send(Err(message.clone()));
    }
    let _ = events.try_send(RuntimeEvent::Failed(message));
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
        command
            .arg(location.entry)
            .current_dir(location.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(directory) = location.node_directory {
            let mut paths = vec![directory];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            command.env(
                "PATH",
                std::env::join_paths(paths).map_err(|e| format!("无法设置后台 PATH：{e}"))?,
            );
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
        let (p, e, f) = (pending.clone(), events_tx.clone(), failure.clone());
        thread::spawn(move || {
            for value in rx {
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
            for line in BufReader::new(stdout).lines() {
                let value = match line
                    .and_then(|s| serde_json::from_str::<Value>(&s).map_err(std::io::Error::other))
                {
                    Ok(value) => value,
                    Err(error) => {
                        fail(&p, &e, &f, format!("后台协议错误：{error}"));
                        return;
                    }
                };
                if let Some(id) = value["id"].as_u64() {
                    if let Some(reply) = p.lock().unwrap().remove(&id) {
                        let result = if value.get("error").is_some() {
                            Err(format!(
                                "{}：{}",
                                value["error"]["code"], value["error"]["message"]
                            ))
                        } else {
                            Ok(value["result"].clone())
                        };
                        let _ = reply.try_send(result);
                    }
                } else if value["event"] == "agent" {
                    match serde_json::from_value(value["data"].clone()) {
                        Ok(event) => {
                            let _ = e.try_send(RuntimeEvent::Agent(event));
                        }
                        Err(error) => {
                            fail(&p, &e, &f, format!("Agent 事件格式错误：{error}"));
                            return;
                        }
                    }
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
        let owned = self.owned.clone();
        // Serialize concurrent cleanup callers. A second caller receives the
        // same completed report, never "done" merely because Child was taken.
        smol::unblock(move || owned.lock().unwrap().cleanup(grace)).await
    }
    pub async fn request(&self, method: &str, params: Value) -> Reply {
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
            self.pending.lock().unwrap().insert(id, tx);
        }
        if self
            .tx
            .send(json!({"id":id,"method":method,"params":params}))
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            return Err("后台写入线程已停止".into());
        }
        rx.recv()
            .await
            .unwrap_or_else(|_| Err("后台响应通道已关闭".into()))
    }
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
