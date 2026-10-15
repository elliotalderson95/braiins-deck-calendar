//! Calendar dates as plain day numbers (days since 1970-01-01), so date
//! arithmetic is integer arithmetic and the same code runs on the Deck and in
//! native tests.

pub const SECS_PER_DAY: i64 = 86_400;

const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const LONG_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const SHORT_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const LONG_WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// A calendar date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ymd {
    pub year: i64,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
}

impl Ymd {
    #[must_use]
    pub fn from_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        Self {
            year: yoe + era * 400 + i64::from(month <= 2),
            month: u8::try_from(month).expect("BUG: a month is 1 to 12"),
            day: u8::try_from(day).expect("BUG: a day is 1 to 31"),
        }
    }

    /// The date's day number; out-of-range months and days roll over the way
    /// `2026-14-35` would on a wall calendar's arithmetic, which suits
    /// month-end sums but is not a validity check (see [`Ymd::is_valid`]).
    #[must_use]
    pub fn days(self) -> i64 {
        let (year, month, day) = (self.year, i64::from(self.month), i64::from(self.day));
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = (month + 9) % 12;
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Whether this is a date that exists, such as not 31 June.
    #[must_use]
    pub fn is_valid(self) -> bool {
        (1..=12).contains(&self.month)
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
    }
}

#[must_use]
pub fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[must_use]
pub fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// 0 for Monday up to 6 for Sunday.
#[must_use]
pub fn weekday(days: i64) -> u8 {
    // 1970-01-01 was a Thursday.
    u8::try_from((days + 3).rem_euclid(7)).expect("BUG: rem_euclid(7) is 0 to 6")
}

/// The day number of the first of `month` (1 to 12) in `year`.
#[must_use]
pub fn month_start(year: i64, month: u8) -> i64 {
    Ymd {
        year,
        month,
        day: 1,
    }
    .days()
}

/// The year and month one month on from `year`/`month`, by `delta` months.
#[must_use]
pub fn add_months(year: i64, month: u8, delta: i64) -> (i64, u8) {
    let index = year * 12 + i64::from(month) - 1 + delta;
    (
        index.div_euclid(12),
        u8::try_from(index.rem_euclid(12) + 1).expect("BUG: a month is 1 to 12"),
    )
}

#[must_use]
pub fn short_month(month: u8) -> &'static str {
    SHORT_MONTHS[usize::from(month.clamp(1, 12)) - 1]
}

#[must_use]
pub fn long_month(month: u8) -> &'static str {
    LONG_MONTHS[usize::from(month.clamp(1, 12)) - 1]
}

/// `weekday` is 0 for Monday up to 6 for Sunday.
#[must_use]
pub fn short_weekday(weekday: u8) -> &'static str {
    SHORT_WEEKDAYS[usize::from(weekday % 7)]
}

#[must_use]
pub fn long_weekday(weekday: u8) -> &'static str {
    LONG_WEEKDAYS[usize::from(weekday % 7)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_numbers_match_known_dates() {
        assert_eq!(
            Ymd {
                year: 1970,
                month: 1,
                day: 1
            }
            .days(),
            0
        );
        assert_eq!(
            Ymd {
                year: 2000,
                month: 3,
                day: 1
            }
            .days(),
            11_017
        );
        assert_eq!(
            Ymd {
                year: 2026,
                month: 10,
                day: 15
            }
            .days(),
            20_741
        );
        assert_eq!(
            Ymd {
                year: 1969,
                month: 12,
                day: 31
            }
            .days(),
            -1
        );
    }

    #[test]
    fn dates_survive_the_round_trip() {
        for days in (-800_000..800_000).step_by(37) {
            let date = Ymd::from_days(days);
            assert!(date.is_valid(), "{date:?}");
            assert_eq!(date.days(), days);
        }
    }

    #[test]
    fn weekdays_count_from_monday() {
        // 15 October 2026 is a Thursday, 1 January 1970 too.
        assert_eq!(
            weekday(
                Ymd {
                    year: 2026,
                    month: 10,
                    day: 15
                }
                .days()
            ),
            3
        );
        assert_eq!(weekday(0), 3);
        assert_eq!(weekday(-1), 2);
        assert_eq!(
            weekday(
                Ymd {
                    year: 2024,
                    month: 2,
                    day: 29
                }
                .days()
            ),
            3
        );
    }

    #[test]
    fn month_lengths_follow_the_leap_rule() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 4), 30);
        assert!(
            !Ymd {
                year: 2026,
                month: 6,
                day: 31
            }
            .is_valid()
        );
    }

    #[test]
    fn months_step_across_years() {
        assert_eq!(add_months(2026, 12, 1), (2027, 1));
        assert_eq!(add_months(2026, 1, -1), (2025, 12));
        assert_eq!(add_months(2026, 10, 0), (2026, 10));
        assert_eq!(add_months(2026, 3, -15), (2024, 12));
    }
}
