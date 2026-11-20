use serde::{Deserialize, Serialize};

const DAY_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum ActivityRange {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "3d")]
    ThreeDays,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
}

impl ActivityRange {
    pub(super) const OPTIONS: [Option<Self>; 5] = [
        None,
        Some(Self::OneDay),
        Some(Self::ThreeDays),
        Some(Self::SevenDays),
        Some(Self::ThirtyDays),
    ];

    pub(super) fn label(range: Option<Self>) -> &'static str {
        match range {
            None => "全部时间",
            Some(Self::OneDay) => "最近 1 天",
            Some(Self::ThreeDays) => "最近 3 天",
            Some(Self::SevenDays) => "最近 7 天",
            Some(Self::ThirtyDays) => "最近 30 天",
        }
    }

    pub(super) fn cutoff(self, wall_now: u64) -> u64 {
        let days = match self {
            Self::OneDay => 1,
            Self::ThreeDays => 3,
            Self::SevenDays => 7,
            Self::ThirtyDays => 30,
        };
        wall_now.saturating_sub(days * DAY_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::{ActivityRange, DAY_MS};
    use crate::collaboration::inbox::activity_matches;

    #[test]
    fn rolling_ranges_include_the_exact_boundary_but_never_an_unknown_time() {
        let now = 40 * DAY_MS + 42;
        for (days, range) in [1, 3, 7, 30]
            .into_iter()
            .zip(ActivityRange::OPTIONS.into_iter().flatten())
        {
            let cutoff = range.cutoff(now);
            assert_eq!(cutoff, now - days * DAY_MS);
            assert!(activity_matches(cutoff, Some(cutoff)));
            assert!(!activity_matches(cutoff - 1, Some(cutoff)));
        }
        assert!(activity_matches(0, None));
        assert!(!activity_matches(0, Some(ActivityRange::OneDay.cutoff(0))));
    }
}
