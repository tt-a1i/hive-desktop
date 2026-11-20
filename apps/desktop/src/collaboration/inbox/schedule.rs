use super::snoozes::{Delay, now_ms};
use crate::{language::tr, palette::Color};
use chrono::{
    DateTime, Datelike, Days, FixedOffset, Local, LocalResult, NaiveDate, NaiveDateTime, TimeDelta,
    TimeZone, Timelike,
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

const MAX_CUSTOM_DAYS: u64 = 366;

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

    pub(super) fn from_epoch_local(epoch: u64) -> Option<Self> {
        DateTime::from_timestamp_millis(epoch.try_into().ok()?)
            .map(|utc| Self(utc.with_timezone(&Local).fixed_offset()))
    }

    fn epoch(self) -> u64 {
        self.0.timestamp_millis().try_into().unwrap_or(0)
    }

    pub(super) fn delay(self) -> Delay {
        Delay::Until(self.epoch())
    }

    pub(super) fn label(self) -> String {
        let format = if self.0.timestamp_subsec_millis() == 0 {
            "%Y-%m-%d %H:%M:%S (UTC%:z)"
        } else {
            "%Y-%m-%d %H:%M:%S%.3f (UTC%:z)"
        };
        self.0.format(format).to_string()
    }

    fn custom(self, now: u64) -> Result<Self, &'static str> {
        let epoch = self.epoch();
        if epoch <= now {
            return Err("请选择未来的暂缓时间。");
        }
        if epoch - now > MAX_CUSTOM_DAYS * 86_400_000 {
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

pub(super) fn presets() -> Vec<(&'static str, Option<Deadline>)> {
    presets_at(Local::now())
}

fn presets_at<Tz: TimeZone>(now: DateTime<Tz>) -> Vec<(&'static str, Option<Deadline>)> {
    let now = now.with_nanosecond(0).unwrap_or(now);
    let zone = now.timezone();
    let today = now.date_naive();
    let at = |date: Option<NaiveDate>, hour| {
        Deadline::from_wall_time(zone.from_local_datetime(&date?.and_hms_opt(hour, 0, 0)?))
    };
    let next_monday = today.checked_add_days(Days::new(
        7 - u64::from(today.weekday().num_days_from_monday()),
    ));
    let mut options = vec![
        (
            "15 分钟",
            Some(Deadline(
                (now.clone() + TimeDelta::minutes(15)).fixed_offset(),
            )),
        ),
        (
            "1 小时",
            Some(Deadline((now.clone() + TimeDelta::hours(1)).fixed_offset())),
        ),
        (
            "3 小时",
            Some(Deadline((now.clone() + TimeDelta::hours(3)).fixed_offset())),
        ),
    ];
    if let Some(tonight) = at(Some(today), 18)
        && tonight.0 > now
    {
        options.push(("今晚 18:00", Some(tonight)));
    }
    options.extend([
        ("明天 09:00", at(today.succ_opt(), 9)),
        (
            "明天此时",
            Deadline::from_wall_time(
                zone.from_local_datetime(&tomorrow_wall_time(now.naive_local())),
            ),
        ),
        ("下周一 09:00", at(next_monday, 9)),
    ]);
    options
}

type Save = Rc<dyn Fn(Delay, &mut App) -> Result<(), &'static str>>;

fn resolve_custom(wall_time: Option<NaiveDateTime>, now: u64) -> Result<Deadline, &'static str> {
    let wall_time = wall_time.ok_or("请选择日期和时间。")?;
    Deadline::from_local(wall_time)
        .ok_or("所选时间在本地时区不存在，请选择其他时间。")?
        .custom(now)
}

fn custom_prefill(
    previous: Option<u64>,
    now: DateTime<Local>,
) -> (NaiveDateTime, Result<Deadline, &'static str>) {
    let epoch = now.timestamp_millis() as u64;
    if let Some(previous) = previous.and_then(Deadline::from_epoch_local) {
        return (previous.0.naive_local(), previous.custom(epoch));
    }
    let tomorrow_morning = tomorrow_wall_time(now.naive_local())
        .date()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    (
        tomorrow_morning,
        resolve_custom(Some(tomorrow_morning), epoch),
    )
}

struct CustomSnooze {
    picker: Entity<DatePickerState>,
    deadline: Result<Deadline, &'static str>,
    save: Save,
    save_error: Option<&'static str>,
}

impl CustomSnooze {
    fn new(
        save: Save,
        previous_deadline: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let now = Local::now();
        let today = now.date_naive();
        let last_day = today
            .checked_add_days(Days::new(MAX_CUSTOM_DAYS))
            .unwrap_or(today);
        let (initial_wall_time, deadline) = custom_prefill(previous_deadline, now);
        let include_seconds =
            initial_wall_time.second() != 0 || initial_wall_time.nanosecond() != 0;
        let (format, precision) = if include_seconds {
            ("%Y-%m-%d %H:%M:%S", TimePrecision::Second)
        } else {
            ("%Y-%m-%d %H:%M", TimePrecision::Minute)
        };
        let picker = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx)
                .date_format(format)
                .time_precision(precision)
                .disabled_matcher(move |day: &chrono::NaiveDate| *day < today || *day > last_day);
            state.set_date_time(initial_wall_time, window, cx);
            state
        });
        cx.subscribe(&picker, |this, _, event, cx| {
            let DatePickerEvent::Change(value) = event;
            this.deadline = resolve_custom(value.start(), now_ms());
            this.save_error = None;
            cx.notify();
        })
        .detach();
        Self {
            picker,
            deadline,
            save,
            save_error: None,
        }
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

pub(super) fn open_custom_snooze(
    save: Save,
    previous_deadline: Option<u64>,
    window: &mut Window,
    cx: &mut App,
) {
    // PopupMenu restores focus after activating its item. Open the next dialog
    // after that restoration so keyboard focus belongs to the picker dialog.
    window.defer(cx, move |window, cx| {
        let picker = cx.new(|cx| CustomSnooze::new(save, previous_deadline, window, cx));
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
    use super::{Deadline, MAX_CUSTOM_DAYS, presets_at, tomorrow_wall_time};
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
        let max = MAX_CUSTOM_DAYS * 86_400_000;
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

    #[test]
    fn calendar_presets_include_only_a_future_evening_and_use_the_following_monday() {
        let zone = FixedOffset::east_opt(8 * 3600).unwrap();
        let preview = |hour| {
            presets_at(zone.with_ymd_and_hms(2027, 1, 4, hour, 0, 0).unwrap())
                .into_iter()
                .filter_map(|(id, deadline)| Some((id, deadline?.label())))
                .collect::<std::collections::HashMap<_, _>>()
        };
        let early = preview(14);
        assert_eq!(early["3 小时"], "2027-01-04 17:00:00 (UTC+08:00)");
        assert_eq!(early["今晚 18:00"], "2027-01-04 18:00:00 (UTC+08:00)");
        assert_eq!(early["明天 09:00"], "2027-01-05 09:00:00 (UTC+08:00)");
        assert_eq!(early["下周一 09:00"], "2027-01-11 09:00:00 (UTC+08:00)");
        assert!(!preview(18).contains_key("今晚 18:00"));
    }

    #[cfg(unix)]
    #[test]
    fn tomorrow_keeps_wall_time_in_a_real_dst_zone() {
        use super::custom_prefill;
        use chrono::{Local, Timelike, Utc};
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
        let calendar = presets_at(now);
        let label = |id| {
            calendar
                .iter()
                .find(|(name, _)| *name == id)
                .unwrap()
                .1
                .unwrap()
                .label()
        };
        assert_eq!(label("明天 09:00"), "2025-03-09 09:00:00 (UTC-07:00)");
        assert_eq!(label("下周一 09:00"), "2025-03-10 09:00:00 (UTC-07:00)");
        let gap = Local.with_ymd_and_hms(2025, 3, 8, 2, 30, 0).unwrap();
        assert!(Deadline::from_local(tomorrow_wall_time(gap.naive_local())).is_none());
        let now = Local.with_ymd_and_hms(2025, 11, 1, 9, 30, 0).unwrap();
        let tomorrow = Deadline::from_local(tomorrow_wall_time(now.naive_local())).unwrap();
        assert_eq!(
            tomorrow.epoch() - now.timestamp_millis() as u64,
            25 * 3_600_000
        );
        assert!(tomorrow.label().ends_with("09:30:00 (UTC-08:00)"));
        // An existing timestamp from the second overlap occurrence stays exact
        // when opened in another zone, including subsecond precision on old files.
        let stored = Utc
            .with_ymd_and_hms(2025, 11, 2, 9, 30, 11)
            .unwrap()
            .with_nanosecond(123_000_000)
            .unwrap()
            .timestamp_millis() as u64;
        let (wall, preview) = custom_prefill(Some(stored), now);
        let deadline = preview.unwrap();
        assert_eq!(
            wall.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2025-11-02 01:30:11"
        );
        assert_eq!(deadline.label(), "2025-11-02 01:30:11.123 (UTC-08:00)");
        assert_eq!(deadline.epoch(), stored);
    }
}
