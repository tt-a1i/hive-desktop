use super::snoozes::{Delay, now_ms};
use crate::{language::tr, palette::Color};
use chrono::{
    DateTime, Days, FixedOffset, Local, LocalResult, NaiveDateTime, TimeDelta, TimeZone, Timelike,
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        date_picker::{DatePicker, DatePickerEvent, DatePickerState},
        time_field::TimePrecision,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::rc::Rc;

const MAX_CUSTOM_DAYS: i64 = 366;

#[derive(Clone, Copy)]
pub(super) struct Deadline(DateTime<FixedOffset>);

impl Deadline {
    fn from_wall_time<Tz: TimeZone>(value: LocalResult<DateTime<Tz>>) -> Option<Self> {
        // An overlapping wall time uses its first occurrence. A gap has no
        // instant to offer. Show the offset so the selected occurrence is clear.
        value.earliest().map(|value| Self(value.fixed_offset()))
    }

    fn from_local(value: NaiveDateTime) -> Option<Self> {
        Self::from_wall_time(Local.from_local_datetime(&value))
    }

    fn epoch(self) -> u64 {
        self.0.timestamp_millis().try_into().unwrap_or(0)
    }

    pub(super) fn delay(self) -> Delay {
        Delay::Until(self.epoch())
    }

    pub(super) fn label(self) -> String {
        self.0.format("%Y-%m-%d %H:%M:%S (UTC%:z)").to_string()
    }

    fn custom(self, now: u64) -> Result<Self, &'static str> {
        let epoch = self.epoch();
        if epoch <= now {
            return Err("请选择未来的暂缓时间。");
        }
        if epoch - now > MAX_CUSTOM_DAYS as u64 * 86_400_000 {
            return Err("请选择未来一年内的暂缓时间。");
        }
        Ok(self)
    }
}

fn tomorrow_wall_time(now: NaiveDateTime) -> NaiveDateTime {
    now.date()
        .succ_opt()
        .unwrap_or(now.date())
        .and_time(now.time())
}

pub(super) fn presets() -> [(&'static str, Option<Deadline>); 3] {
    let now = Local::now();
    let now = now.with_nanosecond(0).unwrap_or(now);
    [
        (
            "15 分钟",
            Some(Deadline((now + TimeDelta::minutes(15)).fixed_offset())),
        ),
        (
            "1 小时",
            Some(Deadline((now + TimeDelta::hours(1)).fixed_offset())),
        ),
        (
            "明天此时",
            Deadline::from_local(tomorrow_wall_time(now.naive_local())),
        ),
    ]
}

type Save = Rc<dyn Fn(Delay, &mut App) -> Result<(), &'static str>>;

struct CustomSnooze {
    picker: Entity<DatePickerState>,
    deadline: Result<Deadline, &'static str>,
    save: Save,
    save_error: Option<&'static str>,
}

impl CustomSnooze {
    fn new(save: Save, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let now = Local::now();
        let today = now.date_naive();
        let last_day = today
            .checked_add_days(Days::new(MAX_CUSTOM_DAYS as u64))
            .unwrap_or(today);
        let tomorrow_morning = tomorrow_wall_time(now.naive_local())
            .date()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let picker = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx)
                .date_format("%Y-%m-%d %H:%M")
                .time_precision(TimePrecision::Minute)
                .disabled_matcher(move |day: &chrono::NaiveDate| *day < today || *day > last_day);
            state.set_date_time(tomorrow_morning, window, cx);
            state
        });
        cx.subscribe(&picker, |this, _, event, cx| {
            let DatePickerEvent::Change(value) = event;
            this.deadline = Self::resolve(value.start());
            this.save_error = None;
            cx.notify();
        })
        .detach();
        Self {
            picker,
            deadline: Self::resolve(Some(tomorrow_morning)),
            save,
            save_error: None,
        }
    }

    fn resolve(wall_time: Option<NaiveDateTime>) -> Result<Deadline, &'static str> {
        let wall_time = wall_time.ok_or("请选择日期和时间。")?;
        Deadline::from_local(wall_time)
            .ok_or("所选时间在本地时区不存在，请选择其他时间。")?
            .custom(now_ms())
    }

    fn confirm(&mut self, cx: &mut Context<Self>) -> bool {
        // The stored Deadline is the resolved time already shown to the user.
        match self
            .deadline
            .and_then(|deadline| deadline.custom(now_ms()))
            .and_then(|deadline| (self.save)(deadline.delay(), cx))
        {
            Ok(()) => true,
            Err(error) => {
                self.save_error = Some(error);
                cx.notify();
                false
            }
        }
    }
}

impl Render for CustomSnooze {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .min_w_0()
            .child(DatePicker::new(&self.picker))
            .when_some(self.deadline.ok(), |view, deadline| {
                let preview = format!("{} · {}", tr("唤回时间"), deadline.label());
                view.child(
                    div()
                        .id("snooze-deadline-preview")
                        .role(Role::Label)
                        .aria_label(preview.clone())
                        .text_sm()
                        .child(preview),
                )
            })
            .when_some(
                self.save_error.or_else(|| self.deadline.err()),
                |view, error| {
                    view.child(
                        div()
                            .id("snooze-deadline-error")
                            .role(Role::Alert)
                            .aria_label(tr(error))
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child(tr(error)),
                    )
                },
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("cancel-custom-snooze")
                            .ghost()
                            .label(tr("取消"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("save-custom-snooze")
                            .primary()
                            .label(tr("保存暂缓"))
                            .disabled(self.deadline.is_err())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.confirm(cx) {
                                    window.close_dialog(cx);
                                }
                            })),
                    ),
            )
    }
}

pub(super) fn open_custom_snooze(save: Save, window: &mut Window, cx: &mut App) {
    // PopupMenu restores focus after activating its item. Open the next dialog
    // after that restoration so keyboard focus belongs to the picker dialog.
    window.defer(cx, move |window, cx| {
        let picker = cx.new(|cx| CustomSnooze::new(save, window, cx));
        let width = (window.viewport_size().width - px(36.)).min(px(420.));
        window.open_dialog(cx, move |dialog, _, _| {
            let confirm = picker.downgrade();
            dialog
                .title(tr("自定义暂缓时间"))
                .w(width)
                .child(picker.clone())
                .on_ok(move |_, _, cx| {
                    confirm
                        .update(cx, |picker, cx| picker.confirm(cx))
                        .unwrap_or(false)
                })
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{Deadline, MAX_CUSTOM_DAYS, tomorrow_wall_time};
    use chrono::{FixedOffset, LocalResult, TimeZone};

    #[test]
    fn custom_deadlines_validate_future_range_and_show_the_selected_offset() {
        let early = FixedOffset::west_opt(7 * 3600)
            .unwrap()
            .with_ymd_and_hms(2027, 11, 7, 1, 30, 0)
            .unwrap();
        let late = FixedOffset::west_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2027, 11, 7, 1, 30, 0)
            .unwrap();
        assert!(Deadline::from_wall_time::<FixedOffset>(LocalResult::None).is_none());
        let deadline = Deadline::from_wall_time(LocalResult::Ambiguous(early, late)).unwrap();
        assert_eq!(deadline.epoch(), early.timestamp_millis() as u64);
        assert!(deadline.label().ends_with("(UTC-07:00)"));
        assert!(deadline.custom(deadline.epoch()).is_err());
        let max = MAX_CUSTOM_DAYS as u64 * 86_400_000;
        assert!(deadline.custom(deadline.epoch() - max).is_ok());
        assert!(deadline.custom(deadline.epoch() - max - 1).is_err());
    }

    #[test]
    fn tomorrow_advances_the_calendar_and_keeps_the_wall_clock() {
        let zone = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = zone
            .with_ymd_and_hms(2027, 12, 31, 18, 42, 9)
            .unwrap()
            .naive_local();
        let next = zone
            .with_ymd_and_hms(2028, 1, 1, 18, 42, 9)
            .unwrap()
            .naive_local();
        assert_eq!(tomorrow_wall_time(now), next);
    }

    #[cfg(unix)]
    #[test]
    fn tomorrow_keeps_wall_time_in_a_real_dst_zone() {
        use chrono::Local;
        const CHILD: &str = "HIVE_INBOX_DST_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let path = module_path!().split_once("::").unwrap().1;
            let outcome = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(format!(
                    "{path}::tomorrow_keeps_wall_time_in_a_real_dst_zone"
                ))
                .arg("--nocapture")
                .env(CHILD, "1")
                .env("TZ", "America/Los_Angeles")
                .output()
                .unwrap();
            let report = String::from_utf8_lossy(&outcome.stdout);
            assert!(
                outcome.status.success() && report.contains("1 passed"),
                "{report}\n{}",
                String::from_utf8_lossy(&outcome.stderr)
            );
            return;
        }
        let now = Local.with_ymd_and_hms(2025, 3, 8, 9, 30, 0).unwrap();
        let tomorrow = Deadline::from_local(tomorrow_wall_time(now.naive_local())).unwrap();
        assert_eq!(
            tomorrow.epoch() - now.timestamp_millis() as u64,
            23 * 3_600_000
        );
        assert!(tomorrow.label().ends_with("09:30:00 (UTC-07:00)"));
        let gap = Local.with_ymd_and_hms(2025, 3, 8, 2, 30, 0).unwrap();
        assert!(Deadline::from_local(tomorrow_wall_time(gap.naive_local())).is_none());
        let now = Local.with_ymd_and_hms(2025, 11, 1, 9, 30, 0).unwrap();
        let tomorrow = Deadline::from_local(tomorrow_wall_time(now.naive_local())).unwrap();
        assert_eq!(
            tomorrow.epoch() - now.timestamp_millis() as u64,
            25 * 3_600_000
        );
        assert!(tomorrow.label().ends_with("09:30:00 (UTC-08:00)"));
    }
}
