//! Sidecar ownership only. This does not adopt terminals or claim detached macOS processes.
use std::{
    io,
    process::{Child, ChildStdin, ChildStdout, Command},
    thread,
    time::{Duration, Instant},
};
#[cfg(target_os = "macos")]
#[path = "process_supervisor/macos.rs"]
mod macos;
#[cfg(target_os = "macos")]
use macos::Scope;
#[cfg(windows)]
#[path = "process_supervisor/windows.rs"]
mod windows;
#[cfg(windows)]
use windows::Scope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupReport {
    pub forced: bool,
    pub scope: &'static str,
}
pub struct OwnedProcess {
    child: Child,
    scope: Scope,
    cleaned: Option<CleanupReport>,
    forced: bool,
}
impl OwnedProcess {
    /// Windows callers gate descendant startup until this returns (the Node
    /// sidecar waits for desktop requests); this is not a suspended generic launcher.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut scope = Scope::prepare(command)?;
        let mut child = command.spawn()?;
        if let Err(error) = scope.attach(&child) {
            // Before initialize, the sidecar has not been asked to start agents.
            // kill()/wait() use the still-owned process handle, never a looked-up PID.
            let kill = child.kill();
            let wait = child.wait();
            return Err(io::Error::other(format!(
                "Could not bind owned sidecar: {error}; leader kill: {kill:?}; reap: {wait:?}"
            )));
        }
        Ok(Self {
            child,
            scope,
            cleaned: None,
            forced: false,
        })
    }
    #[cfg(test)]
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }
    #[cfg(test)]
    pub fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }
    pub fn leader_exited(&self) -> io::Result<bool> {
        self.scope.leader_exited(&self.child)
    }
    pub fn is_cleaned(&self) -> bool {
        self.cleaned.is_some()
    }
    pub fn cleanup(&mut self, grace: Duration) -> Result<CleanupReport, String> {
        if let Some(report) = &self.cleaned {
            return Ok(report.clone());
        }
        let report = self
            .cleanup_inner(grace)
            .map_err(|error| error.to_string())?;
        self.cleaned = Some(report.clone());
        Ok(report)
    }
    fn cleanup_inner(&mut self, grace: Duration) -> io::Result<CleanupReport> {
        let deadline = Instant::now() + grace;
        let normal = loop {
            if self.scope.leader_exited(&self.child)? && self.scope.descendants_gone(&self.child)? {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            thread::sleep(Duration::from_millis(10));
        };
        if !normal {
            // Preserve forced cleanup across a failed confirmation and retry.
            self.forced = true;
            self.scope.terminate()?;
        }
        // Keep the leader unreaped through every signal to its process group.
        // An unreaped leader reserves the numeric PID/PGID even after an abnormal exit.
        let deadline = Instant::now() + Duration::from_secs(3);
        while !(self.scope.leader_exited(&self.child)?
            && self.scope.descendants_gone(&self.child)?)
        {
            if Instant::now() >= deadline {
                return Err(io::Error::other(format!(
                    "Could not confirm owned sidecar scope exited: {}",
                    Scope::DESCRIPTION
                )));
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.child.wait()?;
        Ok(CleanupReport {
            forced: self.forced,
            scope: Scope::DESCRIPTION,
        })
    }
}
#[cfg(test)]
#[path = "process_supervisor/tests.rs"]
mod tests;
