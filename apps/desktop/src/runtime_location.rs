use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub struct RuntimeLocation {
    pub entry: PathBuf,
    pub node: OsString,
    pub cwd: PathBuf,
    pub node_directory: Option<PathBuf>,
}

/// Installed packages carry their runtime next to the executable. Environment
/// overrides remain available for development and explicit user configuration.
pub fn locate_runtime(
    executable: &Path,
    development_root: &Path,
    entry_override: Option<OsString>,
    node_override: Option<OsString>,
) -> Result<RuntimeLocation, String> {
    let parent = executable.parent().ok_or("应用路径没有父目录")?;
    let mac_bundle = parent.file_name().is_some_and(|name| name == "MacOS")
        && parent
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "Contents");
    let resources = if mac_bundle {
        Some(parent.parent().unwrap().join("Resources"))
    } else {
        let candidate = parent.join("resources");
        candidate.is_dir().then_some(candidate)
    };
    let cwd = resources
        .as_ref()
        .map(|path| path.join("runtime"))
        .unwrap_or_else(|| development_root.to_path_buf());
    let entry = entry_override.map(PathBuf::from).unwrap_or_else(|| {
        if resources.is_some() {
            cwd.join("dist/main.js")
        } else {
            cwd.join("packages/runtime/dist/main.js")
        }
    });
    if !entry.is_file() {
        return Err(format!(
            "找不到后台 {}，请重新安装应用；源码运行请先执行 pnpm build:runtime",
            entry.display()
        ));
    }
    let bundled_node = resources.map(|path| {
        if mac_bundle {
            path.join("node/bin/node")
        } else {
            path.join("node/node.exe")
        }
    });
    let node_directory = bundled_node
        .as_ref()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf);
    let node = match node_override {
        Some(node) => node,
        None => match bundled_node {
            Some(node) => {
                if !node.is_file() {
                    return Err(format!(
                        "找不到应用内的 Node {}，请重新安装应用",
                        node.display()
                    ));
                }
                node.into_os_string()
            }
            None => "node".into(),
        },
    };
    Ok(RuntimeLocation {
        entry,
        node,
        cwd,
        node_directory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "hive-runtime-location-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn file(&self, path: &str) -> PathBuf {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"fixture").unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn mac_bundle_uses_its_own_runtime() {
        let f = Fixture::new();
        let exe = f.file("Hive Desktop.app/Contents/MacOS/hive-desktop");
        let entry = f.file("Hive Desktop.app/Contents/Resources/runtime/dist/main.js");
        let node = f.file("Hive Desktop.app/Contents/Resources/node/bin/node");
        let location = locate_runtime(&exe, Path::new("/unavailable/source"), None, None).unwrap();
        assert_eq!(location.entry, entry);
        assert_eq!(Path::new(&location.node), node.as_path());
        assert_eq!(location.cwd, entry.parent().unwrap().parent().unwrap());
    }
    #[test]
    fn windows_portable_uses_its_own_runtime() {
        let f = Fixture::new();
        let exe = f.file("portable/hive-desktop.exe");
        let entry = f.file("portable/resources/runtime/dist/main.js");
        let node = f.file("portable/resources/node/node.exe");
        let location = locate_runtime(&exe, &f.0, None, None).unwrap();
        assert_eq!(location.entry, entry);
        assert_eq!(Path::new(&location.node), node.as_path());
    }
    #[test]
    fn broken_bundle_does_not_fall_back_to_development() {
        let f = Fixture::new();
        let exe = f.file("Hive.app/Contents/MacOS/hive-desktop");
        f.file("source/packages/runtime/dist/main.js");
        assert!(locate_runtime(&exe, &f.0.join("source"), None, None).is_err());
        f.file("Hive.app/Contents/Resources/runtime/dist/main.js");
        assert!(locate_runtime(&exe, &f.0.join("source"), None, None).is_err());
    }
    #[test]
    fn development_and_explicit_overrides_remain_available() {
        let f = Fixture::new();
        let exe = f.file("target/debug/hive-desktop");
        let entry = f.file("source/packages/runtime/dist/main.js");
        let location = locate_runtime(&exe, &f.0.join("source"), None, None).unwrap();
        assert_eq!(location.entry, entry);
        assert_eq!(location.node, "node");
        let custom = f.file("custom/main.js");
        let location = locate_runtime(
            &exe,
            &f.0,
            Some(custom.clone().into_os_string()),
            Some("custom-node".into()),
        )
        .unwrap();
        assert_eq!(location.entry, custom);
        assert_eq!(location.node, "custom-node");
    }
}
