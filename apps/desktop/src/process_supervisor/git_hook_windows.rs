//! Real production transport -> sidecar -> Git -> hook, in the desktop's Job.
use super::*;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows_sys::Win32::{
    Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        TerminateProcess, WaitForSingleObject,
    },
};

pub(super) struct Probe(pub(super) OwnedHandle);
pub(super) struct RuntimeCleanup(pub(super) std::sync::Arc<Runtime>);
impl Drop for RuntimeCleanup {
    fn drop(&mut self) {
        if let Err(error) = smol::block_on(self.0.shutdown_checked(Duration::ZERO)) {
            eprintln!("Git hook fixture cleanup failed: {error}");
        }
    }
}
impl Probe {
    pub(super) fn open(pid: u32, terminate: bool) -> Self {
        let rights = PROCESS_SYNCHRONIZE
            | PROCESS_QUERY_LIMITED_INFORMATION
            | if terminate { PROCESS_TERMINATE } else { 0 };
        let handle = unsafe { OpenProcess(rights, 0, pid) };
        assert!(
            !handle.is_null(),
            "OpenProcess({pid}): {}",
            std::io::Error::last_os_error()
        );
        Self(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
    pub(super) fn running(&self) -> bool {
        match unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => false,
            WAIT_TIMEOUT => true,
            other => panic!("Unexpected process wait result {other}"),
        }
    }
}

pub(super) fn run(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn desktop_job_reclaims_real_commit_hook_after_sidecar_death() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        root.join("packages/runtime/dist/main.js").is_file(),
        "Run pnpm build:runtime first; missing runtime is a failure, never a skip"
    );
    let directory =
        std::env::temp_dir().join(format!("hive-windows-git-hook-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let repo = directory.join("private repo");
    fs::create_dir(&repo).unwrap();
    let config = directory.join("gitconfig");
    fs::write(&config, "").unwrap();
    let git = |args: &[&str]| {
        let mut command = Command::new("git");
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .args(["-C"])
            .arg(&repo)
            .args(["-c", "core.fsmonitor=false", "-c", "core.autocrlf=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0");
        run(&mut command)
    };
    git(&["init", "--quiet", "--template=", "--initial-branch=main"]);
    for (key, value) in [
        ("user.name", "Hive fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgSign", "false"),
    ] {
        git(&["config", key, value]);
    }
    fs::write(repo.join("file.txt"), "baseline\n").unwrap();
    git(&["add", "file.txt"]);
    git(&["commit", "--quiet", "-m", "baseline"]);
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&["add", "file.txt"]);
    let head = git(&["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let unknown_lock = repo.join(".git/refs/heads/unrelated.lock");
    fs::write(&unknown_lock, "unknown owner; leave unchanged\n").unwrap();
    fs::create_dir_all(repo.join(".git/hooks")).unwrap();
    fs::write(
        repo.join(".git/hooks/pre-commit"),
        "#!/bin/sh\nexec \"$HIVE_TEST_NODE\" \"$HIVE_TEST_GIT_HOOK\"\n",
    )
    .unwrap();
    let hook = directory.join("hook.cjs");
    fs::write(
        &hook,
        r#"
const fs=require('node:fs'),path=require('node:path'),{spawn}=require('node:child_process');
const dir=process.env.HIVE_TEST_GIT_DIRECTORY;
const helper=spawn(process.execPath,['-e',`
 const fs=require('node:fs'),path=require('node:path'),dir=process.argv[1];
 setInterval(()=>{if(fs.existsSync(path.join(dir,'probe-after-exit')))
 fs.writeFileSync(path.join(dir,'late-write'),'survived');},10);
`,dir],{stdio:'ignore'});
fs.writeFileSync(path.join(dir,'hook.json'),JSON.stringify({hook:process.pid,helper:helper.pid}));
setInterval(()=>{},1000);
"#,
    )
    .unwrap();
    let mut external_command = Command::new("node");
    external_command
        .args(["-e", "setInterval(()=>{},1000)"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut external = OwnedProcess::spawn(&mut external_command).unwrap();
    let independent = Probe::open(external.id(), false);
    let node = run(Command::new("node").args(["-p", "process.execPath"]))
        .trim()
        .to_owned();
    let mut command = Command::new(&node);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .arg(root.join("packages/runtime/dist/main.js"))
        .current_dir(&directory)
        .env("HIVE_DESKTOP_DATA_DIR", directory.join("state"))
        .env("PI_CODING_AGENT_DIR", directory.join("agent"))
        .env("PI_OFFLINE", "1")
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HIVE_TEST_NODE", &node)
        .env("HIVE_TEST_GIT_HOOK", &hook)
        .env("HIVE_TEST_GIT_DIRECTORY", &directory)
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
                smol::Timer::after(Duration::from_secs(20)).await;
                Err(format!("Timed out: {method}"))
            })
            .await
            .unwrap()
        })
    };
    request("initialize", json!({}));
    let workspace = request(
        "hive.request",
        json!({"method":"POST", "path":"/api/workspaces",
        "body":{"path":repo, "name":"Windows hook ownership", "autostart_orchestrator":false}}),
    );
    let preview = request("git.commit.preview", json!({"workspaceId":workspace["id"]}));
    assert_eq!(preview["ok"], true, "{preview}");
    // A lock not created by this operation must block the write and survive it.
    let index_lock = repo.join(".git/index.lock");
    fs::write(&index_lock, "fixture foreign owner\n").unwrap();
    let refused = request(
        "git.commit.apply",
        json!({"workspaceId":workspace["id"],
        "expectedVersion":preview["preview"]["version"], "message":"must be refused"}),
    );
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"]["code"], "INDEX_BUSY", "{refused}");
    assert_eq!(
        fs::read_to_string(&index_lock).unwrap(),
        "fixture foreign owner\n"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&["rev-parse", "HEAD"]), head);
    assert!(!directory.join("hook.json").exists());
    // Only the test removes its explicitly created fixture lock before retrying.
    fs::remove_file(&index_lock).unwrap();
    let pending = runtime.begin_request(
        "git.commit.apply",
        json!({"workspaceId":workspace["id"],
        "expectedVersion":preview["preview"]["version"], "message":"must not complete"}),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let pids: Value = loop {
        if let Ok(bytes) = fs::read(directory.join("hook.json")) {
            if let Ok(value) = serde_json::from_slice(&bytes) {
                break value;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Real Git hook did not start"
        );
        thread::sleep(Duration::from_millis(10));
    };
    // Resolve the real Git ancestor, including Git-for-Windows' shell wrapper.
    let processes: Value = serde_json::from_str(&run(Command::new("powershell.exe").args([
        "-NoProfile", "-NonInteractive", "-Command",
        "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name | ConvertTo-Json -Compress"
    ]))).unwrap();
    let mut cursor = pids["hook"].as_u64().unwrap();
    let mut git_pid = None;
    for _ in 0..16 {
        let process = processes
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["ProcessId"].as_u64() == Some(cursor))
            .unwrap();
        if process["Name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("git.exe"))
        {
            git_pid = Some(cursor as u32);
            break;
        }
        cursor = process["ParentProcessId"].as_u64().unwrap();
    }
    let git_pid = git_pid.expect("Hook ancestor must be real Git");
    let probes = [
        Probe::open(git_pid, false),
        Probe::open(pids["hook"].as_u64().unwrap() as u32, false),
        Probe::open(pids["helper"].as_u64().unwrap() as u32, false),
    ];
    for process in &probes {
        assert!(process.running());
        assert!(
            runtime
                .owned
                .lock()
                .unwrap()
                .contains_handle(process.0.as_raw_handle())
                .unwrap()
        );
    }
    assert!(
        !runtime
            .owned
            .lock()
            .unwrap()
            .contains_handle(independent.0.as_raw_handle())
            .unwrap()
    );
    assert_ne!(
        unsafe { TerminateProcess(sidecar.0.as_raw_handle(), 17) },
        0
    );
    // Production leader watcher must reclaim the Job without test-triggered cleanup.
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while !runtime.owned.lock().unwrap().is_cleaned() {
        assert!(
            std::time::Instant::now() < deadline,
            "Production leader watcher did not confirm Job cleanup"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!sidecar.running());
    for process in &probes {
        assert!(!process.running(), "Original owned process object survived");
    }
    assert!(smol::block_on(pending).is_err());
    assert!(independent.running());
    fs::write(
        directory.join("probe-after-exit"),
        "test terminated helpers",
    )
    .unwrap();
    thread::sleep(Duration::from_millis(300));
    assert!(!directory.join("late-write").exists());
    assert_eq!(git(&["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert!(
        repo.join(".git/index.lock").exists(),
        "Interrupted commit lock must remain for inspection"
    );
    assert_eq!(
        fs::read_to_string(&unknown_lock).unwrap(),
        "unknown owner; leave unchanged\n"
    );
    let receipt = json!({"platform":"win32", "entry":"production Runtime::launch_command + dist/main.js JSONL",
        "sidecarPid":sidecar_pid, "gitPid":git_pid, "hook":pids, "sameOwnedJob":true, "originalHandlesSignaled":true,
        "productionLeaderWatcherCleanup":true, "independentAlive":true, "lateWrite":false,
        "headAndIndexUnchanged":true, "indexLockRetained":true, "foreignIndexLockRefusedAndUnchanged":true,
        "unknownLockUnchanged":true, "inference":false});
    eprintln!("WINDOWS_GIT_HOOK_OWNERSHIP_OK: {receipt}");
    if let Some(path) = std::env::var_os("HIVE_GIT_HOOK_RECEIPT") {
        if let Some(parent) = PathBuf::from(&path).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, receipt.to_string()).unwrap();
    }
    external.cleanup(Duration::ZERO).unwrap();
    drop(cleanup);
    drop(runtime);
    fs::remove_dir_all(directory).unwrap();
}
