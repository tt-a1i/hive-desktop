use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

fn build_identity() {
    println!("cargo:rerun-if-changed=build.rs");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut revision = None;
    let mut dirty = None;
    // An exported source tree can live inside another checkout. Its parent's
    // HEAD must never be presented as the desktop's source identity.
    let owns_repository = git(&root, &["rev-parse", "--show-toplevel"])
        .and_then(|path| PathBuf::from(path.trim()).canonicalize().ok())
        .zip(root.canonicalize().ok())
        .is_some_and(|(repository, source)| repository == source);
    if owns_repository {
        revision = git(&root, &["rev-parse", "--verify", "HEAD"])
            .map(|value| value.trim().to_owned())
            .filter(|value| {
                (40..=64).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit())
            });
        dirty = git(&root, &["status", "--porcelain", "--untracked-files=no"])
            .map(|value| !value.trim().is_empty());
        for name in ["HEAD", "index", "packed-refs"] {
            if let Some(path) = git(&root, &["rev-parse", "--git-path", name]) {
                let path = root.join(path.trim());
                if path.exists() {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
        if let Some(reference) = git(&root, &["symbolic-ref", "-q", "HEAD"])
            && let Some(path) = git(&root, &["rev-parse", "--git-path", reference.trim()])
        {
            println!(
                "cargo:rerun-if-changed={}",
                root.join(path.trim()).display()
            );
        }
        // The marker describes tracked-file edits, as with git describe --dirty.
        // Untracked scratch files do not affect it. Refresh after an edit or commit.
        // Watching tracked files avoids including target/ or other build caches.
        if let Some(files) = git(&root, &["ls-files", "-z"]) {
            for file in files.split('\0').filter(|file| !file.is_empty()) {
                println!("cargo:rerun-if-changed={}", root.join(file).display());
            }
        }
    } else {
        println!("cargo:rerun-if-changed={}", root.join(".git").display());
    }
    println!(
        "cargo:rustc-env=HIVE_DESKTOP_GIT_REVISION={}",
        revision.as_deref().unwrap_or("unknown")
    );
    println!(
        "cargo:rustc-env=HIVE_DESKTOP_GIT_DIRTY={}",
        dirty
            .map(|value| if value { "true" } else { "false" })
            .unwrap_or("unknown")
    );
    println!(
        "cargo:rustc-env=HIVE_DESKTOP_BUILD_PROFILE={}",
        env::var("PROFILE").unwrap_or_else(|_| "unknown".into())
    );
    println!(
        "cargo:rustc-env=HIVE_DESKTOP_BUILD_TARGET={}",
        env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
}

fn main() {
    build_identity();
    // Expanding a GPUI tool group overflows the default Windows main-thread
    // stack in the packaged debug executable. Reserve space for the UI thread;
    // the OS commits pages as they are used.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let stack = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            "/STACK:8388608"
        } else {
            "-Wl,--stack,8388608"
        };
        println!("cargo:rustc-link-arg-bin=hive-desktop={stack}");
    }
}
