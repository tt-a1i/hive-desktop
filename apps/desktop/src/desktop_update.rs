use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Deserialize)]
pub(crate) struct Installer {
    pub(crate) id: String,
    executable: PathBuf,
    program: PathBuf,
    pub(crate) version: String,
    pub(crate) sha256: String,
}

impl Installer {
    pub(crate) fn start(self) -> std::io::Result<()> {
        let mut command = Command::new(self.executable);
        let directory = self
            .program
            .parent()
            .ok_or_else(|| std::io::Error::other("Missing installer directory"))?;
        command
            .current_dir(directory)
            .arg(&self.program)
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map(|_| ())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateLaunch {
    state: String,
    to_version: String,
    candidate_pid: Option<u32>,
}

pub(crate) fn authorize_candidate_startup() -> std::io::Result<()> {
    if std::env::var_os("HIVE_DESKTOP_UPDATE_AUTHORIZE").as_deref()
        != Some(std::ffi::OsStr::new("1"))
    {
        return Ok(());
    }
    let Some(path) = std::env::var_os("HIVE_DESKTOP_UPDATE_READY").map(PathBuf::from) else {
        return Err(std::io::Error::other("Missing update startup path"));
    };
    if path.file_name().and_then(|name| name.to_str()) != Some("ready.json") {
        return Err(std::io::Error::other("Invalid update startup path"));
    }
    let journal = path.with_file_name("install-status.json");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Some(record) = read_small_json::<CandidateLaunch>(&journal)
            && record.state == "starting"
            && record.to_version == env!("CARGO_PKG_VERSION")
            && record.candidate_pid == Some(std::process::id())
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(std::io::Error::other(
                "Desktop installer did not confirm startup",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub(crate) fn report_startup() -> std::io::Result<()> {
    use std::io::Write;
    let Some(path) = std::env::var_os("HIVE_DESKTOP_UPDATE_READY").map(PathBuf::from) else {
        return Ok(());
    };
    if path.file_name().and_then(|name| name.to_str()) != Some("ready.json") || path.exists() {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Invalid update startup path"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(
        &mut temporary,
        &serde_json::json!({ "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION") }),
    )?;
    temporary.flush()?;
    temporary.persist_noclobber(path)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallReceipt {
    pub(crate) state: String,
    pub(crate) version: String,
    pub(crate) requested_version: String,
    pub(crate) error: Option<String>,
}

pub(crate) fn read_receipt(data_directory: &Path) -> Option<InstallReceipt> {
    read_small_json(&data_directory.join("desktop-updates/install-result.json"))
}

fn read_small_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    serde_json::from_reader(file.take(16_384)).ok()
}
