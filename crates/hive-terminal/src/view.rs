use crate::colors::ColorPalette;
use crate::event::{GpuiEventProxy, ParserEvent, TerminalEvent};
use crate::input::key_input_bytes;
use crate::mouse::pixel_to_cell;
use crate::render::TerminalRenderer;
use crate::terminal::TerminalState;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::{Selection, SelectionType};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::{Edges, ElementInputHandler, EntityInputHandler, UTF16Selection, *};
use std::sync::Arc;
use std::sync::mpsc;

const CONTEXT: &str = "HiveTerminal";

actions!(hive_terminal, [SendTab, SendBacktab]);

#[derive(Clone, Debug)]
pub struct TerminalConfig {
    pub cols: usize,

    pub rows: usize,

    pub font_family: String,

    pub font_size: Pixels,

    pub scrollback: usize,

    pub line_height_multiplier: f32,

    pub padding: Edges<Pixels>,

    pub colors: ColorPalette,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            // GPUI takes real family names, not CSS generic family aliases.
            font_family: if cfg!(target_os = "macos") {
                "Menlo"
            } else if cfg!(target_os = "windows") {
                "Consolas"
            } else {
                "DejaVu Sans Mono"
            }
            .into(),
            font_size: px(14.0),
            scrollback: 10000,
            line_height_multiplier: 1.0,
            padding: Edges::all(px(0.0)),
            colors: ColorPalette::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LayoutMetrics {
    origin: Point<Pixels>,
    cell_width: Pixels,
    cell_height: Pixels,
    cols: usize,
}

pub struct TerminalView {
    state: TerminalState,

    renderer: TerminalRenderer,

    focus_handle: FocusHandle,

    input_enabled: bool,

    event_rx: mpsc::Receiver<ParserEvent>,

    config: TerminalConfig,

    layout_metrics: Arc<parking_lot::Mutex<LayoutMetrics>>,

    dragging_selection: bool,

    ime: crate::ime::Composition,
}

impl TerminalView {
    pub fn new(config: TerminalConfig, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("tab", SendTab, Some(CONTEXT)),
            KeyBinding::new("shift-tab", SendBacktab, Some(CONTEXT)),
        ]);

        let (event_tx, event_rx) = mpsc::channel();

        let event_proxy = GpuiEventProxy::new(event_tx);

        let state = TerminalState::new(config.cols, config.rows, config.scrollback, event_proxy);

        let renderer = TerminalRenderer::new(
            config.font_family.clone(),
            config.font_size,
            config.line_height_multiplier,
            config.colors.clone(),
        );

        let focus_handle = cx.focus_handle().tab_stop(false);

        Self {
            state,
            renderer,
            focus_handle,
            input_enabled: false,
            event_rx,
            config,
            layout_metrics: Arc::new(parking_lot::Mutex::new(LayoutMetrics::default())),
            dragging_selection: false,
            ime: crate::ime::Composition::default(),
        }
    }

    /// Parse synchronously before the host acknowledges these bytes upstream.
    pub fn feed(&mut self, bytes: &[u8], cx: &mut Context<Self>) -> usize {
        self.state.process_bytes(bytes);
        self.process_events(cx);
        cx.notify();
        bytes.len()
    }

    /// A restore starts from a fresh parser, including partial UTF-8/ANSI state.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.state.reset();
        while self.event_rx.try_recv().is_ok() {}
        self.ime.clear();
        self.dragging_selection = false;
        cx.notify();
    }

    pub fn set_input_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.input_enabled = enabled;
        if !enabled {
            self.ime.clear();
        }
        cx.notify();
    }

    fn on_action_send_tab(&mut self, _: &SendTab, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.ime.marked.is_some() {
            return;
        }
        if self.write_input_bytes(b"\t", _cx) {
            _cx.stop_propagation();
            _cx.notify();
        }
    }

    fn on_action_send_backtab(
        &mut self,
        _: &SendBacktab,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if self.ime.marked.is_some() {
            return;
        }
        if self.write_input_bytes(b"\x1b[Z", _cx) {
            _cx.stop_propagation();
            _cx.notify();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_lowercase();
        let modifiers = &event.keystroke.modifiers;
        let clipboard_shortcut = if cfg!(target_os = "macos") {
            modifiers.platform
        } else {
            modifiers.control && modifiers.shift
        };
        if clipboard_shortcut && key == "c" {
            self.copy_selection(_cx);
            _cx.stop_propagation();
            return;
        }
        if clipboard_shortcut && key == "v" {
            self.paste_from_clipboard(_cx);
            _cx.stop_propagation();
            return;
        }
        // Ctrl-C without the copy shortcut must reach the shell as SIGINT.
        if !self.input_enabled {
            return;
        }

        if let Some(bytes) = key_input_bytes(
            &event.keystroke,
            self.state.mode(),
            self.ime.marked.is_some(),
        ) && self.write_input_bytes(&bytes, _cx)
        {
            _cx.stop_propagation();
            _cx.notify();
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let selection_type = match event.click_count {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        if let Some(point) = self.hit_test_cell(event.position) {
            self.state
                .update_selection(Some(Selection::new(selection_type, point, Side::Left)));
            self.dragging_selection = true;
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(point) = self.hit_test_cell(event.position) {
            self.update_selection_endpoint(point);
        }
        self.dragging_selection = false;
        // Selection stays available for an explicit copy action.
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging_selection || !event.dragging() {
            return;
        }
        if let Some(point) = self.hit_test_cell(event.position) {
            self.update_selection_endpoint(point);
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let delta = match event.delta {
            ScrollDelta::Lines(delta) => delta.y.round() as i32,
            ScrollDelta::Pixels(delta) => {
                let line_height: f32 = self.renderer.cell_height.into();
                if line_height <= 0.0 {
                    0
                } else {
                    (f32::from(delta.y) / line_height).round() as i32
                }
            }
        };
        if delta != 0 {
            self.state.scroll_display(Scroll::Delta(delta));
            cx.notify();
        }
    }

    fn hit_test_cell(&self, position: Point<Pixels>) -> Option<alacritty_terminal::index::Point> {
        let metrics = *self.layout_metrics.lock();
        if metrics.cell_width <= px(0.0) || metrics.cell_height <= px(0.0) {
            return None;
        }
        if position.x < metrics.origin.x || position.y < metrics.origin.y {
            return None;
        }
        let mut point = pixel_to_cell(
            position,
            metrics.origin,
            metrics.cell_width,
            metrics.cell_height,
        );
        if point.column.0 >= metrics.cols {
            point.column = alacritty_terminal::index::Column(metrics.cols.saturating_sub(1));
        }
        let offset = self.state.display_offset() as i32;
        point.line = alacritty_terminal::index::Line(point.line.0 - offset);
        Some(point)
    }

    fn update_selection_endpoint(&mut self, point: alacritty_terminal::index::Point) {
        self.state.with_term_mut(|term| {
            if let Some(selection) = term.selection.as_mut() {
                selection.update(point, Side::Right);
            }
        });
    }

    fn process_events(&mut self, cx: &mut Context<Self>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ParserEvent::Reply(data) => cx.emit(TerminalEvent::Input {
                    bytes: data.into_bytes(),
                    user_input: false,
                }),
                ParserEvent::Title(title) => cx.emit(TerminalEvent::Title(title)),
                ParserEvent::Bell => cx.emit(TerminalEvent::Bell),
            }
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.state.cols(), self.state.rows())
    }

    pub fn resize(&mut self, cols: usize, rows: usize, cx: &mut Context<Self>) {
        self.state.resize(cols.max(1), rows.max(1));
        cx.notify();
    }

    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn update_config(&mut self, config: TerminalConfig, cx: &mut Context<Self>) {
        self.renderer.font_family = config.font_family.clone();
        self.renderer.font_size = config.font_size;
        self.renderer.line_height_multiplier = config.line_height_multiplier;
        self.renderer.palette = config.colors.clone();

        self.config = config;

        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.state.clear_screen();
        self.ime.clear();
        cx.notify();
    }

    pub fn has_selection(&self) -> bool {
        self.state
            .selection_text()
            .is_some_and(|text| !text.is_empty())
    }

    pub fn copy_selection(&self, cx: &mut Context<Self>) -> bool {
        if let Some(text) = self.state.selection_text().filter(|text| !text.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return true;
        }
        false
    }

    pub fn paste_from_clipboard(&self, cx: &mut Context<Self>) -> bool {
        if !self.input_enabled {
            return false;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let bytes = crate::input::paste_bytes(&text, self.state.mode());
            return self.write_input_bytes(&bytes, cx);
        }
        false
    }

    fn write_input_bytes(&self, bytes: &[u8], cx: &mut Context<Self>) -> bool {
        if !self.input_enabled || bytes.is_empty() {
            return false;
        }
        cx.emit(TerminalEvent::Input {
            bytes: bytes.to_vec(),
            user_input: true,
        });
        self.state.scroll_to_bottom();
        true
    }

    #[allow(dead_code)]
    fn calculate_dimensions(&self, bounds: Bounds<Pixels>) -> (usize, usize) {
        let width_f32: f32 = bounds.size.width.into();
        let height_f32: f32 = bounds.size.height.into();
        let cell_width_f32: f32 = self.renderer.cell_width.into();
        let cell_height_f32: f32 = self.renderer.cell_height.into();

        let cols = ((width_f32 / cell_width_f32) as usize).max(1);
        let rows = ((height_f32 / cell_height_f32) as usize).max(1);
        (cols, rows)
    }

    fn build_context_menu(
        view: WeakEntity<Self>,
        has_selection: bool,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        menu.item(
            PopupMenuItem::new("复制")
                .disabled(!has_selection)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        let _ = view.update(cx, |this, cx| {
                            let _ = this.copy_selection(cx);
                        });
                    }
                }),
        )
        .item(PopupMenuItem::new("粘贴").on_click({
            let view = view.clone();
            move |_, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    let _ = this.paste_from_clipboard(cx);
                    cx.notify();
                });
            }
        }))
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, range) = self.ime.text_for(_range_utf16);
        *adjusted_range = Some(range);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.ime.selected.clone(),
            reversed: false,
        })
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.ime.text.encode_utf16().count())
    }

    fn set_selected_text_range(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (_, range) = self.ime.text_for(range);
        self.ime.selected = range;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.ime.marked.clone()
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime.clear();
        _cx.notify();
    }

    fn paste(&mut self, item: ClipboardItem, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            let bytes = crate::input::paste_bytes(&text, self.state.mode());
            self.write_input_bytes(&bytes, cx);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime.clear();
        if !text.is_empty() {
            let _ = self.write_input_bytes(text.as_bytes(), cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input_enabled {
            return;
        }
        self.ime.mark(_range, new_text, new_selected_range);
        self.state.scroll_to_bottom();
        _window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let metrics = *self.layout_metrics.lock();
        let (before, _) = self.ime.text_for(0.._range_utf16.start);
        let columns = unicode_width::UnicodeWidthStr::width(before.as_str());
        self.state.with_term(|term| {
            let cursor = term.grid().cursor.point;
            let offset = term.grid().display_offset() as i32;
            let visible_row = cursor.line.0 + offset;
            Some(Bounds {
                origin: Point {
                    x: metrics.origin.x + metrics.cell_width * ((cursor.column.0 + columns) as f32),
                    y: metrics.origin.y + metrics.cell_height * (visible_row as f32),
                },
                size: Size {
                    width: metrics.cell_width.max(px(1.0)),
                    height: metrics.cell_height.max(px(1.0)),
                },
            })
        })
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.process_events(cx);

        let state_arc = self.state.term_arc();
        let renderer = self.renderer.clone();
        let padding = self.config.padding;
        let layout_metrics = self.layout_metrics.clone();
        let focus_handle = self.focus_handle.clone();
        let entity = cx.entity();
        let ime_text = self.ime.text.clone();
        let ime_marked_range = self.ime.marked.clone();

        div()
            .size_full()
            .bg(self.config.colors.background())
            .track_focus(&self.focus_handle)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::on_action_send_tab))
            .on_action(cx.listener(Self::on_action_send_backtab))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll))
            .context_menu({
                let view = cx.entity().downgrade();
                let has_selection = self.has_selection();
                move |menu, window, cx| {
                    Self::build_context_menu(view.clone(), has_selection, menu, window, cx)
                }
            })
            .child(
                canvas(
                    move |bounds, _window, _cx| bounds,
                    move |bounds, _, window, cx| {
                        use alacritty_terminal::grid::Dimensions;

                        window.handle_input(
                            &focus_handle,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );

                        let mut measured_renderer = renderer.clone();
                        measured_renderer.measure_cell(window);

                        let available_width: f32 =
                            (bounds.size.width - padding.left - padding.right).into();
                        let available_height: f32 =
                            (bounds.size.height - padding.top - padding.bottom).into();
                        let cell_width_f32: f32 = measured_renderer.cell_width.into();
                        let cell_height_f32: f32 = measured_renderer.cell_height.into();

                        let cols = ((available_width / cell_width_f32) as usize).max(1);
                        let rows = ((available_height / cell_height_f32) as usize).max(1);

                        let changed = {
                            let term = state_arc.lock();
                            cols != term.columns() || rows != term.screen_lines()
                        };
                        if changed {
                            entity.update(cx, |view, cx| {
                                view.state.resize(cols, rows);
                                cx.emit(TerminalEvent::Resize { cols, rows });
                            });
                        }
                        let term = state_arc.lock();

                        let origin = Point {
                            x: bounds.origin.x + padding.left,
                            y: bounds.origin.y + padding.top,
                        };
                        *layout_metrics.lock() = LayoutMetrics {
                            origin,
                            cell_width: measured_renderer.cell_width,
                            cell_height: measured_renderer.cell_height,
                            cols,
                        };

                        measured_renderer.paint(
                            bounds,
                            padding,
                            &term,
                            window,
                            cx,
                            (!ime_text.is_empty())
                                .then_some((ime_text.as_str(), ime_marked_range.as_ref())),
                        );
                    },
                )
                .size_full(),
            )
    }
}

impl EventEmitter<TerminalEvent> for TerminalView {}
