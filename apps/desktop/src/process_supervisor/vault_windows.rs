//! Production Runtime transport and real PiVault read with an offline fake op.exe.
use super::git_hook_windows::{Probe, RuntimeCleanup, run};
use super::*;
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::System::Threading::TerminateProcess;

#[test]
fn desktop_job_reclaims_vault_pipe_holders_after_sidecar_death() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        root.join("packages/runtime/dist/main.js").is_file(),
        "Build runtime first"
    );
    let dir = std::env::temp_dir().join(format!("hive-windows-vault-{}", uuid::Uuid::new_v4()));
    let agent = dir.join("agent");
    let data = dir.join("state");
    let project = dir.join("project");
    let bin = dir.join("bin");
    for p in [&agent, &data, &project, &bin] {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(agent.join("models.json"), json!({"providers":{"fixture":{"api":"openai-responses","baseUrl":"http://127.0.0.1:1/v1","models":[{"id":"fixture","name":"Fixture","contextWindow":1000,"maxTokens":100}]}}}).to_string()).unwrap();
    fs::write(
        agent.join("settings.json"),
        json!({"defaultProvider":"fixture","defaultModel":"fixture"}).to_string(),
    )
    .unwrap();
    let source = root.join("apps/desktop/src/process_supervisor/vault_op_fixture.cs");
    let exe = bin.join("op.exe");
    run(Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command", "Add-Type -TypeDefinition (Get-Content -LiteralPath $env:HIVE_TEST_SOURCE -Raw) -OutputAssembly $env:HIVE_TEST_EXE -OutputType ConsoleApplication"]).env("HIVE_TEST_SOURCE",source).env("HIVE_TEST_EXE",&exe));
    let node = run(Command::new("node").args(["-p", "process.execPath"]))
        .trim()
        .to_owned();
    let mut independent_cmd = Command::new(&node);
    independent_cmd
        .args(["-e", "setInterval(()=>{},1000)"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut independent = OwnedProcess::spawn(&mut independent_cmd).unwrap();
    let other = Probe::open(independent.id(), false);
    let path = std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let mut command = Command::new(&node);
    command
        .arg(root.join("packages/runtime/dist/main.js"))
        .current_dir(&project)
        .env("HOME", &dir)
        .env("USERPROFILE", &dir)
        .env("LOCALAPPDATA", &dir)
        .env("HIVE_DESKTOP_DATA_DIR", &data)
        .env("PI_CODING_AGENT_DIR", &agent)
        .env("PI_OFFLINE", "1")
        .env("HIVE_TEST_VAULT_DIR", &dir)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let runtime = Runtime::launch_command(command).unwrap();
    let cleanup = RuntimeCleanup(runtime.clone());
    let sidecar_pid = runtime.owned.lock().unwrap().id();
    let sidecar = Probe::open(sidecar_pid, true);
    let request = |method: &str, params| {
        smol::block_on(async {
            smol::future::race(runtime.request(method, params), async {
                smol::Timer::after(Duration::from_secs(30)).await;
                Err(format!("Timed out: {method}"))
            })
            .await
            .unwrap()
        })
    };
    request("initialize", json!({}));
    let ws = request(
        "hive.request",
        json!({"method":"POST","path":"/api/workspaces","body":{"path":project,"name":"Vault ownership fixture","autostart_orchestrator":false}}),
    );
    let view = request("pi.vault.get", json!({}));
    request(
        "pi.vault.change",
        json!({"revision":view["revision"],"configRevision":view["configRevision"],"provider":"fixture","reference":"op://fixture/item/field"}),
    );
    let chat = request(
        "chat.create",
        json!({"workspaceId":ws["id"],"backend":"pi"}),
    );
    request("chat.refreshPiModels", json!({"sessionId":chat["id"]}));
    request(
        "chat.setModel",
        json!({"sessionId":chat["id"],"provider":"fixture","modelId":"fixture"}),
    );
    let pending = runtime.begin_request(
        "chat.prompt",
        json!({"sessionId":chat["id"],"text":"owned offline fixture"}),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let pids: Vec<u32> = loop {
        if let Ok(raw) = fs::read_to_string(dir.join("op-pids")) {
            if dir.join("helper-ready").exists() {
                break raw.split_whitespace().map(|x| x.parse().unwrap()).collect();
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "private op and helper did not start"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(pids.len(), 3);
    assert_eq!(pids[2], 3, "fake op stdout must be a pipe");
    let helper_status = fs::read_to_string(dir.join("helper-ready")).unwrap();
    let helper_status: Vec<u32> = helper_status
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    assert_eq!(
        helper_status,
        [pids[1], 3],
        "helper must hold the inherited output pipe"
    );
    let query = format!(
        "(Get-CimInstance Win32_Process -Filter 'ProcessId={}').ParentProcessId",
        pids[0]
    );
    let host_pid: u32 = run(Command::new("powershell.exe").args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &query,
    ]))
    .trim()
    .parse()
    .unwrap();
    let host = Probe::open(host_pid, false);
    let op = Probe::open(pids[0], false);
    let helper = Probe::open(pids[1], false);
    for p in [&host, &op, &helper] {
        assert!(p.running());
        assert!(
            runtime
                .owned
                .lock()
                .unwrap()
                .contains_handle(p.0.as_raw_handle())
                .unwrap()
        );
    }
    assert!(
        !runtime
            .owned
            .lock()
            .unwrap()
            .contains_handle(other.0.as_raw_handle())
            .unwrap()
    );
    assert_ne!(
        unsafe { TerminateProcess(sidecar.0.as_raw_handle(), 17) },
        0
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while !runtime.owned.lock().unwrap().is_cleaned() {
        assert!(
            std::time::Instant::now() < deadline,
            "production watcher did not reclaim Job"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!sidecar.running() && !host.running() && !op.running() && !helper.running());
    assert!(other.running());
    let _ = smol::block_on(pending);
    let receipt = json!({"entry":"production Runtime::launch_command + dist/main.js JSONL + PiVault", "sidecarPid":sidecar_pid,"hostPid":host_pid,"opPid":pids[0],"helperPid":pids[1],"sameDesktopJob":true,"watcherCleaned":true,"ownedOriginalHandlesSignaled":true,"unrelatedAlive":true,"privateFakeOp":true,"modelRequests":0});
    eprintln!("WINDOWS_VAULT_OWNERSHIP_OK: {receipt}");
    if let Some(path) = std::env::var_os("HIVE_VAULT_OWNER_RECEIPT") {
        if let Some(parent) = PathBuf::from(&path).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, receipt.to_string()).unwrap();
    }
    independent.cleanup(Duration::ZERO).unwrap();
    drop(cleanup);
    drop(runtime);
    fs::remove_dir_all(dir).unwrap();
}
