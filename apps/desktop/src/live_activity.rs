use crate::app::HiveApp;
use crate::language::tr;
use crate::palette::{Color, lerp_rgba};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, HighlightStyle, IntoElement,
    ParentElement as _, Render, Rgba, SharedString, Styled as _, StyledText, Subscription, Task,
    Window, div, px,
};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::time::{Duration, Instant};

const ANIM_FRAME_INTERVAL: Duration = Duration::from_millis(66);

pub(crate) fn format_elapsed_seconds(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        if m > 0 && s > 0 {
            format!("{h}h {m}m {s}s")
        } else if m > 0 {
            format!("{h}h {m}m")
        } else if s > 0 {
            format!("{h}h {s}s")
        } else {
            format!("{h}h")
        }
    }
}

pub(crate) fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 10_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format_elapsed_seconds(ms / 1000)
    }
}

pub(crate) fn pulse_alpha(phase: f32, min: f32, max: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    let wave = 0.5 + 0.5 * (phase * std::f32::consts::TAU).cos();
    min + (max - min) * wave
}

pub(crate) fn shine_highlight_ranges(
    text: &str,
    phase: f32,
    base: Rgba,
    peak: Rgba,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let char_count = text.chars().count();
    if char_count == 0 {
        return Vec::new();
    }
    let center = -0.25 + phase.clamp(0.0, 1.0) * 1.50;
    let half_width = 0.24_f32;
    let mut ranges: Vec<(Range<usize>, u8)> = Vec::new();

    for (idx, (byte_start, ch)) in text.char_indices().enumerate() {
        let byte_end = byte_start + ch.len_utf8();
        let pos = (idx as f32 + 0.5) / (char_count as f32);
        let dist = ((pos - center) / half_width).abs();
        let weight = if dist < 1.0 {
            0.5 * (1.0 + (std::f32::consts::PI * dist).cos())
        } else {
            0.0
        };
        let step = if weight <= 0.08 {
            0
        } else {
            ((weight * 5.0).round() as u8).clamp(1, 5)
        };
        if step == 0 {
            continue;
        }
        if let Some((last_range, last_step)) = ranges.last_mut()
            && *last_step == step
            && last_range.end == byte_start
        {
            last_range.end = byte_end;
        } else {
            ranges.push((byte_start..byte_end, step));
        }
    }

    ranges
        .into_iter()
        .map(|(range, step)| {
            debug_assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
            let t = step as f32 / 5.0;
            let style = HighlightStyle {
                color: Some(lerp_rgba(base, peak, t).into()),
                ..Default::default()
            };
            (range, style)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LiveActivitySchedule {
    pub run_anim_clock: bool,
    pub run_elapsed_timer: bool,
}

pub(crate) fn compute_schedule(
    has_active_animations: bool,
    has_active_elapsed: bool,
    window_active: bool,
    reduce_motion: bool,
) -> LiveActivitySchedule {
    LiveActivitySchedule {
        run_anim_clock: has_active_animations && window_active && !reduce_motion,
        run_elapsed_timer: has_active_elapsed && window_active,
    }
}

pub(crate) struct LiveAnimClock {
    running: bool,
    epoch: Instant,
    task: Option<Task<()>>,
}

impl LiveAnimClock {
    pub(crate) fn new() -> Self {
        Self {
            running: false,
            epoch: Instant::now(),
            task: None,
        }
    }

    pub(crate) fn phase(&self, period_ms: u64) -> f32 {
        if !self.running || period_ms == 0 {
            return 0.0;
        }
        (self.epoch.elapsed().as_millis() % u128::from(period_ms)) as f32 / period_ms as f32
    }

    pub(crate) fn sync(&mut self, should_run: bool, cx: &mut Context<Self>) {
        if should_run {
            self.running = true;
            if self.task.is_none() {
                self.task = Some(cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(ANIM_FRAME_INTERVAL).await;
                    let keep_going = this
                        .update(cx, |clock, cx| {
                            if clock.running {
                                cx.notify();
                                true
                            } else {
                                clock.task = None;
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !keep_going {
                        break;
                    }
                }));
            }
        } else {
            self.running = false;
            self.task = None;
        }
    }
}

pub(crate) struct LivePulseDot {
    clock: Entity<LiveAnimClock>,
    color: Rgba,
    size_px: f32,
    animate: bool,
    _sub: Subscription,
}

impl LivePulseDot {
    fn new(
        clock: Entity<LiveAnimClock>,
        color: Rgba,
        size_px: f32,
        animate: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let sub = cx.observe(&clock, |this, _, cx| {
            if this.animate {
                cx.notify();
            }
        });
        Self {
            clock,
            color,
            size_px,
            animate,
            _sub: sub,
        }
    }

    fn update_props(&mut self, color: Rgba, size_px: f32, animate: bool, cx: &mut Context<Self>) {
        if self.color != color || (self.size_px - size_px).abs() > f32::EPSILON || self.animate != animate {
            self.color = color;
            self.size_px = size_px;
            self.animate = animate;
            cx.notify();
        }
    }
}

impl Render for LivePulseDot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let clock = self.clock.read(cx);
        let alpha = if self.animate && clock.running {
            pulse_alpha(clock.phase(2000), 0.42, 1.0)
        } else {
            1.0
        };
        div()
            .size(px(self.size_px))
            .flex_shrink_0()
            .rounded_full()
            .bg(self.color.opacity(alpha))
    }
}

pub(crate) struct LiveShineLabel {
    clock: Entity<LiveAnimClock>,
    text: SharedString,
    base: Rgba,
    peak: Rgba,
    font_size_px: f32,
    font_weight: FontWeight,
    animate: bool,
    _sub: Subscription,
}

impl LiveShineLabel {
    fn new(
        clock: Entity<LiveAnimClock>,
        text: SharedString,
        base: Rgba,
        peak: Rgba,
        font_size_px: f32,
        font_weight: FontWeight,
        animate: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let sub = cx.observe(&clock, |this, _, cx| {
            if this.animate {
                cx.notify();
            }
        });
        Self {
            clock,
            text,
            base,
            peak,
            font_size_px,
            font_weight,
            animate,
            _sub: sub,
        }
    }

    fn update_props(
        &mut self,
        text: SharedString,
        base: Rgba,
        peak: Rgba,
        font_size_px: f32,
        font_weight: FontWeight,
        animate: bool,
        cx: &mut Context<Self>,
    ) {
        if self.text != text
            || self.base != base
            || self.peak != peak
            || (self.font_size_px - font_size_px).abs() > f32::EPSILON
            || self.font_weight != font_weight
            || self.animate != animate
        {
            self.text = text;
            self.base = base;
            self.peak = peak;
            self.font_size_px = font_size_px;
            self.font_weight = font_weight;
            self.animate = animate;
            cx.notify();
        }
    }
}

impl Render for LiveShineLabel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let clock = self.clock.read(cx);
        let container = div()
            .flex_shrink_0()
            .text_size(px(self.font_size_px))
            .font_weight(self.font_weight)
            .text_color(self.base);
        if self.animate && clock.running {
            let highlights =
                shine_highlight_ranges(&self.text, clock.phase(2200), self.base, self.peak);
            container.child(StyledText::new(self.text.clone()).with_highlights(highlights))
        } else {
            container.child(self.text.clone())
        }
    }
}

pub(crate) struct LiveActivityState {
    pub busy_started_at: HashMap<String, Instant>,
    pub busy_unknown_start: HashSet<String>,
    pub clock: Entity<LiveAnimClock>,
    pub pulse_dots: HashMap<String, Entity<LivePulseDot>>,
    pub shine_labels: HashMap<String, Entity<LiveShineLabel>>,
    pub elapsed_task: Option<Task<()>>,
    pub window_active: bool,
}

impl LiveActivityState {
    pub(crate) fn new(window_active: bool, cx: &mut App) -> Self {
        Self {
            busy_started_at: HashMap::new(),
            busy_unknown_start: HashSet::new(),
            clock: cx.new(|_| LiveAnimClock::new()),
            pulse_dots: HashMap::new(),
            shine_labels: HashMap::new(),
            elapsed_task: None,
            window_active,
        }
    }

    pub(crate) fn mark_busy(&mut self, session_id: &str) {
        if self.busy_unknown_start.contains(session_id) {
            return;
        }
        self.busy_started_at
            .entry(session_id.to_owned())
            .or_insert_with(Instant::now);
    }

    pub(crate) fn mark_busy_unknown_start(&mut self, session_id: &str) {
        if !self.busy_started_at.contains_key(session_id) {
            self.busy_unknown_start.insert(session_id.to_owned());
        }
    }

    pub(crate) fn mark_busy_from_user_send(&mut self, session_id: &str) {
        self.busy_unknown_start.remove(session_id);
        self.mark_busy(session_id);
    }

    pub(crate) fn mark_busy_with_timestamp(&mut self, session_id: &str, started_at_ms: i64) {
        if self.busy_started_at.contains_key(session_id) {
            return;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        if started_at_ms > 0 && started_at_ms <= now_ms {
            let delta = (now_ms - started_at_ms) as u64;
            if delta <= 86_400_000
                && let Some(started) = Instant::now().checked_sub(Duration::from_millis(delta))
            {
                self.busy_unknown_start.remove(session_id);
                self.busy_started_at.insert(session_id.to_owned(), started);
                return;
            }
        }
        self.mark_busy_unknown_start(session_id);
    }

    pub(crate) fn unmark_busy(&mut self, session_id: &str) {
        self.busy_started_at.remove(session_id);
        self.busy_unknown_start.remove(session_id);
    }

    pub(crate) fn clear_busy(&mut self, cx: &mut App) {
        self.busy_started_at.clear();
        self.busy_unknown_start.clear();
        self.elapsed_task = None;
        self.pulse_dots.clear();
        self.shine_labels.clear();
        self.clock.update(cx, |clock, cx| clock.sync(false, cx));
    }

    pub(crate) fn on_window_activation(&mut self, active: bool, cx: &mut App) {
        self.window_active = active;
        if !active {
            self.elapsed_task = None;
            self.clock.update(cx, |clock, cx| clock.sync(false, cx));
        }
    }
}

impl HiveApp {
    pub(crate) fn sync_live_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_active = window.is_window_active();
        let reduce_motion = cx.reduce_motion();
        self.live_activity.window_active = window_active;

        let mut pulse_specs: HashMap<String, (Rgba, f32, bool)> = HashMap::new();
        let mut shine_specs: HashMap<
            String,
            (SharedString, Rgba, Rgba, f32, FontWeight, bool),
        > = HashMap::new();

        if self.sidebar_open {
            for id in &self.busy {
                if self.interaction_count(id) == 0 {
                    pulse_specs.insert(
                        format!("session:{id}"),
                        (Color::Accent.get(cx), 5., true),
                    );
                }
            }
        }

        let has_session_failure =
            self.agent_diagnostic.is_some() || self.delivery_failure.is_some();
        let current_session_id = self.session.as_ref().map(|s| s.id.as_str());
        let running = current_session_id.is_some_and(|id| self.busy.contains(id));
        let topbar_pill_visible = !has_session_failure
            && self.error.is_none()
            && (!self.status.is_background_connected() || running);
        let topbar_running = running && self.status.is_background_connected();
        if topbar_pill_visible {
            pulse_specs.insert(
                "topbar:dot".to_owned(),
                (Color::Accent.get(cx), 6., topbar_running),
            );
            if topbar_running {
                shine_specs.insert(
                    "topbar:label".to_owned(),
                    (
                        SharedString::from(tr("回复中")),
                        Color::Secondary.get(cx),
                        Color::Text.get(cx),
                        12.,
                        FontWeight::MEDIUM,
                        true,
                    ),
                );
            }
        }

        let mut has_active_thinking_elapsed = false;
        let mut has_active_tool_elapsed = false;
        for message in &self.messages {
            for block in &message.thinking {
                if block.running {
                    if block.started_at.is_some() {
                        has_active_thinking_elapsed = true;
                    }
                    shine_specs.insert(
                        format!("thinking:{}:{}", message.id, block.index),
                        (
                            SharedString::from(tr("思考中…")),
                            Color::Muted.get(cx),
                            Color::Text.get(cx),
                            12.,
                            FontWeight::MEDIUM,
                            true,
                        ),
                    );
                }
            }
            if let Some(tool) = &message.tool
                && tool.status == "执行中"
            {
                if tool.started_at.is_some() {
                    has_active_tool_elapsed = true;
                }
                let args = self.tool_inputs.get(&message.id).filter(|a| !a.is_null());
                let label = crate::tool_message::tool_action_label(&tool.name, tool.status, args);
                shine_specs.insert(
                    format!("tool:{}", message.id),
                    (
                        SharedString::from(label),
                        Color::Secondary.get(cx),
                        Color::Accent.get(cx),
                        12.,
                        FontWeight::MEDIUM,
                        true,
                    ),
                );
            }
        }

        self.live_activity
            .pulse_dots
            .retain(|k, _| pulse_specs.contains_key(k));
        for (key, (color, size_px, animate)) in pulse_specs {
            if let Some(dot) = self.live_activity.pulse_dots.get(&key).cloned() {
                dot.update(cx, |dot, cx| dot.update_props(color, size_px, animate, cx));
            } else {
                let clock = self.live_activity.clock.clone();
                let dot = cx.new(|cx| LivePulseDot::new(clock, color, size_px, animate, cx));
                self.live_activity.pulse_dots.insert(key, dot);
            }
        }

        self.live_activity
            .shine_labels
            .retain(|k, _| shine_specs.contains_key(k));
        for (key, (text, base, peak, font_size_px, font_weight, animate)) in shine_specs {
            if let Some(label) = self.live_activity.shine_labels.get(&key).cloned() {
                label.update(cx, |label, cx| {
                    label.update_props(text, base, peak, font_size_px, font_weight, animate, cx)
                });
            } else {
                let clock = self.live_activity.clock.clone();
                let label = cx.new(|cx| {
                    LiveShineLabel::new(
                        clock,
                        text,
                        base,
                        peak,
                        font_size_px,
                        font_weight,
                        animate,
                        cx,
                    )
                });
                self.live_activity.shine_labels.insert(key, label);
            }
        }

        let has_active_animations = self
            .live_activity
            .pulse_dots
            .values()
            .any(|dot| dot.read(cx).animate)
            || self
                .live_activity
                .shine_labels
                .values()
                .any(|label| label.read(cx).animate);
        let has_active_elapsed = (topbar_running
            && current_session_id.is_some_and(|id| self.live_activity.busy_started_at.contains_key(id)))
            || has_active_thinking_elapsed
            || has_active_tool_elapsed;

        let schedule = compute_schedule(
            has_active_animations,
            has_active_elapsed,
            window_active,
            reduce_motion,
        );

        self.live_activity
            .clock
            .update(cx, |clock, cx| clock.sync(schedule.run_anim_clock, cx));

        if schedule.run_elapsed_timer {
            if self.live_activity.elapsed_task.is_none() {
                self.live_activity.elapsed_task = Some(cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let keep_going = this
                        .update(cx, |app, cx| {
                            if app.live_activity.window_active && app.live_activity.elapsed_task.is_some() {
                                cx.notify();
                                true
                            } else {
                                app.live_activity.elapsed_task = None;
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !keep_going {
                        break;
                    }
                }));
            }
        } else {
            self.live_activity.elapsed_task = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::rgb;

    #[test]
    fn elapsed_and_duration_formatting_follow_compact_scales() {
        assert_eq!(format_elapsed_seconds(0), "0s");
        assert_eq!(format_elapsed_seconds(59), "59s");
        assert_eq!(format_elapsed_seconds(60), "1m 0s");
        assert_eq!(format_elapsed_seconds(65), "1m 5s");
        assert_eq!(format_elapsed_seconds(3600), "1h");
        assert_eq!(format_elapsed_seconds(3661), "1h 1m 1s");

        assert_eq!(format_duration_ms(0), "0ms");
        assert_eq!(format_duration_ms(42), "42ms");
        assert_eq!(format_duration_ms(950), "950ms");
        assert_eq!(format_duration_ms(1400), "1.4s");
        assert_eq!(format_duration_ms(9900), "9.9s");
        assert_eq!(format_duration_ms(10_000), "10s");
        assert_eq!(format_duration_ms(65_000), "1m 5s");
    }

    #[test]
    fn shine_highlights_stay_on_utf8_boundaries_for_mixed_and_cjk_text() {
        let base = rgb(0x888888);
        let peak = rgb(0xffffff);
        assert!(shine_highlight_ranges("", 0.5, base, peak).is_empty());

        for sample in ["回复中", "正在运行命令", "Thinking…", "A", "混合中英文 🚀 Text"] {
            for step in 0..=20 {
                let phase = step as f32 / 20.0;
                let ranges = shine_highlight_ranges(sample, phase, base, peak);
                let mut prev_end = 0;
                for (range, style) in ranges {
                    assert!(sample.is_char_boundary(range.start));
                    assert!(sample.is_char_boundary(range.end));
                    assert!(range.start < range.end);
                    assert!(range.start >= prev_end);
                    assert!(style.color.is_some());
                    prev_end = range.end;
                }
            }
        }
    }

    #[test]
    fn schedule_gates_timers_on_idle_inactive_and_reduce_motion() {
        assert_eq!(
            compute_schedule(false, false, true, false),
            LiveActivitySchedule {
                run_anim_clock: false,
                run_elapsed_timer: false,
            }
        );
        assert_eq!(
            compute_schedule(true, true, true, false),
            LiveActivitySchedule {
                run_anim_clock: true,
                run_elapsed_timer: true,
            }
        );
        assert_eq!(
            compute_schedule(true, true, false, false),
            LiveActivitySchedule {
                run_anim_clock: false,
                run_elapsed_timer: false,
            }
        );
        assert_eq!(
            compute_schedule(true, true, true, true),
            LiveActivitySchedule {
                run_anim_clock: false,
                run_elapsed_timer: true,
            }
        );
    }

    #[gpui_kit::test]
    fn mark_busy_with_timestamp_falls_back_to_unknown_start_for_invalid_or_stale_timestamps(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|cx| {
            let mut state = LiveActivityState::new(true, cx);
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            state.mark_busy_with_timestamp("s-zero", 0);
            assert!(state.busy_started_at.get("s-zero").is_none());
            assert!(state.busy_unknown_start.contains("s-zero"));

            state.mark_busy_with_timestamp("s-future", now_ms + 60_000);
            assert!(state.busy_started_at.get("s-future").is_none());
            assert!(state.busy_unknown_start.contains("s-future"));

            state.mark_busy_with_timestamp("s-stale", now_ms - 86_400_001);
            assert!(state.busy_started_at.get("s-stale").is_none());
            assert!(state.busy_unknown_start.contains("s-stale"));

            // Subsequent mark_busy must not fabricate 0s for unknown-start sessions.
            state.mark_busy("s-stale");
            assert!(state.busy_started_at.get("s-stale").is_none());

            // Valid timestamp clears unknown-start and records started_at.
            state.mark_busy_with_timestamp("s-stale", now_ms - 5_000);
            assert!(state.busy_started_at.get("s-stale").is_some());
            assert!(!state.busy_unknown_start.contains("s-stale"));

            // Already-recorded started_at is preserved even if a later call passes invalid timestamp.
            state.mark_busy_with_timestamp("s-stale", -1);
            assert!(state.busy_started_at.get("s-stale").is_some());
            assert!(!state.busy_unknown_start.contains("s-stale"));
        });
    }
}
