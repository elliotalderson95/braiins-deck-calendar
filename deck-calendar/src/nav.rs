//! Which day and month the screen shows, and how taps move them.

use crate::civil::{self, Ymd};

/// How long a day picked by a tap stays picked, in seconds.
pub const PICK_SECS: i64 = 90;
/// How far ahead the smaller tiles' taps and the week arrows reach, in days.
const REACH_AHEAD: i64 = 60;
/// How far back the week arrows reach, in days.
const REACH_BACK: i64 = 35;

/// What a tap asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Back,
    Forward,
    Today,
    Pick(i64),
    Cycle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nav {
    pub today: i64,
    /// The day the list starts from.
    pub selected: i64,
    /// The month the grid shows.
    pub month: (i64, u8),
    /// When a picked day goes back to today; 0 when none is picked.
    pub reset_at: i64,
}

impl Nav {
    #[must_use]
    pub fn new(today: i64) -> Self {
        Self {
            today,
            selected: today,
            month: month_of(today),
            reset_at: 0,
        }
    }

    /// Notes that it is a new day. Unless a day was picked, the screen goes
    /// to the new today.
    pub fn new_day(&mut self, today: i64) {
        self.today = today;
        if self.reset_at == 0 {
            self.selected = today;
            self.month = month_of(today);
        }
    }

    /// Lets a picked day lapse once its time is up.
    pub fn expire(&mut self, now: i64) {
        if self.reset_at != 0 && now >= self.reset_at {
            self.reset_at = 0;
            self.selected = self.today;
            self.month = month_of(self.today);
        }
    }

    /// Follows a tap. `browses_months` says whether the arrows move by month
    /// (the big tile) or by week; `next_busy` finds the first day from the
    /// one given that has events.
    pub fn apply(
        &mut self,
        tap: Tap,
        now: i64,
        browses_months: bool,
        next_busy: impl Fn(i64) -> Option<i64>,
    ) {
        match tap {
            Tap::Back | Tap::Forward => {
                let by = if tap == Tap::Back { -1 } else { 1 };
                if browses_months {
                    self.month = self.near(self.month, by);
                    self.selected = if self.month == month_of(self.today) {
                        self.today
                    } else {
                        civil::month_start(self.month.0, self.month.1)
                    };
                } else {
                    self.selected = (self.selected + 7 * by)
                        .clamp(self.today - REACH_BACK, self.today + REACH_AHEAD);
                    self.month = month_of(self.selected);
                }
            }
            Tap::Today => {
                self.selected = self.today;
                self.month = month_of(self.today);
            }
            Tap::Pick(day) => {
                self.selected = day;
                if browses_months && self.near(month_of(day), 0) == month_of(day) {
                    self.month = month_of(day);
                }
            }
            Tap::Cycle => {
                self.selected = match next_busy(self.selected + 1) {
                    Some(day) if day <= self.today + REACH_AHEAD => day,
                    _ => self.today,
                };
                if self.near(month_of(self.selected), 0) == month_of(self.selected) {
                    self.month = month_of(self.selected);
                }
            }
        }
        self.reset_at = if self.selected == self.today {
            0
        } else {
            now + PICK_SECS
        };
    }

    /// `month` moved by `by` months, if that stays within a month either side
    /// of this one (the months events are fetched for), else `month` as it was.
    fn near(&self, month: (i64, u8), by: i64) -> (i64, u8) {
        let today = month_of(self.today);
        let next = civil::add_months(month.0, month.1, by);
        if next >= civil::add_months(today.0, today.1, -1)
            && next <= civil::add_months(today.0, today.1, 1)
        {
            next
        } else {
            month
        }
    }
}

fn month_of(day: i64) -> (i64, u8) {
    let date = Ymd::from_days(day);
    (date.year, date.month)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i64, month: u8, day: u8) -> i64 {
        Ymd { year, month, day }.days()
    }

    fn nav() -> Nav {
        Nav::new(day(2026, 10, 15))
    }

    fn none(_: i64) -> Option<i64> {
        None
    }

    #[test]
    fn starts_on_today() {
        let n = nav();
        assert_eq!(
            (n.selected, n.month, n.reset_at),
            (day(2026, 10, 15), (2026, 10), 0)
        );
    }

    #[test]
    fn the_big_tiles_arrows_move_by_month_within_a_month_either_side() {
        let mut n = nav();
        n.apply(Tap::Forward, 1_000, true, none);
        assert_eq!((n.month, n.selected), ((2026, 11), day(2026, 11, 1)));
        assert_eq!(n.reset_at, 1_000 + PICK_SECS);
        n.apply(Tap::Forward, 1_000, true, none);
        assert_eq!(n.month, (2026, 11), "no further than next month");
        n.apply(Tap::Back, 1_000, true, none);
        assert_eq!(
            (n.month, n.selected, n.reset_at),
            ((2026, 10), day(2026, 10, 15), 0),
            "back to this month is today"
        );
        n.apply(Tap::Back, 1_000, true, none);
        assert_eq!((n.month, n.selected), ((2026, 9), day(2026, 9, 1)));
        n.apply(Tap::Back, 1_000, true, none);
        assert_eq!(n.month, (2026, 9), "no further back than last month");
    }

    #[test]
    fn the_other_tiles_arrows_move_by_week() {
        let mut n = nav();
        n.apply(Tap::Forward, 5, false, none);
        assert_eq!(n.selected, day(2026, 10, 22));
        n.apply(Tap::Back, 5, false, none);
        n.apply(Tap::Back, 5, false, none);
        assert_eq!(n.selected, day(2026, 10, 8));
        for _ in 0..20 {
            n.apply(Tap::Back, 5, false, none);
        }
        assert_eq!(n.selected, day(2026, 10, 15) - REACH_BACK);
    }

    #[test]
    fn a_picked_day_lapses_after_a_while() {
        let mut n = nav();
        n.apply(Tap::Pick(day(2026, 10, 20)), 100, true, none);
        assert_eq!(n.selected, day(2026, 10, 20));
        n.expire(100 + PICK_SECS - 1);
        assert_eq!(n.selected, day(2026, 10, 20));
        n.expire(100 + PICK_SECS);
        assert_eq!((n.selected, n.reset_at), (day(2026, 10, 15), 0));
    }

    #[test]
    fn picking_a_neighbouring_months_day_turns_the_grid_to_it() {
        let mut n = nav();
        n.apply(Tap::Pick(day(2026, 11, 2)), 0, true, none);
        assert_eq!((n.month, n.selected), ((2026, 11), day(2026, 11, 2)));
    }

    #[test]
    fn today_brings_everything_back() {
        let mut n = nav();
        n.apply(Tap::Forward, 0, true, none);
        n.apply(Tap::Today, 0, true, none);
        assert_eq!(
            (n.selected, n.month, n.reset_at),
            (day(2026, 10, 15), (2026, 10), 0)
        );
    }

    #[test]
    fn cycling_goes_to_the_next_day_with_events_then_back_to_today() {
        let busy = |from: i64| {
            [day(2026, 10, 17), day(2026, 10, 20)]
                .into_iter()
                .find(|&d| d >= from)
        };
        let mut n = nav();
        n.apply(Tap::Cycle, 0, false, busy);
        assert_eq!(n.selected, day(2026, 10, 17));
        n.apply(Tap::Cycle, 0, false, busy);
        assert_eq!(n.selected, day(2026, 10, 20));
        n.apply(Tap::Cycle, 0, false, busy);
        assert_eq!((n.selected, n.reset_at), (day(2026, 10, 15), 0));
    }

    #[test]
    fn a_new_day_moves_the_screen_on_unless_a_day_was_picked() {
        let mut n = nav();
        n.new_day(day(2026, 10, 16));
        assert_eq!(n.selected, day(2026, 10, 16));
        n.apply(Tap::Pick(day(2026, 10, 20)), 0, true, none);
        n.new_day(day(2026, 10, 17));
        assert_eq!(
            (n.today, n.selected),
            (day(2026, 10, 17), day(2026, 10, 20))
        );
        n.new_day(day(2026, 11, 1));
        n.expire(PICK_SECS);
        assert_eq!((n.selected, n.month), (day(2026, 11, 1), (2026, 11)));
    }
}
