use crate::{
    event::{GpuiEventProxy, ParserEvent},
    input::{KeyInput, key_input, keystroke_to_bytes, paste_bytes},
    terminal::TerminalState,
};
use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::TermMode,
    vte::ansi::{Color, NamedColor},
};
use gpui_kit::{Keystroke, Modifiers};
use std::sync::mpsc;

fn state() -> (TerminalState, mpsc::Receiver<ParserEvent>) {
    let (tx, rx) = mpsc::channel();
    (TerminalState::new(12, 4, 100, GpuiEventProxy::new(tx)), rx)
}
fn row(state: &TerminalState, count: usize) -> String {
    state.with_term(|term| {
        (0..count)
            .map(|col| term.grid()[Line(0)][Column(col)].c)
            .collect()
    })
}

#[test]
fn real_parser_overwrites_cells_tracks_color_utf8_and_alternate_screen() {
    let (mut state, _) = state();
    state.process_bytes(b"abc\x1b[2DX");
    assert_eq!(row(&state, 3), "aXc");
    state.process_bytes(b"\x1b[H\x1b[3");
    state.process_bytes(b"1mR");
    state.with_term(|term| {
        assert_eq!(
            term.grid()[Line(0)][Column(0)].fg,
            Color::Named(NamedColor::Red)
        )
    });
    state.process_bytes(&"中".as_bytes()[..2]);
    state.process_bytes(&"中".as_bytes()[2..]);
    assert_eq!(row(&state, 2), "R中");
    state.process_bytes(b"\x1b[?1049hALT");
    assert!(state.mode().contains(TermMode::ALT_SCREEN));
    state.process_bytes(b"\x1b[?1049l");
    assert!(!state.mode().contains(TermMode::ALT_SCREEN));
    assert_eq!(row(&state, 2), "R中");
    state.resize(20, 8);
    assert_eq!((state.cols(), state.rows()), (20, 8));
}

#[test]
fn reset_replaces_partial_parser_modes_and_selection_and_dsr_uses_real_cursor() {
    let (mut state, rx) = state();
    state.process_bytes(b"abc");
    let mut selection = Selection::new(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Left,
    );
    selection.update(Point::new(Line(0), Column(2)), Side::Right);
    state.update_selection(Some(selection));
    assert_eq!(state.selection_text().as_deref(), Some("abc"));
    state.process_bytes(b"\x1b[?2004h\x1b[31");
    assert!(state.mode().contains(TermMode::BRACKETED_PASTE));
    state.reset();
    assert!(!state.mode().contains(TermMode::BRACKETED_PASTE));
    assert_eq!(state.selection_text(), None);
    state.process_bytes(b"ok\x1b[6n");
    assert_eq!(row(&state, 3), "ok ");
    let replies: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            ParserEvent::Reply(data) => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(replies, vec!["\x1b[1;3R"]);
}

#[test]
fn native_modes_control_paste_and_cursor_keys_without_intercepting_interrupt() {
    let (mut state, _) = state();
    assert_eq!(paste_bytes("a\r\nb", state.mode()), b"a\rb");
    state.process_bytes(b"\x1b[?2004h\x1b[?1h");
    assert_eq!(
        paste_bytes("中\nb\x1b[201~", state.mode()),
        "\x1b[200~中\nb[201~\x1b[201~".as_bytes()
    );
    assert_eq!(
        keystroke_to_bytes(&Keystroke::parse("up").unwrap(), state.mode()),
        Some(b"\x1bOA".to_vec())
    );
    assert_eq!(
        keystroke_to_bytes(&Keystroke::parse("ctrl-c").unwrap(), state.mode()),
        Some(vec![3])
    );
    assert_eq!(
        keystroke_to_bytes(&Keystroke::parse("cmd-c").unwrap(), state.mode()),
        None
    );
}

#[test]
fn scrollback_shortcuts_keep_terminal_and_ime_navigation_on_their_existing_paths() {
    let empty = TermMode::empty();
    let alt_screen = TermMode::ALT_SCREEN;
    let mac = Modifiers::command();
    let windows = Modifiers::control();
    let input = |key, mode, composing, secondary| {
        key_input(&Keystroke::parse(key).unwrap(), mode, composing, secondary)
    };

    for secondary in [mac, windows] {
        assert!(matches!(
            input("shift-pageup", empty, false, secondary),
            Some(KeyInput::Scroll(Scroll::PageUp))
        ));
        assert!(matches!(
            input("shift-pagedown", empty, false, secondary),
            Some(KeyInput::Scroll(Scroll::PageDown))
        ));
        assert!(input("shift-pageup", empty, true, secondary).is_none());
        for (key, mode, expected) in [
            ("pageup", empty, b"\x1b[5~".as_slice()),
            ("alt-shift-pageup", empty, b"\x1b[5~"),
            ("ctrl-shift-pageup", empty, b"\x1b[5~"),
            ("shift-pageup", alt_screen, b"\x1b[5~"),
            ("shift-pagedown", alt_screen, b"\x1b[6~"),
            ("home", empty, b"\x1b[H"),
            ("end", empty, b"\x1b[F"),
        ] {
            assert!(matches!(
                input(key, mode, false, secondary),
                Some(KeyInput::Write(bytes)) if bytes == expected
            ));
        }
    }

    assert!(matches!(
        input("cmd-shift-home", empty, false, mac),
        Some(KeyInput::Scroll(Scroll::Top))
    ));
    assert!(matches!(
        input("cmd-shift-end", empty, false, mac),
        Some(KeyInput::Scroll(Scroll::Bottom))
    ));
    assert!(matches!(
        input("ctrl-shift-home", empty, false, windows),
        Some(KeyInput::Scroll(Scroll::Top))
    ));
    assert!(matches!(
        input("ctrl-shift-end", empty, false, windows),
        Some(KeyInput::Scroll(Scroll::Bottom))
    ));
    assert!(input("cmd-shift-home", alt_screen, false, mac).is_none());
    assert!(input("cmd-shift-home", empty, false, windows).is_none());
    assert!(input("cmd-shift-home", empty, true, mac).is_none());
    assert!(input("cmd-ctrl-shift-home", empty, false, mac).is_none());
    for (mode, composing, secondary) in [
        (alt_screen, false, windows),
        (empty, true, windows),
        (empty, false, mac),
    ] {
        assert!(matches!(
            input("ctrl-shift-home", mode, composing, secondary),
            Some(KeyInput::Write(bytes)) if bytes == b"\x1b[H"
        ));
    }
}

#[test]
fn local_scrollback_pages_clamp_to_retained_output_and_jump_back_to_latest() {
    let (mut state, rx) = state();
    let output = (1..=20)
        .map(|line| format!("line {line}\r\n"))
        .collect::<String>();
    state.process_bytes(output.as_bytes());

    for (shortcut, expected_offset) in [
        ("shift-pagedown", 0),
        ("shift-pageup", 4),
        ("shift-pageup", 8),
        ("shift-pagedown", 4),
        ("cmd-shift-home", 17),
        ("shift-pageup", 17),
        ("shift-pagedown", 13),
        ("cmd-shift-end", 0),
        ("shift-pageup", 4),
    ] {
        let Some(KeyInput::Scroll(scroll)) = key_input(
            &Keystroke::parse(shortcut).unwrap(),
            state.mode(),
            false,
            Modifiers::command(),
        ) else {
            panic!("{shortcut} should scroll locally");
        };
        state.scroll_display(scroll);
        assert_eq!(state.display_offset(), expected_offset, "{shortcut}");
    }
    state.scroll_to_bottom();
    assert_eq!(state.display_offset(), 0);
    assert!(rx.try_recv().is_err());
}

#[test]
fn ime_tracks_utf16_marked_text_separately_from_candidate_selection() {
    let mut ime = crate::ime::Composition::default();
    ime.mark(None, "你😀好", Some(1..3));
    assert_eq!(ime.marked, Some(0..4));
    assert_eq!(ime.selected, 1..3);
    assert_eq!(ime.text_for(1..3), ("😀".into(), 1..3));
    ime.mark(Some(1..3), "中", Some(1..1));
    assert_eq!(ime.text, "你中好");
    assert_eq!(ime.marked, Some(1..2));
    assert_eq!(ime.selected, 2..2);
    ime.clear();
    assert_eq!(
        (ime.text.as_str(), ime.marked, ime.selected),
        ("", None, 0..0)
    );
}

#[test]
fn renderer_uses_native_inverse_colors_and_both_columns_of_wide_cells() {
    let (mut state, _) = state();
    state.process_bytes("\x1b[41;7m中".as_bytes());
    let renderer = crate::render::TerminalRenderer::new(
        "monospace".into(),
        gpui_kit::px(14.0),
        1.0,
        crate::ColorPalette::default(),
    );
    state.with_term(|term| {
        let cell = term.grid()[Line(0)][Column(0)].clone();
        let (backgrounds, runs) = renderer.layout_row(0, std::iter::once((0, cell)), term.colors());
        assert_eq!((backgrounds[0].start_col, backgrounds[0].end_col), (0, 2));
        assert_eq!(backgrounds[0].color, renderer.palette.foreground());
        assert_ne!(runs[0].fg_color, renderer.palette.foreground());
    });
}

#[test]
fn mac_return_payload_and_other_controls_bypass_printable_defer_but_not_composition() {
    use crate::input::key_input_bytes;
    let cases: &[(&str, Option<&str>, &[u8])] = &[
        ("enter", Some("\n"), b"\r"),
        ("tab", Some("\t"), b"\t"),
        ("up", None, b"\x1b[A"),
        ("backspace", None, b"\x7f"),
        ("escape", None, b"\x1b"),
    ];
    for (key, text, expected) in cases {
        let mut key = Keystroke::parse(key).unwrap();
        key.key_char = text.map(str::to_string);
        assert_eq!(
            key_input_bytes(&key, TermMode::empty(), false).as_deref(),
            Some(*expected)
        );
        assert_eq!(key_input_bytes(&key, TermMode::empty(), true), None);
    }
    let mut space = Keystroke::parse("space").unwrap();
    space.key_char = Some(" ".into());
    assert_eq!(key_input_bytes(&space, TermMode::empty(), false), None);
    assert_eq!(key_input_bytes(&space, TermMode::empty(), true), None);
    assert_eq!(
        key_input_bytes(
            &Keystroke::parse("ctrl-c").unwrap(),
            TermMode::empty(),
            true
        ),
        Some(vec![3])
    );
}
