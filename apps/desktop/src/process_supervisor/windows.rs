use std::{
    io, mem,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, ERROR_MORE_DATA, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectBasicProcessIdList,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            WaitForSingleObject,
        },
    },
};

pub struct Scope {
    job: OwnedHandle,
    terminating_members: Vec<OwnedHandle>,
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
        Ok(Self {
            job,
            terminating_members: Vec::new(),
        })
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
    pub fn contains(&self, process: HANDLE) -> io::Result<bool> {
        let mut contained = 0;
        if unsafe { IsProcessInJob(process, self.handle(), &mut contained) } == 0 {
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
        if info.ActiveProcesses != 0 {
            return Ok(false);
        }
        // A failed termination confirmation must not become a false success on
        // retry merely because Job accounting already dropped these members.
        for member in &self.terminating_members {
            match unsafe { WaitForSingleObject(member.as_raw_handle(), 0) } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => return Ok(false),
                _ => return Err(io::Error::last_os_error()),
            }
        }
        Ok(true)
    }
    pub fn terminate(&mut self, _child: &Child) -> io::Result<()> {
        // Capture the original objects before termination removes members from
        // Job accounting. ActiveProcesses can reach zero before their process
        // handles become signaled (observed on Windows CI after leader exit).
        self.terminating_members.extend(self.member_handles()?);
        if unsafe { TerminateJobObject(self.handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        for member in &self.terminating_members {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let milliseconds = remaining
                .as_millis()
                .saturating_add(1)
                .min(u32::MAX as u128) as u32;
            match unsafe { WaitForSingleObject(member.as_raw_handle(), milliseconds) } {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Owned Job member did not finish terminating",
                    ));
                }
                _ => return Err(io::Error::last_os_error()),
            }
        }
        Ok(())
    }
    fn member_handles(&self) -> io::Result<Vec<OwnedHandle>> {
        // usize storage gives the variable-length Win32 structure its required
        // alignment on both Windows architectures. The header is two DWORDs.
        let mut storage = vec![0usize; 32];
        loop {
            let info = storage
                .as_mut_ptr()
                .cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>();
            let success = unsafe {
                QueryInformationJobObject(
                    self.handle(),
                    JobObjectBasicProcessIdList,
                    info.cast(),
                    std::mem::size_of_val(storage.as_slice()) as u32,
                    std::ptr::null_mut(),
                )
            };
            if success == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_MORE_DATA as i32) {
                    storage.resize(storage.len() * 2, 0);
                    continue;
                }
                return Err(error);
            }
            if unsafe { (*info).NumberOfProcessIdsInList < (*info).NumberOfAssignedProcesses } {
                storage.resize(storage.len() * 2, 0);
                continue;
            }
            let ids = unsafe {
                std::slice::from_raw_parts(
                    (*info).ProcessIdList.as_ptr(),
                    (*info).NumberOfProcessIdsInList as usize,
                )
            };
            let mut handles = Vec::with_capacity(ids.len());
            for &pid in ids {
                let handle = unsafe {
                    OpenProcess(
                        PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                        0,
                        pid as u32,
                    )
                };
                if handle.is_null() {
                    let error = io::Error::last_os_error();
                    // The member may have exited between the Job query and open.
                    if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                        continue;
                    }
                    return Err(error);
                }
                let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
                // A recycled numeric PID must never become an exit witness for
                // this Job. No process is terminated through these handles.
                if self.contains(handle.as_raw_handle())? {
                    handles.push(handle);
                }
            }
            return Ok(handles);
        }
    }
    pub const DESCRIPTION: &'static str = "owned Windows Job; no breakaway allowed, active process count zero; forced member handles signaled";
}
