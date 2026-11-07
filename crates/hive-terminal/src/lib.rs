pub mod colors;
pub mod event;
mod glyphs;
mod ime;
pub mod input;
pub mod mouse;
pub mod render;
pub mod terminal;
pub mod view;

pub use colors::{ColorPalette, ColorPaletteBuilder};
pub use event::{GpuiEventProxy, TerminalEvent};
pub use render::TerminalRenderer;
pub use terminal::TerminalState;
pub use view::{TerminalConfig, TerminalView};

#[cfg(test)]
mod checks;
