use crate::event::GpuiEventProxy;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::selection::Selection;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use parking_lot::Mutex;
use std::sync::Arc;

struct TermDimensions {
    columns: usize,
    screen_lines: usize,
}

impl TermDimensions {
    fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns,
            screen_lines,
        }
    }
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }

    fn last_column(&self) -> alacritty_terminal::index::Column {
        alacritty_terminal::index::Column(self.columns.saturating_sub(1))
    }
}

pub struct TerminalState {
    term: Arc<Mutex<Term<GpuiEventProxy>>>,

    parser: Processor,

    cols: usize,

    rows: usize,
    history: usize,
    proxy: GpuiEventProxy,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize, history: usize, event_proxy: GpuiEventProxy) -> Self {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let config = Config {
            scrolling_history: history,
            ..Config::default()
        };

        let dimensions = TermDimensions::new(cols, rows);

        let term = Term::new(config, &dimensions, event_proxy.clone());

        let parser = Processor::new();

        Self {
            term: Arc::new(Mutex::new(term)),
            parser,
            cols,
            rows,
            history,
            proxy: event_proxy,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.cols, self.rows, self.history, self.proxy.clone());
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        let mut term = self.term.lock();
        self.parser.advance(&mut *term, bytes);
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        self.cols = cols;
        self.rows = rows;

        let mut term = self.term.lock();

        let dimensions = TermDimensions::new(cols, rows);

        term.resize(dimensions);
    }

    pub fn mode(&self) -> TermMode {
        let term = self.term.lock();
        *term.mode()
    }

    pub fn with_term<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Term<GpuiEventProxy>) -> R,
    {
        let term = self.term.lock();
        f(&term)
    }

    pub fn with_term_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Term<GpuiEventProxy>) -> R,
    {
        let mut term = self.term.lock();
        f(&mut term)
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn scroll_display(&self, scroll: Scroll) {
        let mut term = self.term.lock();
        term.scroll_display(scroll);
    }

    pub fn scroll_to_bottom(&self) {
        self.scroll_display(Scroll::Bottom);
    }

    pub fn clear_selection(&self) {
        let mut term = self.term.lock();
        term.selection = None;
    }

    pub fn update_selection(&self, selection: Option<Selection>) {
        let mut term = self.term.lock();
        term.selection = selection;
    }

    pub fn selection_text(&self) -> Option<String> {
        let term = self.term.lock();
        term.selection_to_string()
    }

    pub fn clear_screen(&mut self) {
        self.process_bytes(b"[3J[H[2J");
        let mut term = self.term.lock();
        term.selection = None;
    }

    pub fn display_offset(&self) -> usize {
        let term = self.term.lock();
        term.grid().display_offset()
    }

    pub fn term_arc(&self) -> Arc<Mutex<Term<GpuiEventProxy>>> {
        Arc::clone(&self.term)
    }
}
