use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Deserialize)]
pub(crate) struct Installer {
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
    let path = data_directory.join("desktop-updates/install-result.json");
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 16_384 {
        return None;
    }
    serde_json::from_reader(file).ok()
}
