use crate::protocol::PiEvent;
use serde_json::{Value, json};
use smol::channel::{Receiver, Sender, bounded, unbounded};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
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
    child: Mutex<Option<Child>>,
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
        let entry = std::env::var_os("HIVE_DESKTOP_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("packages/runtime/dist/main.js"));
        if !entry.is_file() {
            return Err(format!(
                "找不到后台 {}，请先运行 pnpm build:runtime",
                entry.display()
            ));
        }
        let mut child =
            Command::new(std::env::var_os("HIVE_DESKTOP_NODE").unwrap_or_else(|| "node".into()))
                .arg(entry)
                .current_dir(root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| format!("启动 Node 后台失败：{e}"))?;
        let mut stdin = child.stdin.take().ok_or("后台没有 stdin")?;
        let stdout = child.stdout.take().ok_or("后台没有 stdout")?;
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
        let (p, e, f) = (pending.clone(), events_tx, failure.clone());
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
        Ok(Arc::new(Self {
            tx,
            pending,
            next: AtomicU64::new(1),
            events,
            child: Mutex::new(Some(child)),
            failure,
        }))
    }
    pub async fn shutdown(&self) {
        self.shutdown_with_grace(Duration::from_millis(100)).await;
    }
    pub async fn shutdown_with_grace(&self, grace: Duration) {
        let _ = self
            .tx
            .send(json!({"id":0,"method":"shutdown","params":{}}));
        let child = self.child.lock().unwrap().take();
        // GPUI gives quit observers 200 ms. Reap inside that window, rather than
        // leaving a detached cleanup thread behind when the application exits.
        if let Some(child) = child {
            smol::unblock(move || reap(child, grace)).await;
        }
    }
    pub async fn request(&self, method: &str, params: Value) -> Reply {
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
        // Reap only our child, off the UI thread. Give the runtime time to dispose Pi and Hive.
        let Some(owned) = self.child.get_mut().unwrap().take() else {
            return;
        };
        let _ = self
            .tx
            .send(json!({"id":0,"method":"shutdown","params":{}}));
        thread::spawn(move || reap(owned, Duration::from_secs(3)));
    }
}
fn reap(mut child: Child, grace: Duration) {
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Err(_) => break,
            _ => thread::sleep(Duration::from_millis(10)),
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}
