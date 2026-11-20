use super::OpenTarget;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn supports(target: OpenTarget) -> bool {
    target == OpenTarget::Finder
}

pub(super) fn locate(_: OpenTarget) -> Option<PathBuf> {
    let path = PathBuf::from("/usr/bin/xdg-open");
    path.is_file().then_some(path)
}

pub(super) fn icon(_: OpenTarget, _: &Path) -> Option<image::DynamicImage> {
    None
}

pub(super) fn open(_: OpenTarget, app: &Path, path: &Path) -> Result<(), String> {
    let result = Command::new(app)
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).trim().to_owned())
    }
}
