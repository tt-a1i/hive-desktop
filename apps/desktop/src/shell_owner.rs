//! Pi starts each shell in its own process group for cancellation. On macOS that
//! group needs its own parent-exit watcher; it is outside the Node owner's group.
use std::ffi::OsString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

const MODE: &str = "--hive-owned-shell";

pub fn run_if_requested() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(MODE)) {
        return;
    }
    let result = run(args.collect());
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Hive shell supervision failed: {error}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: Vec<OsString>) -> io::Result<i32> {
    let parent: libc::pid_t = args
        .first()
        .and_then(|s| s.to_str())
        .and_then(|s| s.parse().ok())
        .filter(|pid| *pid > 1)
        .ok_or_else(|| io::Error::other("missing Node owner PID"))?;
    let shell = args
        .get(1)
        .ok_or_else(|| io::Error::other("missing shell executable"))?;
    // Only kill a group created specifically for this helper by Pi's detached
    // spawn. Never signal a group inherited from a terminal or desktop window.
    if unsafe { libc::getpgrp() != libc::getpid() || libc::getppid() != parent } {
        return Err(io::Error::other(
            "shell has no matching detached Node owner",
        ));
    }
    let fd = unsafe { libc::kqueue() };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let queue = unsafe { OwnedFd::from_raw_fd(fd) };
    let change = libc::kevent {
        ident: parent as usize,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    if unsafe {
        libc::kevent(
            queue.as_raw_fd(),
            &change,
            1,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    // Register first, then recheck: a parent death during registration must not
    // allow the command to start unobserved.
    if unsafe { libc::getppid() } != parent {
        return Err(io::Error::other("Node owner exited before shell startup"));
    }
    std::thread::Builder::new()
        .name("shell-owner".into())
        .spawn(move || {
            loop {
                let mut event: libc::kevent = unsafe { std::mem::zeroed() };
                let count = unsafe {
                    libc::kevent(
                        queue.as_raw_fd(),
                        std::ptr::null(),
                        0,
                        &mut event,
                        1,
                        std::ptr::null(),
                    )
                };
                if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                // Fail closed on a watcher error as well as parent exit. This helper
                // stays group leader, so the negative PID cannot name another group.
                unsafe {
                    libc::killpg(libc::getpid(), libc::SIGKILL);
                }
                std::process::exit(1);
            }
        })?;
    let mut child = Command::new(shell)
        .args(&args[2..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let status = match child.wait() {
        Ok(status) => status,
        Err(_) => {
            // A failed wait must not leave an unowned executing command.
            unsafe {
                libc::killpg(libc::getpid(), libc::SIGKILL);
            }
            std::process::exit(1);
        }
    };
    Ok(status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)))
}
