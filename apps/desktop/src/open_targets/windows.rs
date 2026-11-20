use super::{OpenTarget, OpenTarget::*};
use std::{
    env,
    ffi::OsString,
    fs,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    process::Command,
    ptr, slice,
};
use windows_sys::Win32::{
    Graphics::Gdi::*,
    System::{
        Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        Registry::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
            RegGetValueW,
        },
    },
    UI::{
        Shell::{
            SHFILEINFOW, SHGFI_ICON, SHGFI_PIDL, SHGFI_SMALLICON, SHGetFileInfoW,
            SHParseDisplayName, ShellExecuteW,
        },
        WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON, SW_SHOWNORMAL},
    },
};

fn executable(target: OpenTarget) -> Option<(&'static str, &'static [&'static str])> {
    Some(match target {
        Finder => ("explorer.exe", &[]),
        WindowsTerminal => ("wt.exe", &["Microsoft/WindowsApps/wt.exe"]),
        Devin => (
            "Devin.exe",
            &["Programs/Devin/Devin.exe", "Devin/Devin.exe"],
        ),
        Vscode => (
            "Code.exe",
            &[
                "Programs/Microsoft VS Code/Code.exe",
                "Microsoft VS Code/Code.exe",
            ],
        ),
        VscodeInsiders => (
            "Code - Insiders.exe",
            &[
                "Programs/Microsoft VS Code Insiders/Code - Insiders.exe",
                "Microsoft VS Code Insiders/Code - Insiders.exe",
            ],
        ),
        Cursor => (
            "Cursor.exe",
            &["Programs/cursor/Cursor.exe", "Cursor/Cursor.exe"],
        ),
        Zed => ("zed.exe", &["Programs/Zed/Zed.exe", "Zed/Zed.exe"]),
        Windsurf => (
            "Windsurf.exe",
            &["Programs/Windsurf/Windsurf.exe", "Windsurf/Windsurf.exe"],
        ),
        Antigravity => (
            "Antigravity.exe",
            &[
                "Programs/Antigravity/Antigravity.exe",
                "Antigravity/Antigravity.exe",
            ],
        ),
        Qoder => (
            "Qoder.exe",
            &["Programs/Qoder/Qoder.exe", "Qoder/Qoder.exe"],
        ),
        SublimeText => (
            "sublime_text.exe",
            &[
                "Sublime Text/sublime_text.exe",
                "Sublime Text 3/sublime_text.exe",
            ],
        ),
        IntelliJ => ("idea64.exe", &[]),
        PyCharm => ("pycharm64.exe", &[]),
        WebStorm => ("webstorm64.exe", &[]),
        GoLand => ("goland64.exe", &[]),
        RustRover => ("rustrover64.exe", &[]),
        Rider => ("rider64.exe", &[]),
        CLion => ("clion64.exe", &[]),
        AndroidStudio => ("studio64.exe", &["Android/Android Studio/bin/studio64.exe"]),
        WezTerm => (
            "wezterm-gui.exe",
            &[
                "WezTerm/wezterm-gui.exe",
                "Programs/WezTerm/wezterm-gui.exe",
            ],
        ),
        Alacritty => (
            "alacritty.exe",
            &[
                "Alacritty/alacritty.exe",
                "Programs/Alacritty/alacritty.exe",
            ],
        ),
        _ => return None,
    })
}

pub(super) fn supports(target: OpenTarget) -> bool {
    executable(target).is_some()
}

fn jetbrains_exe(roots: &[PathBuf], executable: &str) -> Option<PathBuf> {
    roots
        .iter()
        .flat_map(|root| {
            [
                root.join("JetBrains"),
                root.join("Programs/JetBrains"),
                root.join("JetBrains/Toolbox/apps"),
            ]
        })
        .filter_map(|directory| fs::read_dir(directory).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("bin").join(executable))
        .filter(|candidate| candidate.is_file())
        .max()
}

fn registered_app(executable: &str) -> Option<PathBuf> {
    let key: Vec<_> = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{executable}")
        .encode_utf16()
        .chain([0])
        .collect();
    let mut path = vec![0u16; 32_768];
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut byte_count = (path.len() * size_of::<u16>()) as u32;
        // App Paths stores the full executable path as its default value.
        let result = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                ptr::null(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                ptr::null_mut(),
                path.as_mut_ptr().cast(),
                &mut byte_count,
            )
        };
        let len = byte_count as usize / size_of::<u16>();
        if result == 0
            && let Some(value) = path.get(..len)
        {
            let end = value.iter().position(|&unit| unit == 0).unwrap_or(len);
            let executable = PathBuf::from(OsString::from_wide(&value[..end]));
            if executable.is_absolute()
                && executable
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                && executable.is_file()
            {
                return Some(executable);
            }
        }
    }
    None
}

pub(super) fn locate(target: OpenTarget) -> Option<PathBuf> {
    let (exe, subpaths) = executable(target)?;
    if target == Finder {
        let explorer = PathBuf::from(env::var_os("SystemRoot")?).join(exe);
        return explorer.is_file().then_some(explorer);
    }
    let roots: Vec<_> = [
        "LOCALAPPDATA",
        "ProgramW6432",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ]
    .into_iter()
    .filter_map(env::var_os)
    .map(PathBuf::from)
    .filter(|root| root.is_absolute())
    .collect();

    roots
        .iter()
        .flat_map(|root| subpaths.iter().map(move |subpath| root.join(subpath)))
        .find(|candidate| candidate.is_file())
        .or_else(|| registered_app(exe))
        .or_else(|| {
            env::var_os("PATH").and_then(|search_path| {
                env::split_paths(&search_path)
                    .filter(|dir| dir.is_absolute())
                    .map(|dir| dir.join(exe))
                    .find(|candidate| candidate.is_file())
            })
        })
        .or_else(|| {
            matches!(
                target,
                IntelliJ | PyCharm | WebStorm | GoLand | RustRover | Rider | CLion
            )
            .then(|| jetbrains_exe(&roots, exe))
            .flatten()
        })
}

pub(super) fn icon(target: OpenTarget, path: &Path) -> Option<image::DynamicImage> {
    // Shell icon lookup on this background thread uses a COM apartment.
    let initialized = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) } >= 0;
    let rendered = if target == WindowsTerminal {
        terminal_app_icon().or_else(|| executable_icon(path))
    } else {
        executable_icon(path)
    };
    if initialized {
        unsafe { CoUninitialize() };
    }
    rendered
}

fn terminal_app_icon() -> Option<image::DynamicImage> {
    let app_name: Vec<_> = r"shell:AppsFolder\Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"
        .encode_utf16()
        .chain([0])
        .collect();
    let mut id = ptr::null_mut();
    let result = unsafe {
        SHParseDisplayName(
            app_name.as_ptr(),
            ptr::null_mut(),
            &mut id,
            0,
            ptr::null_mut(),
        )
    };
    if result < 0 || id.is_null() {
        return None;
    }
    let rendered = shell_icon(id.cast(), SHGFI_PIDL);
    unsafe { CoTaskMemFree(id.cast()) };
    rendered
}

fn executable_icon(path: &Path) -> Option<image::DynamicImage> {
    let file: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    shell_icon(file.as_ptr(), 0)
}

fn shell_icon(location: *const u16, flags: u32) -> Option<image::DynamicImage> {
    let mut info = SHFILEINFOW::default();
    // SHGetFileInfo transfers ownership of hIcon with SHGFI_ICON.
    let icon = unsafe {
        SHGetFileInfoW(
            location,
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON | flags,
        )
    };
    if icon == 0 || info.hIcon.is_null() {
        return None;
    }
    let rendered = icon_pixels(info.hIcon);
    unsafe { DestroyIcon(info.hIcon) };
    rendered
}

fn icon_pixels(icon: HICON) -> Option<image::DynamicImage> {
    const SIZE: i32 = 32;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = ptr::null_mut();
    // The DIB is a top-down array of 32-bit BGRA pixels owned by GDI.
    unsafe {
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, bitmap);
        let byte_count = (SIZE * SIZE * 4) as usize;
        ptr::write_bytes(bits.cast::<u8>(), 0, byte_count);
        let result = DrawIconEx(dc, 0, 0, icon, SIZE, SIZE, 0, ptr::null_mut(), DI_NORMAL);
        let mut pixels = slice::from_raw_parts(bits.cast::<u8>(), byte_count).to_vec();
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        if result == 0 {
            return None;
        }
        let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            if !has_alpha && pixel[..3] != [0, 0, 0] {
                pixel[3] = 255;
            }
        }
        image::RgbaImage::from_raw(SIZE as u32, SIZE as u32, pixels)
            .map(image::DynamicImage::ImageRgba8)
    }
}

fn launch(target: OpenTarget, app: &Path, folder: &Path) -> Command {
    let mut command = Command::new(app);
    match target {
        WindowsTerminal => {
            // wt resolves "." against the invoking process's working directory.
            // Passing the folder as an argv value would expose it to wt's command parsing.
            command.args(["new-tab", "-d", "."]).current_dir(folder);
        }
        WezTerm => {
            command.args(["start", "--cwd"]).arg(folder);
        }
        Alacritty => {
            command.arg("--working-directory").arg(folder);
        }
        _ => {
            command.arg(folder);
        }
    }
    command
}

pub(super) fn open(target: OpenTarget, app: &Path, folder: &Path) -> Result<(), String> {
    if target == Finder {
        let verb: Vec<_> = "explore".encode_utf16().chain([0]).collect();
        let path: Vec<_> = folder.as_os_str().encode_wide().chain([0]).collect();
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        } as isize;
        return if result > 32 {
            Ok(())
        } else {
            Err(format!("File Explorer returned code {result}"))
        };
    }
    launch(target, app, folder)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(super) fn file_command(app: &Path, file: &Path) -> Result<Command, String> {
    let mut command = Command::new(app);
    command.arg(file);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_exact_file_as_one_editor_argument() {
        let app = Path::new(r"C:\Program Files\Microsoft VS Code\Code.exe");
        let file = Path::new(r"C:\Worktree\同名 & ; %TEMP%.cmd");
        let command = file_command(app, file).unwrap();
        assert_eq!(command.get_program(), app.as_os_str());
        assert_eq!(command.get_args().collect::<Vec<_>>(), [file.as_os_str()]);
        assert!(command.get_current_dir().is_none());
    }

    #[test]
    fn windows_terminal_starts_in_the_folder_without_parsing_its_name() {
        let app = Path::new(r"C:\Program Files\Terminal\wt.exe");
        let folder = Path::new(r"C:\my project & ; %TEMP% 空格");
        let command = launch(WindowsTerminal, app, folder);
        assert_eq!(command.get_program(), app.as_os_str());
        assert_eq!(command.get_current_dir(), Some(folder));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["new-tab", "-d", "."]
        );
    }
}
