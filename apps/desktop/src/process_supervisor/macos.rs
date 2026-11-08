use std::os::unix::process::CommandExt;
use std::{
    io, mem,
    process::{Child, Command},
};

pub struct Scope {
    pgid: libc::pid_t,
}
impl Scope {
    pub fn prepare(command: &mut Command) -> io::Result<Self> {
        command.process_group(0);
        Ok(Self { pgid: 0 })
    }
    pub fn attach(&mut self, child: &Child) -> io::Result<()> {
        self.pgid = child.id() as libc::pid_t;
        // Child is never reaped until cleanup has finished sending group signals.
        if unsafe { libc::getpgid(self.pgid) } != self.pgid {
            return Err(io::Error::other(
                "Owned sidecar did not enter its private process group",
            ));
        }
        Ok(())
    }
    pub fn leader_exited(&self, child: &Child) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info.si_pid != 0)
    }
    fn members(&self) -> io::Result<Vec<libc::pid_t>> {
        let mut slots = 64usize;
        loop {
            let mut pids = vec![0; slots];
            let capacity = (slots * mem::size_of::<libc::pid_t>()) as i32;
            unsafe {
                *libc::__error() = 0;
            }
            let count =
                unsafe { libc::proc_listpgrppids(self.pgid, pids.as_mut_ptr().cast(), capacity) };
            if count < 0
                || (count == 0
                    && io::Error::last_os_error()
                        .raw_os_error()
                        .is_some_and(|code| code != 0 && code != libc::ESRCH))
            {
                return Err(io::Error::last_os_error());
            }
            if (count as usize) < slots {
                pids.truncate(count as usize);
                pids.retain(|pid| *pid > 0);
                return Ok(pids);
            }
            slots = slots
                .checked_mul(2)
                .ok_or_else(|| io::Error::other("Process group enumeration overflow"))?;
        }
    }
    pub fn descendants_gone(&self, child: &Child) -> io::Result<bool> {
        Ok(self.members()?.iter().all(|pid| *pid == child.id() as i32))
    }
    pub fn terminate(&self, child: &Child) -> io::Result<()> {
        if unsafe { libc::killpg(self.pgid, libc::SIGKILL) } == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            // macOS returns EPERM when the group only contains the unreaped,
            // exited leader. Recheck the actual scope after this signal race.
            if error.raw_os_error() == Some(libc::EPERM)
                && self.leader_exited(child)?
                && self.descendants_gone(child)?
            {
                return Ok(());
            }
            return Err(error);
        }
        Ok(())
    }
    pub const DESCRIPTION: &'static str = "owned macOS process group; descendants that deliberately leave the group are outside this boundary";
}
