use alacritty_terminal::index::{Column, Line, Point as AlacPoint};
use alacritty_terminal::term::TermMode;
use gpui_kit::{MouseButton, Pixels, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    Simple,
    Word,
    Line,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub start: AlacPoint,
    pub end: AlacPoint,
    pub selection_type: SelectionType,
}

impl Selection {
    pub fn new(start: AlacPoint, end: AlacPoint, selection_type: SelectionType) -> Self {
        Self {
            start,
            end,
            selection_type,
        }
    }

    pub fn contains(&self, point: AlacPoint) -> bool {
        let (start, end) = if self.start < self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };

        point >= start && point <= end
    }
}

pub fn pixel_to_cell(
    position: Point<Pixels>,
    origin: Point<Pixels>,
    cell_width: Pixels,
    cell_height: Pixels,
) -> AlacPoint {
    let col = ((position.x - origin.x) / cell_width).floor();
    let col = col.max(0.0) as usize;

    let row = ((position.y - origin.y) / cell_height).floor();
    let row = row.max(0.0) as i32;

    AlacPoint::new(Line(row), Column(col))
}

pub fn selection_type_from_clicks(click_count: usize) -> SelectionType {
    match click_count {
        1 => SelectionType::Simple,
        2 => SelectionType::Word,
        _ => SelectionType::Line,
    }
}

pub fn mouse_button_report(
    button: MouseButton,
    pressed: bool,
    point: AlacPoint,
    modifiers: u8,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode
        .intersects(TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG)
    {
        return None;
    }

    let button_code = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => return None,
    };

    let button_value = button_code | modifiers;

    let col = point.column.0 + 1;
    let row = point.line.0 + 1;

    let action = if pressed { b'M' } else { b'm' };

    let sequence = format!("\x1b[<{};{};{}{}", button_value, col, row, action as char);
    Some(sequence.into_bytes())
}

pub fn scroll_report(
    delta: i32,
    point: AlacPoint,
    modifiers: u8,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if mode.intersects(TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG)
    {
        let button_code = if delta > 0 { 64 } else { 65 };
        let button_value = button_code | modifiers;

        let col = point.column.0 + 1;
        let row = point.line.0 + 1;

        let sequence = format!("\x1b[<{};{};{}M", button_value, col, row);
        return Some(sequence.into_bytes());
    }

    if mode.contains(TermMode::ALT_SCREEN) {
        return Some(scroll_to_arrow_keys(delta, mode));
    }

    None
}

fn scroll_to_arrow_keys(delta: i32, mode: TermMode) -> Vec<u8> {
    let count = delta.abs().min(5) as usize;

    let arrow_seq = if delta > 0 {
        if mode.contains(TermMode::APP_CURSOR) {
            b"\x1bOA"
        } else {
            b"\x1b[A"
        }
    } else {
        if mode.contains(TermMode::APP_CURSOR) {
            b"\x1bOB"
        } else {
            b"\x1b[B"
        }
    };

    let mut result = Vec::with_capacity(arrow_seq.len() * count);
    for _ in 0..count {
        result.extend_from_slice(arrow_seq);
    }
    result
}

pub fn encode_modifiers(shift: bool, alt: bool, control: bool) -> u8 {
    let mut modifiers = 0;
    if shift {
        modifiers |= 4;
    }
    if alt {
        modifiers |= 8;
    }
    if control {
        modifiers |= 16;
    }
    modifiers
}

pub fn pixels_to_scroll_lines(pixel_delta: Pixels, cell_height: Pixels) -> i32 {
    let lines = (pixel_delta / cell_height).round();
    lines.clamp(-10.0, 10.0) as i32
}
