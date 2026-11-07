use alacritty_terminal::event::{Event, EventListener};
use std::sync::mpsc::Sender;

/// Events belong to the host's current terminal attachment, never an owned PTY.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Input { bytes: Vec<u8>, user_input: bool },
    Resize { cols: usize, rows: usize },
    Title(String),
    Bell,
}

pub(crate) enum ParserEvent {
    Reply(String),
    Title(String),
    Bell,
}

#[derive(Clone)]
pub struct GpuiEventProxy {
    tx: Sender<ParserEvent>,
}
impl GpuiEventProxy {
    pub(crate) fn new(tx: Sender<ParserEvent>) -> Self {
        Self { tx }
    }
}
impl EventListener for GpuiEventProxy {
    fn send_event(&self, event: Event) {
        let event = match event {
            Event::PtyWrite(data) => ParserEvent::Reply(data),
            Event::Title(title) => ParserEvent::Title(title),
            Event::ResetTitle => ParserEvent::Title(String::new()),
            Event::Bell => ParserEvent::Bell,
            // Remote output must not read or overwrite the user's clipboard (OSC 52).
            // Process lifetime is the host bridge's responsibility, not an ANSI event.
            _ => return,
        };
        let _ = self.tx.send(event);
    }
}
