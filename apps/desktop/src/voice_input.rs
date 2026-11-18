use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::{app::HiveApp, language::tr, process_supervisor::OwnedProcess};

const LISTENING: u8 = 0;
const FINISH: u8 = 1;
const CANCEL: u8 = 2;
const RESPONSE_LIMIT: u64 = 64 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Reply {
    Completed { text: String },
    Error { code: String },
}

struct AudioFixture {
    file: PathBuf,
    #[cfg(target_os = "macos")]
    locale: Option<String>,
}

#[cfg(feature = "ui-preview")]
fn preview_audio() -> Option<AudioFixture> {
    std::env::var_os("HIVE_DESKTOP_PREVIEW_VOICE_FILE").map(|file| AudioFixture {
        file: PathBuf::from(file),
        #[cfg(target_os = "macos")]
        locale: std::env::var("HIVE_DESKTOP_PREVIEW_VOICE_LOCALE").ok(),
    })
}

#[cfg(not(feature = "ui-preview"))]
fn preview_audio() -> Option<AudioFixture> {
    None
}

struct Capture {
    control: Arc<AtomicU8>,
    done: smol::channel::Receiver<()>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.control.store(CANCEL, Ordering::Release);
    }
}

#[derive(Default)]
pub(crate) struct VoiceInput {
    active: Option<Capture>,
    pending: Option<String>,
}

impl VoiceInput {
    pub(crate) fn running(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn finishing(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|capture| capture.control.load(Ordering::Acquire) != LISTENING)
    }

    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn cancel(&mut self) -> Option<smol::channel::Receiver<()>> {
        self.pending = None;
        self.active.as_ref().map(|capture| {
            capture.control.store(CANCEL, Ordering::Release);
            capture.done.clone()
        })
    }
}

fn speech_error(code: &str) -> &'static str {
    match code {
        "speech_denied" => tr("请在系统设置的隐私与安全性中允许 Hive Desktop 使用语音识别。"),
        "mic_denied" => tr("请在系统的麦克风隐私设置中允许桌面应用使用麦克风。"),
        "mic_unavailable" => tr("无法打开默认麦克风，请检查设备和系统权限。"),
        "bundle_required" => tr("请从已打包的 Hive Desktop 打开语音输入。"),
        "unavailable" => tr("当前系统听写语言的本机识别不可用，请检查已安装的语音语言。"),
        "no_speech" => tr("没有识别到语音，草稿已保留。"),
        "timeout" => tr("等待听写完成超时，草稿已保留。"),
        _ => tr("系统听写未完成，草稿已保留。"),
    }
}

impl HiveApp {
    pub(crate) fn can_dictate(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "windows"))
            && self.connected
            && !self.loading
            && !self.closing
            && self.draft_target.is_some()
            && self.draft_target == self.draft_owner
            && (self.session.is_none() || self.conversation_ready)
            && !self.conversation_is_read_only()
            && self.workspace_writes_allowed()
    }

    pub(crate) fn toggle_voice_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active) = &self.voice.active {
            let _ = active.control.compare_exchange(
                LISTENING,
                FINISH,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            cx.notify();
            return;
        }
        if !self.can_dictate() || self.voice.pending() {
            return;
        }
        let target = self.draft_target.clone();
        let connection = self.connection_generation;
        let control = Arc::new(AtomicU8::new(LISTENING));
        let (done, finished) = smol::channel::bounded(1);
        self.voice.active = Some(Capture {
            control: control.clone(),
            done: finished,
        });
        self.error = None;
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let worker_control = control.clone();
            let reply =
                smol::unblock(move || run_capture(&worker_control, preview_audio().as_ref())).await;
            let _ = done.try_send(());
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    let cancelled = control.load(Ordering::Acquire) == CANCEL;
                    this.voice.active = None;
                    if !cancelled
                        && this.connection_generation == connection
                        && this.draft_target == target
                        && this.can_dictate()
                    {
                        match reply {
                            Ok(Some(Reply::Completed { text })) => {
                                this.voice.pending = Some(text);
                                this.insert_voice_text(window, cx);
                            }
                            Ok(Some(Reply::Error { code })) => {
                                this.error = Some(speech_error(&code).to_owned().into())
                            }
                            Err(error) => {
                                this.error =
                                    Some(crate::app::ErrorNotice::Detail("无法完成系统听写", error))
                            }
                            Ok(None) => {}
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn insert_voice_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_dictate() {
            return;
        }
        let Some(text) = self.voice.pending.as_deref() else {
            return;
        };
        let inserted = self.input.update(cx, |input, cx| {
            if input.marked_text_range(window, cx).is_some() {
                return false;
            }
            let current = input.value();
            let fragment = suffix(&current, text);
            let end = current.encode_utf16().count();
            let previous_len = current.len();
            input.replace_text_in_range(Some(end..end), &fragment, window, cx);
            input.focus_handle(cx).focus(window, cx);
            input.value().len() > previous_len
        });
        if inserted {
            self.voice.pending = None;
            self.schedule_draft_save(cx);
            cx.notify();
        }
    }
}

fn suffix(current: &str, speech: &str) -> String {
    if current.is_empty() || current.ends_with(char::is_whitespace) {
        speech.to_owned()
    } else {
        format!(" {speech}")
    }
}

fn run_capture(
    control: &AtomicU8,
    fixture: Option<&AudioFixture>,
) -> Result<Option<Reply>, String> {
    if control.load(Ordering::Acquire) == CANCEL {
        return Ok(None);
    }
    let mut command = platform_command(fixture).map_err(|error| error.to_string())?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut process = OwnedProcess::spawn(&mut command).map_err(|error| error.to_string())?;
    let mut reader = None;
    let completed = (|| -> std::io::Result<bool> {
        let mut input = process
            .take_stdin()
            .ok_or_else(|| std::io::Error::other("Speech control pipe unavailable"))?;
        let output = process
            .take_stdout()
            .ok_or_else(|| std::io::Error::other("Speech result pipe unavailable"))?;
        reader = Some(
            std::thread::Builder::new()
                .stack_size(128 * 1024)
                .spawn(move || {
                    let mut bytes = Vec::new();
                    output.take(RESPONSE_LIMIT + 1).read_to_end(&mut bytes)?;
                    Ok::<_, std::io::Error>(bytes)
                })?,
        );
        if control.load(Ordering::Acquire) == CANCEL {
            return Ok(false);
        }
        input.write_all(b"start\n")?;
        let mut deadline = Instant::now() + Duration::from_secs(140);
        let mut stopping = false;
        loop {
            let state = control.load(Ordering::Acquire);
            if state == CANCEL {
                return Ok(false);
            }
            if process.leader_exited()? {
                return Ok(true);
            }
            if state == FINISH && !stopping {
                if let Err(error) = input.write_all(b"finish\n")
                    && error.kind() != std::io::ErrorKind::BrokenPipe
                {
                    return Err(error);
                }
                stopping = true;
                deadline = Instant::now() + Duration::from_secs(5);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    })();
    process.cleanup(Duration::from_millis(1200))?;
    let bytes = reader
        .map(|reader| {
            reader
                .join()
                .map_err(|_| "Speech response reader stopped".to_owned())
        })
        .transpose()?
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    if control.load(Ordering::Acquire) == CANCEL {
        return Ok(None);
    }
    if !completed.map_err(|error| error.to_string())? {
        return Ok(Some(Reply::Error {
            code: "timeout".to_owned(),
        }));
    }
    if bytes.len() as u64 > RESPONSE_LIMIT {
        return Err("Speech result exceeds its size limit".to_owned());
    }
    let reply = serde_json::from_slice::<Reply>(&bytes)
        .map_err(|_| "No valid response from system speech recognition".to_owned())?;
    Ok(Some(match reply {
        Reply::Completed { text } if text.trim().is_empty() => Reply::Error {
            code: "no_speech".to_owned(),
        },
        Reply::Completed { text } => Reply::Completed {
            text: text.trim().to_owned(),
        },
        error => error,
    }))
}

#[cfg(target_os = "macos")]
fn platform_command(fixture: Option<&AudioFixture>) -> std::io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--voice-capture");
    if let Some(fixture) = fixture {
        command.arg(&fixture.file);
        if let Some(locale) = &fixture.locale {
            command.arg(locale);
        }
    }
    Ok(command)
}

#[cfg(target_os = "windows")]
fn platform_command(fixture: Option<&AudioFixture>) -> std::io::Result<Command> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
    let root = std::env::var_os("SystemRoot")
        .ok_or_else(|| std::io::Error::other("Windows SystemRoot is unavailable"))?;
    let script: Vec<_> = include_str!("voice_capture.ps1")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut command =
        Command::new(PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
        ])
        .arg(STANDARD.encode(script))
        .creation_flags(CREATE_NO_WINDOW)
        .env_remove("HIVE_VOICE_AUDIO_FIXTURE");
    if let Some(fixture) = fixture {
        command.env("HIVE_VOICE_AUDIO_FIXTURE", &fixture.file);
    }
    Ok(command)
}

pub(crate) fn run_helper_if_requested() {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next();
    if mode.as_deref() == Some(std::ffi::OsStr::new("--voice-check-file")) {
        let Some(file) = args.next() else {
            std::process::exit(2)
        };
        let fixture = AudioFixture {
            file: PathBuf::from(file),
            #[cfg(target_os = "macos")]
            locale: args.next().and_then(|arg| arg.into_string().ok()),
        };
        let reply = run_capture(&AtomicU8::new(LISTENING), Some(&fixture));
        match reply {
            Ok(Some(reply)) => {
                println!(
                    "{}",
                    serde_json::to_string(&reply).expect("encode speech check")
                );
                std::process::exit(i32::from(!matches!(reply, Reply::Completed { .. })));
            }
            Ok(None) => std::process::exit(2),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(target_os = "macos")]
    if matches!(
        mode.as_deref().and_then(|value| value.to_str()),
        Some("--voice-capture" | "--voice-input-status")
    ) {
        use std::{ffi::CString, os::unix::ffi::OsStrExt, ptr};
        unsafe extern "C" {
            fn hive_voice_capture(
                file: *const std::ffi::c_char,
                locale: *const std::ffi::c_char,
                status: std::ffi::c_int,
            ) -> std::ffi::c_int;
        }
        let status =
            i32::from(mode.as_deref().and_then(|arg| arg.to_str()) == Some("--voice-input-status"));
        let file = args
            .next()
            .and_then(|value| CString::new(value.as_bytes()).ok());
        let locale = args
            .next()
            .and_then(|value| CString::new(value.as_bytes()).ok());
        let code = unsafe {
            hive_voice_capture(
                file.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                locale.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                status,
            )
        };
        std::process::exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::suffix;

    #[test]
    fn appends_speech_to_the_existing_draft_without_overwriting_it() {
        assert_eq!(suffix("", "下一句"), "下一句");
        assert_eq!(suffix("已有🙂", "下一句"), " 下一句");
        assert_eq!(suffix("已有🙂\n", "下一句"), "下一句");
    }
}
