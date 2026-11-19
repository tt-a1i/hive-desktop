//! Detached Pi shells and saved-script PTYs have their own process groups.
//! On macOS those groups need to observe the Node owner's exit independently.
use std::ffi::OsString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

extern "C" fn keep_watching(_: libc::c_int) {}

pub fn run_if_requested() {
    let mut args = std::env::args_os().skip(1);
    let pty = match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("--hive-owned-shell") => false,
        Some("--hive-owned-script") => true,
        _ => return,
    };
    let result = run(args.collect(), pty);
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Hive shell supervision failed: {error}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: Vec<OsString>, pty: bool) -> io::Result<i32> {
    let parent: libc::pid_t = args
        .first()
        .and_then(|s| s.to_str())
        .and_then(|s| s.parse().ok())
        .filter(|pid| *pid > 1)
        .ok_or_else(|| io::Error::other("missing Node owner PID"))?;
    let shell = args
        .get(1)
        .ok_or_else(|| io::Error::other("missing shell executable"))?;
    // Require a private group created by the detached spawn or PTY.
    if unsafe { libc::getpgrp() != libc::getpid() || libc::getppid() != parent } {
        return Err(io::Error::other(
            "shell has no matching detached Node owner",
        ));
    }
    if pty {
        // A closed PTY sends SIGHUP. Stop also signals the whole group with
        // SIGTERM. Keep supervising until the shell exits or Node is gone.
        // These caught handlers reset on exec, preserving shell defaults.
        for signal in [libc::SIGHUP, libc::SIGTERM] {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = keep_watching as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0
                || unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0
            {
                return Err(io::Error::last_os_error());
            }
        }
    }
    let queue = parent_exit_queue(parent)?;
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
    if let Some(signal) = status.signal() {
        if pty {
            // Report the same native signal through node-pty as a direct shell.
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
            }
        }
        return Ok(128 + signal);
    }
    Ok(status.code().unwrap_or(1))
}

/// Register before rechecking the direct parent so a death during startup cannot
/// leave an executing private process group without its owner.
pub(crate) fn parent_exit_queue(parent: libc::pid_t) -> io::Result<OwnedFd> {
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
    Ok(queue)
}
