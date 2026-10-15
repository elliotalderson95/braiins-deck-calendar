//! What the screen shows: the events placed on the Deck's own calendar days,
//! the list of a day, the grid of a month, and the words for times and dates.

use std::collections::BTreeMap;

use crate::civil::{self, SECS_PER_DAY, Ymd};
use crate::expand::Occurrence;
use crate::zones::Zones;

/// The most calendars connected at once.
pub const MAX_CALENDARS: usize = 3;
/// The longest an event is spread over days; anything past this is cut short.
const MAX_SPAN_DAYS: i64 = 62;

/// One event on one occasion, placed on the Deck's calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub title: String,
    pub place: String,
    /// Which connected calendar it came from.
    pub source: u8,
    pub all_day: bool,
    /// Marked "free" in its calendar.
    pub free: bool,
    /// UTC seconds. A whole-day event starts at UTC midnight and ends at the
    /// UTC midnight after its last day.
    pub start: i64,
    pub end: i64,
    /// The day numbers (days since 1970) it covers on the Deck's calendar.
    pub first_day: i64,
    pub last_day: i64,
    /// Minutes after midnight on the first day it starts, and on the last it ends (1440 is midnight at the end).
    pub start_min: u16,
    pub end_min: u16,
}

impl Event {
    /// Places an occurrence on the calendar of the zone `home`.
    #[must_use]
    pub fn place(occurrence: &Occurrence, source: u8, zones: &dyn Zones, home: &str) -> Self {
        let Occurrence {
            title,
            place,
            start,
            end,
            all_day,
            free,
        } = occurrence.clone();
        let (first_day, last_day, start_min, end_min);
        if all_day {
            first_day = start.div_euclid(SECS_PER_DAY);
            last_day = (end.div_euclid(SECS_PER_DAY) - 1).max(first_day);
            (start_min, end_min) = (0, 1_440);
        } else {
            let local = |at: i64| at + i64::from(zones.offset(home, at).unwrap_or(0));
            let (from, until) = (local(start), local(end));
            first_day = from.div_euclid(SECS_PER_DAY);
            start_min = minutes(from.rem_euclid(SECS_PER_DAY) / 60);
            let mut day = until.div_euclid(SECS_PER_DAY);
            let mut min = until.rem_euclid(SECS_PER_DAY) / 60;
            if end <= start {
                (day, min) = (first_day, i64::from(start_min));
            } else if min == 0 && day > first_day {
                // Ending at midnight sharp belongs to the day before.
                (day, min) = (day - 1, 1_440);
            }
            last_day = day.max(first_day);
            end_min = minutes(min);
        }
        Self {
            title,
            place,
            source,
            all_day,
            free,
            start,
            end,
            first_day,
            last_day: last_day.min(first_day + MAX_SPAN_DAYS),
            start_min,
            end_min,
        }
    }

    /// The minutes of `day` this event covers, as `(from, to)`.
    #[must_use]
    pub fn minutes_on(&self, day: i64) -> (u16, u16) {
        (
            if day == self.first_day {
                self.start_min
            } else {
                0
            },
            if day == self.last_day {
                self.end_min
            } else {
                1_440
            },
        )
    }

    /// Whether the event goes on over more than one day.
    #[must_use]
    pub fn spans_days(&self) -> bool {
        self.last_day > self.first_day
    }

    /// Whether it is under way at `now` (UTC seconds). A whole-day event is
    /// not "under way"; it just is.
    #[must_use]
    pub fn is_on_now(&self, now: i64) -> bool {
        !self.all_day && self.start <= now && now < self.end
    }
}

fn minutes(value: i64) -> u16 {
    u16::try_from(value.clamp(0, 1_440)).expect("BUG: clamped to a day")
}

/// All the events, found by the day they fall on.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    events: Vec<Event>,
    days: BTreeMap<i64, Vec<usize>>,
}

impl Schedule {
    #[must_use]
    pub fn new(mut events: Vec<Event>) -> Self {
        events.sort_by_key(|e| (e.first_day, !e.all_day, e.start_min));
        let mut days: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for (index, event) in events.iter().enumerate() {
            for day in event.first_day..=event.last_day {
                days.entry(day).or_default().push(index);
            }
        }
        // Within a day: whole-day events first, then those carried over from
        // yesterday, then by the time they start.
        for list in days.values_mut() {
            list.sort_by_key(|&i| {
                let e = &events[i];
                (!e.all_day, e.first_day, e.start_min, i)
            });
        }
        Self { events, days }
    }

    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The events that touch `day`, in the order they read.
    pub fn on(&self, day: i64) -> impl Iterator<Item = &Event> {
        self.days
            .get(&day)
            .into_iter()
            .flatten()
            .map(|&i| &self.events[i])
    }

    #[must_use]
    pub fn count_on(&self, day: i64) -> usize {
        self.days.get(&day).map_or(0, Vec::len)
    }

    /// The first day from `day` on that has an event.
    #[must_use]
    pub fn next_busy_day(&self, day: i64) -> Option<i64> {
        self.days.range(day..).next().map(|(&d, _)| d)
    }
}

/// The days of a month laid out in weeks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonthGrid {
    pub year: i64,
    pub month: u8,
    /// Day numbers, a row per week.
    pub weeks: Vec<[i64; 7]>,
}

impl MonthGrid {
    /// `first_weekday` is 0 for a week that starts on Monday up to 6 for Sunday.
    #[must_use]
    pub fn new(year: i64, month: u8, first_weekday: u8) -> Self {
        let first = civil::month_start(year, month);
        let lead = i64::from((civil::weekday(first) + 7 - first_weekday % 7) % 7);
        let rows = (lead + i64::from(civil::days_in_month(year, month)) + 6) / 7;
        let weeks = (0..rows)
            .map(|row| {
                std::array::from_fn(|col| first - lead + row * 7 + i64::try_from(col).unwrap_or(0))
            })
            .collect();
        Self { year, month, weeks }
    }

    /// Whether `day` is one of this month's own days, not a neighbour's.
    #[must_use]
    pub fn holds(&self, day: i64) -> bool {
        let date = Ymd::from_days(day);
        date.year == self.year && date.month == self.month
    }
}

/// A time of day: `09:30`, or `9:30 AM` for a 12-hour clock.
#[must_use]
pub fn clock(minute: u16, twelve_hour: bool) -> String {
    let (hour, min) = (minute / 60 % 24, minute % 60);
    if twelve_hour {
        let (shown, suffix) = match hour {
            0 => (12, "AM"),
            1..=11 => (hour, "AM"),
            12 => (12, "PM"),
            _ => (hour - 12, "PM"),
        };
        if min == 0 {
            format!("{shown} {suffix}")
        } else {
            format!("{shown}:{min:02} {suffix}")
        }
    } else {
        format!("{hour:02}:{min:02}")
    }
}

/// How a date reads: the order of day and month follows the Deck's date format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateStyle {
    pub month_first: bool,
}

impl DateStyle {
    /// `15 Oct` or `Oct 15`.
    #[must_use]
    pub fn day_month(self, day: i64) -> String {
        let date = Ymd::from_days(day);
        let month = civil::short_month(date.month);
        if self.month_first {
            format!("{month} {}", date.day)
        } else {
            format!("{} {month}", date.day)
        }
    }

    /// `Thu 15 Oct` or `Thu Oct 15`.
    #[must_use]
    pub fn short(self, day: i64) -> String {
        format!(
            "{} {}",
            civil::short_weekday(civil::weekday(day)),
            self.day_month(day)
        )
    }

    /// `Thursday 15 October` or `Thursday, October 15`.
    #[must_use]
    pub fn long(self, day: i64) -> String {
        let date = Ymd::from_days(day);
        let (weekday, month) = (
            civil::long_weekday(civil::weekday(day)),
            civil::long_month(date.month),
        );
        if self.month_first {
            format!("{weekday}, {month} {}", date.day)
        } else {
            format!("{weekday} {} {month}", date.day)
        }
    }

    /// What a day is called in a list: Today, Tomorrow, or its date.
    #[must_use]
    pub fn heading(self, day: i64, today: i64) -> String {
        match day - today {
            0 => "Today".to_owned(),
            1 => "Tomorrow".to_owned(),
            -1 => "Yesterday".to_owned(),
            _ => self.short(day),
        }
    }
}

/// When an event runs on `day`, as the words a list shows: the start (or
/// "All day"), and the end beneath it when there is one worth showing.
#[must_use]
pub fn when(event: &Event, day: i64, twelve_hour: bool) -> (String, Option<String>) {
    if event.all_day {
        return ("All day".to_owned(), None);
    }
    let (from, to) = event.minutes_on(day);
    let starts = day == event.first_day;
    let ends = day == event.last_day;
    match (starts, ends) {
        (true, true) if event.end > event.start => {
            (clock(from, twelve_hour), Some(clock(to, twelve_hour)))
        }
        (true, true) => (clock(from, twelve_hour), None),
        (true, false) => (clock(from, twelve_hour), Some("onwards".to_owned())),
        (false, true) => ("Until".to_owned(), Some(clock(to, twelve_hour))),
        (false, false) => ("All day".to_owned(), None),
    }
}

/// `text` cut to fit about `width` pixels at font size `size`, ending in an
/// ellipsis when cut. The renderer in firmware 0.6 doesn't cut text itself,
/// and glyph widths vary, so this is an estimate that errs short.
#[must_use]
pub fn fit(text: &str, width: f32, size: f32) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small positive count"
    )]
    let max = ((width / (size * 0.56)).floor().max(1.0)) as usize;
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let keep = max.saturating_sub(1).max(1);
    let mut cut: String = text.chars().take(keep).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

/// A calendar's occurrences as lines of text, to keep between runs; see [`decode`].
///
/// Written by hand: `format!` costs thousands of instructions a line, and a
/// calendar can have thousands of lines.
#[must_use]
pub fn encode(items: &[(u8, Occurrence)]) -> String {
    let mut out = String::with_capacity(items.len() * 72);
    for (source, o) in items {
        push_int(&mut out, o.start);
        out.push('|');
        push_int(&mut out, o.end);
        out.push('|');
        push_int(
            &mut out,
            i64::from(u8::from(o.all_day) | u8::from(o.free) << 1),
        );
        out.push('|');
        push_int(&mut out, i64::from(*source));
        out.push('|');
        push_clean(&mut out, &o.title);
        out.push('|');
        push_clean(&mut out, &o.place);
        out.push('\n');
    }
    out
}

fn push_int(out: &mut String, n: i64) {
    let mut digits = [0_u8; 20];
    let mut at = digits.len();
    let mut rest = n.unsigned_abs();
    loop {
        at -= 1;
        digits[at] = b'0' + u8::try_from(rest % 10).expect("BUG: a digit is below 10");
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    if n < 0 {
        out.push('-');
    }
    out.extend(digits[at..].iter().map(|&d| char::from(d)));
}

/// `text` with the characters the format uses to separate things turned into spaces.
fn push_clean(out: &mut String, text: &str) {
    if text.bytes().all(|b| b != b'|' && b != b'\n') {
        out.push_str(text);
    } else {
        out.extend(
            text.chars()
                .map(|c| if c == '|' || c == '\n' { ' ' } else { c }),
        );
    }
}

/// Reads what [`encode`] wrote. Lines that don't read are skipped.
#[must_use]
pub fn decode(text: &str) -> Vec<(u8, Occurrence)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(6, '|');
            let start = parts.next()?.parse().ok()?;
            let end = parts.next()?.parse().ok()?;
            let flags: u8 = parts.next()?.parse().ok()?;
            let source = parts.next()?.parse().ok()?;
            let occurrence = Occurrence {
                start,
                end,
                all_day: flags & 1 != 0,
                free: flags & 2 != 0,
                title: parts.next()?.to_owned(),
                place: parts.next().unwrap_or("").to_owned(),
            };
            Some((source, occurrence))
        })
        .collect()
}

/// Why a calendar couldn't be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The Deck refused to send the request: no link is saved for the calendar.
    Refused,
    /// The service answered with this HTTP status.
    Http(u32),
    /// The service couldn't be reached.
    Network,
    /// The link opens something that isn't a calendar file, such as a web page.
    NotCalendar,
    /// The calendar file is bigger than the Deck will download.
    TooLarge,
}

impl Fault {
    /// What went wrong, in a sentence for the screen.
    #[must_use]
    pub fn headline(self) -> String {
        match self {
            Self::Refused => "No link is saved for this calendar yet.".to_owned(),
            Self::Http(401 | 403) => "The service refused the link.".to_owned(),
            Self::Http(404 | 410) => "The link doesn't exist any more.".to_owned(),
            Self::Http(status) => format!("The service answered with an error ({status})."),
            Self::Network => "The service couldn't be reached.".to_owned(),
            Self::NotCalendar => "The link opens a web page, not a calendar.".to_owned(),
            Self::TooLarge => "The calendar file is too big to load.".to_owned(),
        }
    }

    /// Whether copying the link again could fix it, rather than the network.
    #[must_use]
    pub fn is_the_link(self) -> bool {
        matches!(self, Self::Http(401 | 403 | 404 | 410) | Self::NotCalendar)
    }
}

/// How one calendar is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceState {
    /// Asked for, no answer yet.
    Waiting,
    /// Read at this time (UTC seconds).
    Ready {
        at: i64,
    },
    Failed(Fault),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::testing::HostLike;

    const ROME: &str = "Europe/Rome";

    fn day(year: i64, month: u8, day: u8) -> i64 {
        Ymd { year, month, day }.days()
    }

    fn at(year: i64, month: u8, d: u8, hour: i64, minute: i64) -> i64 {
        day(year, month, d) * SECS_PER_DAY + hour * 3_600 + minute * 60
    }

    fn timed(title: &str, start: i64, end: i64) -> Event {
        Event::place(
            &Occurrence {
                title: title.into(),
                place: String::new(),
                start,
                end,
                all_day: false,
                free: false,
            },
            0,
            &HostLike,
            ROME,
        )
    }

    fn whole_day(title: &str, first: i64, after_last: i64) -> Event {
        Event::place(
            &Occurrence {
                title: title.into(),
                place: String::new(),
                start: first * SECS_PER_DAY,
                end: after_last * SECS_PER_DAY,
                all_day: true,
                free: false,
            },
            0,
            &HostLike,
            ROME,
        )
    }

    #[test]
    fn a_timed_event_lands_on_the_decks_local_day_and_time() {
        // 22:30 UTC in October is 00:30 the next day in Rome.
        let e = timed("Late", at(2026, 10, 14, 22, 30), at(2026, 10, 14, 23, 30));
        assert_eq!(e.first_day, day(2026, 10, 15));
        assert_eq!((e.start_min, e.end_min), (30, 90));
        assert_eq!(e.last_day, e.first_day);
    }

    #[test]
    fn an_event_that_ends_at_midnight_stays_on_its_day() {
        let e = timed("Evening", at(2026, 10, 15, 18, 0), at(2026, 10, 15, 22, 0));
        assert_eq!(e.last_day, e.first_day);
        assert_eq!(e.end_min, 1_440);
    }

    #[test]
    fn an_event_over_midnight_covers_both_days() {
        let e = timed(
            "Party",
            at(2026, 10, 15, 20, 0),
            at(2026, 10, 15, 23, 0) + 3 * 3_600,
        );
        assert_eq!(e.last_day, e.first_day + 1);
        assert_eq!(e.minutes_on(e.first_day), (22 * 60, 1_440));
        assert_eq!(e.minutes_on(e.last_day), (0, 4 * 60));
        assert!(e.spans_days());
    }

    #[test]
    fn whole_day_events_cover_their_days_and_not_the_end_date() {
        let e = whole_day("Trip", day(2026, 10, 17), day(2026, 10, 20));
        assert_eq!(
            (e.first_day, e.last_day),
            (day(2026, 10, 17), day(2026, 10, 19))
        );
        let one = whole_day("Holiday", day(2026, 10, 20), day(2026, 10, 21));
        assert_eq!(
            (one.first_day, one.last_day),
            (day(2026, 10, 20), day(2026, 10, 20))
        );
    }

    #[test]
    fn a_days_list_reads_whole_day_events_first_and_then_by_time() {
        let events = vec![
            timed("Lunch", at(2026, 10, 15, 11, 0), at(2026, 10, 15, 12, 0)),
            whole_day("Holiday", day(2026, 10, 15), day(2026, 10, 16)),
            timed("Standup", at(2026, 10, 15, 7, 0), at(2026, 10, 15, 7, 15)),
            timed(
                "Night shift",
                at(2026, 10, 14, 20, 0),
                at(2026, 10, 15, 3, 0),
            ),
        ];
        let schedule = Schedule::new(events);
        let titles: Vec<_> = schedule
            .on(day(2026, 10, 15))
            .map(|e| e.title.as_str())
            .collect();
        assert_eq!(titles, ["Holiday", "Night shift", "Standup", "Lunch"]);
        assert_eq!(schedule.count_on(day(2026, 10, 14)), 1);
        assert_eq!(schedule.count_on(day(2026, 10, 16)), 0);
        assert_eq!(schedule.next_busy_day(day(2026, 10, 16)), None);
        assert_eq!(
            schedule.next_busy_day(day(2026, 10, 1)),
            Some(day(2026, 10, 14))
        );
    }

    #[test]
    fn a_month_grid_starts_on_the_chosen_weekday() {
        // October 2026 begins on a Thursday.
        let monday = MonthGrid::new(2026, 10, 0);
        assert_eq!(monday.weeks.len(), 5);
        assert_eq!(monday.weeks[0][0], day(2026, 9, 28));
        assert_eq!(monday.weeks[0][3], day(2026, 10, 1));
        assert_eq!(monday.weeks[4][6], day(2026, 11, 1));
        let sunday = MonthGrid::new(2026, 10, 6);
        assert_eq!(sunday.weeks[0][0], day(2026, 9, 27));
        assert!(monday.holds(day(2026, 10, 31)) && !monday.holds(day(2026, 11, 1)));
    }

    #[test]
    fn a_month_that_needs_six_weeks_gets_six() {
        // March 2026 starts on a Sunday and has 31 days.
        assert_eq!(MonthGrid::new(2026, 3, 0).weeks.len(), 6);
        // February 2027 starts on a Monday and has 28 days.
        assert_eq!(MonthGrid::new(2027, 2, 0).weeks.len(), 4);
    }

    #[test]
    fn times_read_in_either_clock() {
        assert_eq!(clock(9 * 60 + 30, false), "09:30");
        assert_eq!(clock(0, false), "00:00");
        assert_eq!(clock(9 * 60 + 30, true), "9:30 AM");
        assert_eq!(clock(12 * 60, true), "12 PM");
        assert_eq!(clock(0, true), "12 AM");
        assert_eq!(clock(13 * 60 + 5, true), "1:05 PM");
        assert_eq!(clock(1_440, false), "00:00");
    }

    #[test]
    fn dates_follow_the_date_style() {
        let d = day(2026, 10, 15);
        let day_first = DateStyle { month_first: false };
        let month_first = DateStyle { month_first: true };
        assert_eq!(day_first.short(d), "Thu 15 Oct");
        assert_eq!(month_first.short(d), "Thu Oct 15");
        assert_eq!(day_first.long(d), "Thursday 15 October");
        assert_eq!(month_first.long(d), "Thursday, October 15");
        assert_eq!(day_first.heading(d, d), "Today");
        assert_eq!(day_first.heading(d + 1, d), "Tomorrow");
        assert_eq!(day_first.heading(d + 2, d), "Sat 17 Oct");
    }

    #[test]
    fn a_list_says_when_each_event_runs() {
        let d = day(2026, 10, 15);
        let meeting = timed("Meeting", at(2026, 10, 15, 8, 0), at(2026, 10, 15, 9, 0));
        assert_eq!(
            when(&meeting, d, false),
            ("10:00".into(), Some("11:00".into()))
        );
        let reminder = timed("Reminder", at(2026, 10, 15, 8, 0), at(2026, 10, 15, 8, 0));
        assert_eq!(when(&reminder, d, false), ("10:00".into(), None));
        let trip = whole_day("Trip", d, d + 2);
        assert_eq!(when(&trip, d, false), ("All day".into(), None));
        let party = timed("Party", at(2026, 10, 15, 20, 0), at(2026, 10, 16, 3, 0));
        assert_eq!(
            when(&party, d, false),
            ("22:00".into(), Some("onwards".into()))
        );
        assert_eq!(
            when(&party, d + 1, false),
            ("Until".into(), Some("05:00".into()))
        );
    }

    #[test]
    fn long_titles_are_cut_with_an_ellipsis() {
        assert_eq!(fit("Short", 200.0, 16.0), "Short");
        let cut = fit("A very long meeting title that cannot fit", 100.0, 16.0);
        assert!(cut.ends_with('…') && cut.chars().count() <= 12, "{cut}");
        assert_eq!(fit("", 100.0, 16.0), "");
    }

    #[test]
    fn occurrences_survive_being_saved_and_read_back() {
        let items = vec![
            (
                0,
                Occurrence {
                    title: "Standup | daily".into(),
                    place: "Room".into(),
                    start: at(2026, 10, 15, 7, 0),
                    end: at(2026, 10, 15, 7, 15),
                    all_day: false,
                    free: true,
                },
            ),
            (
                2,
                Occurrence {
                    title: "Holiday".into(),
                    place: String::new(),
                    start: day(2026, 10, 16) * SECS_PER_DAY,
                    end: day(2026, 10, 17) * SECS_PER_DAY,
                    all_day: true,
                    free: false,
                },
            ),
        ];
        let back = decode(&encode(&items));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].0, 0);
        assert_eq!(back[0].1.title, "Standup   daily");
        assert_eq!(back[0].1.place, "Room");
        assert!(back[0].1.free && !back[0].1.all_day);
        assert_eq!(back[1].0, 2);
        assert!(back[1].1.all_day);
        assert_eq!(back[1].1.end, items[1].1.end);
        assert!(decode("garbage\n1|2\n").is_empty());
    }
}
