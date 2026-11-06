use std::{
    io, mem,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command},
};
use windows_sys::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{CREATE_NO_WINDOW, WaitForSingleObject},
    },
};

pub struct Scope {
    job: OwnedHandle,
}
impl Scope {
    pub fn prepare(command: &mut Command) -> io::Result<Self> {
        command.creation_flags(CREATE_NO_WINDOW);
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(handle) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // No breakaway flags. Ordinary descendants remain in this unnamed owned Job.
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { job })
    }
    fn handle(&self) -> HANDLE {
        self.job.as_raw_handle()
    }
    pub fn attach(&mut self, child: &Child) -> io::Result<()> {
        // Production Node receives no desktop requests until this returns. Its
        // boot path opens Hive stores/HTTP only, never automatically starts agents.
        if unsafe { AssignProcessToJobObject(self.handle(), child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn leader_exited(&self, child: &Child) -> io::Result<bool> {
        match unsafe { WaitForSingleObject(child.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
    #[cfg(test)]
    pub fn contains(&self, process: HANDLE) -> io::Result<bool> {
        let mut contained = 0;
        if unsafe {
            windows_sys::Win32::System::JobObjects::IsProcessInJob(
                process,
                self.handle(),
                &mut contained,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(contained != 0)
    }
    pub fn descendants_gone(&self, _child: &Child) -> io::Result<bool> {
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { mem::zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.handle(),
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                mem::size_of_val(&info) as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(info.ActiveProcesses == 0)
    }
    pub fn terminate(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub const DESCRIPTION: &'static str =
        "owned Windows Job; no breakaway allowed, active process count verified zero";
}
