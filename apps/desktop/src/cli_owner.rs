//! A detached native host watches the Node parent independently of the desktop
//! sidecar group. Its separately owned CLI group is cleaned before the host exits.
use crate::{process_supervisor::OwnedProcess, shell_owner::parent_exit_queue};
use std::{
    ffi::OsString,
    io,
    os::{fd::AsRawFd, unix::process::ExitStatusExt},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn request_stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

pub fn run_if_requested() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref().and_then(|arg| arg.to_str()) != Some("--hive-owned-cli") {
        return;
    }
    let code = match run(args.collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Hive probe supervision failed: {error}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: Vec<OsString>) -> io::Result<i32> {
    let parent: libc::pid_t = args
        .first()
        .and_then(|arg| arg.to_str())
        .and_then(|arg| arg.parse().ok())
        .filter(|pid| *pid > 1)
        .ok_or_else(|| io::Error::other("missing Node owner PID"))?;
    let executable = args
        .get(1)
        .ok_or_else(|| io::Error::other("missing probe executable"))?;
    if unsafe { libc::getppid() != parent || libc::getpgrp() != libc::getpid() } {
        return Err(io::Error::other("probe has no matching Node owner"));
    }
    let queue = parent_exit_queue(parent)?;
    // Node sends TERM after closing stdin. Keep this host alive until its owned
    // scope is gone; caught handlers reset in the executed CLI.
    for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = request_stop as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0
            || unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    let mut command = Command::new(executable);
    command
        .args(&args[2..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut owned = OwnedProcess::spawn(&mut command)?;
    loop {
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 10_000_000,
        };
        let count = unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &timeout,
            )
        };
        let watcher_failed =
            count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted;
        let leader_exited = match owned.leader_exited() {
            Ok(exited) => exited,
            Err(error) => {
                owned.cleanup(Duration::ZERO).map_err(io::Error::other)?;
                return Err(error);
            }
        };
        if count > 0 || watcher_failed || STOP.load(Ordering::Relaxed) || leader_exited {
            // cleanup keeps the CLI leader unreaped until every group signal is
            // finished, then records its original exit status. Descendants may
            // hold inherited output pipes after that leader has already exited.
            owned.cleanup(Duration::ZERO).map_err(io::Error::other)?;
            let status = owned
                .exit_status()
                .ok_or_else(|| io::Error::other("probe exit status unavailable"))?;
            return Ok(status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)));
        }
    }
}
