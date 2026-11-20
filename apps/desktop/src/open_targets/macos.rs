use super::{OpenTarget, OpenTarget::*};
use objc2::{AnyThread, rc::autoreleasepool};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn bundles(target: OpenTarget) -> &'static [&'static str] {
    match target {
        Finder => &["com.apple.finder"],
        Devin => &["com.cognition.devin", "com.exafunction.windsurf"],
        Vscode => &["com.microsoft.VSCode"],
        VscodeInsiders => &["com.microsoft.VSCodeInsiders"],
        Cursor => &["com.todesktop.230313mzl4w4u92"],
        Zed => &["dev.zed.Zed"],
        Windsurf => &["com.exafunction.windsurf"],
        Antigravity => &["com.google.antigravity"],
        Qoder => &["com.qoder.app"],
        SublimeText => &["com.sublimetext.4", "com.sublimetext.3"],
        IntelliJ => &["com.jetbrains.intellij", "com.jetbrains.intellij.ce"],
        PyCharm => &["com.jetbrains.pycharm", "com.jetbrains.pycharm.ce"],
        WebStorm => &["com.jetbrains.WebStorm"],
        GoLand => &["com.jetbrains.goland"],
        RustRover => &["com.jetbrains.rustrover"],
        Rider => &["com.jetbrains.rider"],
        CLion => &["com.jetbrains.CLion"],
        AndroidStudio => &["com.google.android.studio"],
        Xcode => &["com.apple.dt.Xcode"],
        Terminal => &["com.apple.Terminal"],
        ITerm => &["com.googlecode.iterm2"],
        Ghostty => &["com.mitchellh.ghostty"],
        Warp => &["dev.warp.Warp-Stable"],
        Cmux => &["com.cmuxterm.app"],
        WezTerm => &["com.github.wez.wezterm"],
        Kitty => &["net.kovidgoyal.kitty"],
        Alacritty => &["org.alacritty"],
        WindowsTerminal => &[],
    }
}

pub(super) fn supports(target: OpenTarget) -> bool {
    !bundles(target).is_empty()
}

fn matches_bundle(target: OpenTarget, path: &Path) -> bool {
    // Devin Desktop and Windsurf distributions have shipped the same bundle ID.
    let executables = path.join("Contents/MacOS");
    match target {
        Devin => executables.join("Devin").is_file(),
        Windsurf => executables.join("Windsurf").is_file(),
        _ => path.is_dir(),
    }
}

pub(super) fn locate(target: OpenTarget) -> Option<PathBuf> {
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        for bundle in bundles(target) {
            let id = NSString::from_str(bundle);
            let urls = workspace.URLsForApplicationsWithBundleIdentifier(&id);
            if let Some(path) = urls
                .iter()
                .filter_map(|url| url.path())
                .map(|path| PathBuf::from(path.to_string()))
                .find(|path| matches_bundle(target, path))
            {
                return Some(path);
            }
        }
        let name = match target {
            Devin => "Devin.app",
            Windsurf => "Windsurf.app",
            Finder => return Some(PathBuf::from("/System/Library/CoreServices/Finder.app")),
            Terminal => return Some(PathBuf::from("/System/Applications/Utilities/Terminal.app")),
            _ => return None,
        };
        let home = std::env::home_dir();
        [
            Some(PathBuf::from("/Applications")),
            home.map(|home| home.join("Applications")),
        ]
        .into_iter()
        .flatten()
        .map(|directory| directory.join(name))
        .find(|path| matches_bundle(target, path))
    })
}

pub(super) fn icon(_: OpenTarget, path: &Path) -> Option<image::DynamicImage> {
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let icon = workspace.iconForFile(&NSString::from_str(path.to_str()?));
        let mut bounds = NSRect::new(NSPoint::new(0., 0.), NSSize::new(48., 48.));
        // The destination rectangle is initialized and no drawing hints are needed.
        let small_icon =
            unsafe { icon.CGImageForProposedRect_context_hints(&mut bounds, None, None) }?;
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &small_icon);
        // An empty dictionary supplies no format-specific properties.
        let png = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        let image =
            image::load_from_memory_with_format(&png.to_vec(), image::ImageFormat::Png).ok()?;
        Some(image.thumbnail(48, 48))
    })
}

fn xcode_document(folder: &Path) -> Option<PathBuf> {
    if matches!(
        folder.extension().and_then(|extension| extension.to_str()),
        Some("xcodeproj" | "xcworkspace")
    ) {
        return Some(folder.to_owned());
    }
    let package = folder.join("Package.swift");
    let entries = fs::read_dir(folder).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            let rank = match path.extension()?.to_str()? {
                "xcworkspace" => 0,
                "xcodeproj" => 1,
                _ => return None,
            };
            Some((rank, path.file_stem() != folder.file_name(), path))
        })
        .min()
        .map(|(_, _, path)| path)
        .or_else(|| package.is_file().then_some(package))
}

fn launch(target: OpenTarget, app: &Path, project: &Path) -> Result<Command, String> {
    let mut command = Command::new("/usr/bin/open");
    command.arg("-a").arg(app);
    match target {
        WezTerm => {
            command
                .args(["-n", "--args", "start", "--cwd"])
                .arg(project);
        }
        Alacritty => {
            command
                .args(["-n", "--args", "--working-directory"])
                .arg(project);
        }
        Kitty => {
            command.args(["-n", "--args", "--directory"]).arg(project);
        }
        IntelliJ | PyCharm | WebStorm | GoLand | RustRover | Rider | CLion | AndroidStudio => {
            command.args(["-n", "--args"]).arg(project);
        }
        Warp => {
            let mut url = url::Url::parse("warp://action/new_window").expect("static Warp URL");
            url.query_pairs_mut()
                .append_pair("path", &project.to_string_lossy());
            command.arg(url.as_str().replace('+', "%20"));
        }
        Xcode => {
            let document = xcode_document(project).ok_or_else(|| {
                crate::language::tr("此文件夹中没有 Xcode 工程、工作区或 Swift 软件包").to_owned()
            })?;
            command.arg("--").arg(document);
        }
        _ => {
            command.arg("--").arg(project);
        }
    }
    Ok(command)
}

pub(super) fn open(target: OpenTarget, app: &Path, folder: &Path) -> Result<(), String> {
    let result = launch(target, app, folder)?
        .output()
        .map_err(|error| error.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_targets_have_application_icons() {
        for target in [Finder, Terminal] {
            let bundle = locate(target).expect("system app bundle");
            let image = icon(target, &bundle)
                .unwrap_or_else(|| panic!("missing icon for {target:?}: {bundle:?}"));
            assert!(image.width() >= 16 && image.height() >= 16);
            assert!(image.to_rgba8().pixels().any(|pixel| pixel[3] > 0));
        }
    }

    #[test]
    fn passes_project_paths_as_one_native_argument() {
        let app = Path::new("/Applications/Visual Studio Code.app");
        let project = Path::new("/tmp/my project & ; 空格");
        let command = launch(Vscode, app, project).unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            ["-a", app.to_str().unwrap(), "--", project.to_str().unwrap()]
        );
    }

    #[test]
    fn xcode_prefers_a_matching_workspace_and_supports_swift_packages() {
        let parent = tempfile::tempdir().unwrap();
        let folder = parent.path().join("Demo app");
        fs::create_dir(&folder).unwrap();
        assert!(xcode_document(&folder).is_none());
        let package = folder.join("Package.swift");
        fs::write(&package, "").unwrap();
        assert_eq!(xcode_document(&folder), Some(package));
        fs::create_dir(folder.join("Demo app.xcodeproj")).unwrap();
        fs::create_dir(folder.join("All.xcworkspace")).unwrap();
        let workspace = folder.join("Demo app.xcworkspace");
        fs::create_dir(&workspace).unwrap();
        assert_eq!(xcode_document(&folder), Some(workspace.clone()));
        let command = launch(Xcode, Path::new("/Applications/Xcode.app"), &folder).unwrap();
        assert_eq!(command.get_args().last(), Some(workspace.as_os_str()));
    }
}
