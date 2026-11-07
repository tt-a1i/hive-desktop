use alacritty_terminal::term::TermMode;
use gpui_kit::Keystroke;

/// Platform text payloads are not proof of printable input: macOS attaches
/// key_char="\n" to Return and key_char="\t" to Tab.
pub(crate) fn key_input_bytes(
    keystroke: &Keystroke,
    mode: TermMode,
    composing: bool,
) -> Option<Vec<u8>> {
    let modifiers = &keystroke.modifiers;
    // The OS IME owns candidate commit/navigation while marked text exists.
    // Explicit control shortcuts such as Ctrl-C still belong to the terminal.
    if composing && !modifiers.control && !modifiers.platform {
        return None;
    }
    if matches!(
        keystroke.key.as_str(),
        "enter"
            | "tab"
            | "backspace"
            | "escape"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "insert"
            | "delete"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    ) {
        return keystroke_to_bytes(keystroke, mode);
    }
    if keystroke
        .key_char
        .as_ref()
        .is_some_and(|text| !text.is_empty() && text.chars().all(|ch| !ch.is_control()))
        && !modifiers.control
        && !modifiers.alt
        && !modifiers.platform
        && !modifiers.function
    {
        return None;
    }
    keystroke_to_bytes(keystroke, mode)
}

pub fn keystroke_to_bytes(keystroke: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    if keystroke.modifiers.platform {
        return None;
    }
    match keystroke.key.as_str() {
        "space" => {
            if keystroke.modifiers.control {
                return Some(b"\x00".to_vec());
            }
            return Some(b" ".to_vec());
        }
        "enter" => return Some(b"\r".to_vec()),
        "escape" => return Some(b"\x1b".to_vec()),
        "backspace" => return Some(b"\x7f".to_vec()),
        "tab" => {
            if keystroke.modifiers.shift {
                return Some(b"\x1b[Z".to_vec());
            }
            return Some(b"\t".to_vec());
        }

        "up" => {
            if mode.contains(TermMode::APP_CURSOR) {
                return Some(b"\x1bOA".to_vec());
            }
            return Some(b"\x1b[A".to_vec());
        }
        "down" => {
            if mode.contains(TermMode::APP_CURSOR) {
                return Some(b"\x1bOB".to_vec());
            }
            return Some(b"\x1b[B".to_vec());
        }
        "right" => {
            if mode.contains(TermMode::APP_CURSOR) {
                return Some(b"\x1bOC".to_vec());
            }
            return Some(b"\x1b[C".to_vec());
        }
        "left" => {
            if mode.contains(TermMode::APP_CURSOR) {
                return Some(b"\x1bOD".to_vec());
            }
            return Some(b"\x1b[D".to_vec());
        }

        "home" => return Some(b"\x1b[H".to_vec()),
        "end" => return Some(b"\x1b[F".to_vec()),
        "pageup" => return Some(b"\x1b[5~".to_vec()),
        "pagedown" => return Some(b"\x1b[6~".to_vec()),
        "insert" => return Some(b"\x1b[2~".to_vec()),
        "delete" => return Some(b"\x1b[3~".to_vec()),

        "f1" => return Some(b"\x1bOP".to_vec()),
        "f2" => return Some(b"\x1bOQ".to_vec()),
        "f3" => return Some(b"\x1bOR".to_vec()),
        "f4" => return Some(b"\x1bOS".to_vec()),
        "f5" => return Some(b"\x1b[15~".to_vec()),
        "f6" => return Some(b"\x1b[17~".to_vec()),
        "f7" => return Some(b"\x1b[18~".to_vec()),
        "f8" => return Some(b"\x1b[19~".to_vec()),
        "f9" => return Some(b"\x1b[20~".to_vec()),
        "f10" => return Some(b"\x1b[21~".to_vec()),
        "f11" => return Some(b"\x1b[23~".to_vec()),
        "f12" => return Some(b"\x1b[24~".to_vec()),

        _ => {}
    }

    if keystroke.modifiers.control {
        let key = keystroke.key.as_str();

        if key.len() == 1 {
            let ch = key.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                let upper = ch.to_ascii_uppercase();
                let ctrl_char = (upper as u8) - b'@';
                return Some(vec![ctrl_char]);
            }

            match ch {
                '[' => return Some(b"\x1b".to_vec()),
                '\\' => return Some(b"\x1c".to_vec()),
                ']' => return Some(b"\x1d".to_vec()),
                '^' => return Some(b"\x1e".to_vec()),
                '_' => return Some(b"\x1f".to_vec()),
                '?' => return Some(b"\x7f".to_vec()),
                _ => {}
            }
        }
    }

    if keystroke.modifiers.alt {
        let key = keystroke.key.as_str();
        if key.len() == 1 {
            let ch = key.chars().next().unwrap();
            if ch.is_ascii() {
                let mut bytes = vec![b'\x1b'];
                bytes.push(ch as u8);
                return Some(bytes);
            }
        }
    }

    if let Some(key_char) = &keystroke.key_char
        && !keystroke.modifiers.control
        && !keystroke.modifiers.alt
    {
        return Some(key_char.as_bytes().to_vec());
    }

    let key = keystroke.key.as_str();
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();
        if ch.is_ascii() && !keystroke.modifiers.control {
            let ch = if keystroke.modifiers.shift {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            return Some(vec![ch as u8]);
        }
        if !keystroke.modifiers.control && !keystroke.modifiers.alt {
            return Some(key.as_bytes().to_vec());
        }
    }

    None
}

/// Honor the mode negotiated by the application running in the terminal.
pub fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        let mut bytes = b"\x1b[200~".to_vec();
        // Pasted ESC cannot terminate the wrapper and become shell commands.
        bytes.extend(text.replace('\x1b', "").as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        bytes
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r").into_bytes()
    }
}
