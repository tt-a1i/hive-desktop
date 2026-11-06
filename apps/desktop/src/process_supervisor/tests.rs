use super::*;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn fixture_directory(prefix: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    loop {
        let directory = std::env::temp_dir().join(format!(
            "{}-{}-{}-{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&directory) {
            Ok(()) => return directory,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!(
                "Failed to create fixture directory {}: {error}",
                directory.display()
            ),
        }
    }
}

fn fixture(role: &str, directory: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "process_supervisor::tests::process_tree_fixture",
            "--nocapture",
            "--ignored",
        ])
        .env("HIVE_PROCESS_TREE_ROLE", role)
        .env("HIVE_PROCESS_TREE_DIRECTORY", directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    command
}
#[test]
#[ignore = "subprocess fixture entry point, not a standalone assertion"]
fn process_tree_fixture() {
    let Ok(role) = std::env::var("HIVE_PROCESS_TREE_ROLE") else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os("HIVE_PROCESS_TREE_DIRECTORY").unwrap());
    if role == "grandchild" {
        fs::write(
            directory.join("grandchild.pid"),
            std::process::id().to_string(),
        )
        .unwrap();
        thread::sleep(Duration::from_secs(120));
        return;
    }
    if role == "child" {
        #[cfg(target_os = "macos")]
        let mut grandchild = fixture("grandchild", &directory).spawn().unwrap();
        // Windows mirrors the two ownership layers: sidecar Job outside,
        // per-CLI Job inside. CI must actually run this, not infer it from macOS.
        #[cfg(windows)]
        let mut grandchild = OwnedProcess::spawn(&mut fixture("grandchild", &directory)).unwrap();
        fs::write(directory.join("child.pid"), std::process::id().to_string()).unwrap();
        let mut input = [0];
        if std::io::stdin().read_exact(&mut input).is_err() {
            thread::sleep(Duration::from_secs(120));
            return;
        }
        #[cfg(target_os = "macos")]
        {
            grandchild.kill().unwrap();
            grandchild.wait().unwrap();
        }
        #[cfg(windows)]
        grandchild.cleanup(Duration::ZERO).unwrap();
        return;
    }
    // Like the actual Node host, no external descendants are started until the
    // parent has completed containment and submits its first request.
    let mut input = [0];
    std::io::stdin().read_exact(&mut input).unwrap();
    let mut child = fixture("child", &directory).spawn().unwrap();
    std::io::stdin().read_exact(&mut input).unwrap();
    if input[0] == b'x' {
        std::process::exit(17);
    }
    child.stdin.as_mut().unwrap().write_all(b"q").unwrap();
    assert!(child.wait().unwrap().success());
}
fn pid(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(value) = fs::read_to_string(path) {
            if let Ok(pid) = value.parse() {
                return pid;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Fixture PID did not become ready: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}
#[cfg(target_os = "macos")]
fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
#[cfg(windows)]
struct ProcessProbe {
    pid: u32,
    handle: std::os::windows::io::OwnedHandle,
}
#[cfg(windows)]
impl ProcessProbe {
    fn open(pid: u32) -> Self {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        };
        let handle = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        assert!(
            !handle.is_null(),
            "OpenProcess({pid}) failed: {}",
            std::io::Error::last_os_error()
        );
        Self {
            pid,
            handle: unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle) },
        }
    }
    fn alive(&self) -> bool {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
            System::Threading::WaitForSingleObject,
        };
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => false,
            WAIT_TIMEOUT => true,
            _ => panic!(
                "WaitForSingleObject({}) failed: {}",
                self.pid,
                std::io::Error::last_os_error()
            ),
        }
    }
}
#[cfg(windows)]
fn alive(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let running = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    unsafe {
        CloseHandle(handle);
    }
    running
}
fn check_tree(mode: u8) {
    let directory = fixture_directory("hive-owned-tree");
    let external_directory = directory.join("independent");
    fs::create_dir(&external_directory).unwrap();
    let mut external = fixture("grandchild", &external_directory).spawn().unwrap();
    let independent_pid = pid(&external_directory.join("grandchild.pid"));
    let mut owned = OwnedProcess::spawn(&mut fixture("leader", &directory)).unwrap();
    let mut input = owned.take_stdin().unwrap();
    input.write_all(b"g").unwrap();
    let child = pid(&directory.join("child.pid"));
    let grandchild = pid(&directory.join("grandchild.pid"));
    #[cfg(windows)]
    let (child_probe, grandchild_probe, independent_probe) = (
        ProcessProbe::open(child),
        ProcessProbe::open(grandchild),
        ProcessProbe::open(independent_pid),
    );
    assert!(alive(child) && alive(grandchild));
    #[cfg(windows)]
    assert!(child_probe.alive() && grandchild_probe.alive() && independent_probe.alive());
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        assert!(
            owned
                .scope
                .contains(child_probe.handle.as_raw_handle())
                .unwrap()
        );
        assert!(
            owned
                .scope
                .contains(grandchild_probe.handle.as_raw_handle())
                .unwrap()
        );
        assert!(
            !owned
                .scope
                .contains(independent_probe.handle.as_raw_handle())
                .unwrap()
        );
        eprintln!(
            "fixture mode={} original handles verified in owned Job before cleanup",
            mode as char
        );
    }
    if mode != b'f' {
        input.write_all(&[mode]).unwrap();
    }
    let report = owned
        .cleanup(if mode == b'q' {
            Duration::from_secs(3)
        } else {
            Duration::from_millis(100)
        })
        .unwrap();
    assert_eq!(report.forced, mode != b'q');
    #[cfg(windows)]
    {
        let (child_alive, grandchild_alive, independent_alive) = (
            child_probe.alive(),
            grandchild_probe.alive(),
            independent_probe.alive(),
        );
        eprintln!(
            "fixture mode={} report={report:?}; original handles: child={child} alive={child_alive}, grandchild={grandchild} alive={grandchild_alive}, independent={independent_pid} alive={independent_alive}; PID lookup: child={}, grandchild={}, independent={}",
            mode as char,
            alive(child),
            alive(grandchild),
            alive(independent_pid)
        );
        if child_alive || grandchild_alive {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::WaitForSingleObject;
            let start = Instant::now();
            let child_wait =
                unsafe { WaitForSingleObject(child_probe.handle.as_raw_handle(), 3000) };
            let grandchild_wait =
                unsafe { WaitForSingleObject(grandchild_probe.handle.as_raw_handle(), 3000) };
            eprintln!(
                "termination diagnostics (original immediate assertions retained): child_wait={child_wait}, grandchild_wait={grandchild_wait}, elapsed={:?}",
                start.elapsed()
            );
        }
        assert!(!child_alive, "Owned child survived cleanup");
        assert!(!grandchild_alive, "Owned grandchild survived cleanup");
        assert!(
            independent_alive,
            "Independent process was killed by unrelated scope cleanup"
        );
    }
    #[cfg(target_os = "macos")]
    {
        assert!(!alive(child), "Owned child survived cleanup");
        assert!(!alive(grandchild), "Owned grandchild survived cleanup");
        assert!(
            alive(independent_pid),
            "Independent process was killed by unrelated scope cleanup"
        );
    }
    assert_eq!(
        owned.cleanup(Duration::ZERO).unwrap().forced,
        report.forced,
        "Repeat cleanup must not signal a recycled group ID"
    );
    external.kill().unwrap();
    external.wait().unwrap();
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn owned_tree_normal_shutdown() {
    check_tree(b'q');
}
#[test]
fn owned_tree_leader_abnormal_exit() {
    check_tree(b'x');
}
#[test]
fn owned_tree_forced_shutdown() {
    check_tree(b'f');
}

#[cfg(target_os = "macos")]
#[test]
fn node_sidecar_boot_does_not_spawn_agents_before_initialize() {
    use std::io::{BufRead, BufReader};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = fixture_directory("hive-sidecar-boot");
    let mut command = Command::new("node");
    command
        .arg(root.join("packages/runtime/dist/main.js"))
        .arg("--data-dir")
        .arg(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owned = OwnedProcess::spawn(&mut command).unwrap();
    let stderr = owned.take_stderr().unwrap();
    let (ready, received) = std::sync::mpsc::channel();
    let draining = thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let line = line.unwrap();
            if line.contains("Hive running at") {
                let _ = ready.send(());
            }
        }
    });
    received
        .recv_timeout(Duration::from_secs(10))
        .expect("Actual sidecar failed to start");
    // No initialize or other desktop request has been written. The real Node
    // startup has opened Hive HTTP/SQLite; there must still be no agent children.
    let mut children = [0i32; 32];
    let count = unsafe {
        libc::proc_listchildpids(
            owned.id() as i32,
            children.as_mut_ptr().cast(),
            std::mem::size_of_val(&children) as i32,
        )
    };
    assert_eq!(count, 0, "Sidecar boot spawned children before initialize");
    owned
        .take_stdin()
        .unwrap()
        .write_all(b"{\"id\":1,\"method\":\"shutdown\",\"params\":{}}\n")
        .unwrap();
    let report = owned.cleanup(Duration::from_secs(3)).unwrap();
    assert!(!report.forced);
    draining.join().unwrap();
    fs::remove_dir_all(directory).unwrap();
}
