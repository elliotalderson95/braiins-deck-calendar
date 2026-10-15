//! Turns the events a calendar file holds into the occurrences that fall in a
//! window of time: places each in time (applying its timezone), repeats the
//! repeating ones with the host's recurrence engine, leaves out the
//! occurrences a calendar cancelled or replaced, and keeps those that touch
//! the window.

use std::collections::HashSet;

use crate::civil::{SECS_PER_DAY, Ymd};
use crate::ics::{IcsTime, RawEvent, Zone};
use crate::zones::{Zones, iana_name, local_to_utc};

/// The most occurrences one repeating event gives in a window.
const MAX_REPEATS: u16 = 300;

/// The stretch of time that matters, in UTC seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub lo: i64,
    pub hi: i64,
}

impl Window {
    /// Whether something from `start` to `end` touches the window. Something
    /// with no length counts when its moment is inside.
    #[must_use]
    pub fn touches(self, start: i64, end: i64) -> bool {
        if end > start {
            end > self.lo && start < self.hi
        } else {
            start >= self.lo && start < self.hi
        }
    }
}

/// One event on one occasion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub title: String,
    pub place: String,
    /// UTC seconds. A whole-day event starts at UTC midnight of its first
    /// day and ends at UTC midnight after its last, whatever the zone.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub free: bool,
}

/// What it takes to place times: the host, and the zone for times that name none.
pub struct Cx<'a> {
    pub zones: &'a dyn Zones,
    /// The Deck's own timezone.
    pub home: &'a str,
}

impl Cx<'_> {
    /// The zone the host should apply to a time of day, `None` for UTC.
    fn host_zone(&self, zone: &Zone) -> Option<String> {
        match zone {
            Zone::Utc => None,
            Zone::Floating => Some(self.home.to_owned()),
            Zone::Named(name) => {
                let name = iana_name(name);
                if !name.is_empty() && self.zones.offset(&name, 0).is_some() {
                    Some(name.into_owned())
                } else {
                    Some(self.home.to_owned())
                }
            }
        }
    }

    /// The instant a calendar time means, in UTC seconds.
    fn utc_of(&self, time: &IcsTime) -> i64 {
        match time {
            IcsTime::Date(day) => day * SECS_PER_DAY,
            IcsTime::DateTime { local_secs, zone } => match self.host_zone(zone) {
                None => *local_secs,
                Some(tz) => local_to_utc(self.zones, &tz, *local_secs).unwrap_or(*local_secs),
            },
        }
    }
}

/// Which occurrences of repeating events other events replace (or cancel):
/// the event's UID and the instant of the occurrence.
#[must_use]
pub fn replaced_occurrences(events: &[RawEvent], cx: &Cx<'_>) -> HashSet<(String, i64)> {
    events
        .iter()
        .filter_map(|e| Some((e.uid.clone(), cx.utc_of(e.recurrence_id.as_ref()?))))
        .collect()
}

/// Adds to `out` every occurrence of `event` that touches `window`, leaving
/// out those in `replaced`.
#[expect(
    clippy::implicit_hasher,
    reason = "the set is only ever the default one"
)]
pub fn occurrences_of(
    event: &RawEvent,
    replaced: &HashSet<(String, i64)>,
    cx: &Cx<'_>,
    window: Window,
    out: &mut Vec<Occurrence>,
) {
    if event.cancelled {
        return;
    }
    let repeats = event.rrule.is_some() && event.recurrence_id.is_none();
    for (start, end) in event_times(event, cx, window) {
        if repeats && replaced.contains(&(event.uid.clone(), start)) {
            continue;
        }
        out.push(Occurrence {
            title: if event.title.is_empty() {
                "(No title)".to_owned()
            } else {
                event.title.clone()
            },
            place: event.place.clone(),
            start,
            end,
            all_day: matches!(event.start, IcsTime::Date(_)),
            free: event.free,
        });
    }
}

/// Every occurrence of `events` that touches `window`, in no particular order.
#[must_use]
pub fn occurrences(events: &[RawEvent], cx: &Cx<'_>, window: Window) -> Vec<Occurrence> {
    let replaced = replaced_occurrences(events, cx);
    let mut out = Vec::new();
    for event in events {
        occurrences_of(event, &replaced, cx, window, &mut out);
    }
    out
}

/// The `(start, end)` of each time `event` happens within the window.
fn event_times(event: &RawEvent, cx: &Cx<'_>, window: Window) -> Vec<(i64, i64)> {
    let all_day = matches!(event.start, IcsTime::Date(_));
    let first = cx.utc_of(&event.start);
    let length = length_of(event, first, all_day, cx);

    let starts = if event.rrule.is_some() && event.recurrence_id.is_none() {
        repeats(event, cx, window, length)
    } else {
        vec![first]
    };
    let skipped: HashSet<i64> = event.exdates.iter().map(|at| cx.utc_of(at)).collect();
    starts
        .into_iter()
        .filter(|start| !skipped.contains(start))
        .map(|start| (start, start + length))
        .filter(|&(start, end)| window.touches(start, end))
        .collect()
}

/// How long the event lasts, in seconds: to its end, else its duration, else
/// a whole day for a whole-day event and no time at all for the rest.
fn length_of(event: &RawEvent, start: i64, all_day: bool, cx: &Cx<'_>) -> i64 {
    let length = match (&event.end, event.duration) {
        (Some(end), _) => cx.utc_of(end) - start,
        (None, Some(secs)) => secs,
        (None, None) if all_day => SECS_PER_DAY,
        (None, None) => 0,
    };
    if length < 0 || (all_day && length == 0) {
        if all_day { SECS_PER_DAY } else { 0 }
    } else {
        length
    }
}

/// The starts of a repeating event that fall near the window, asked of the host.
fn repeats(event: &RawEvent, cx: &Cx<'_>, window: Window, length: i64) -> Vec<i64> {
    let Some(rule) = &event.rrule else {
        return Vec::new();
    };
    let (dtstart, tzid) = match &event.start {
        IcsTime::Date(day) => (stamp(day * SECS_PER_DAY), None),
        IcsTime::DateTime { local_secs, zone } => match cx.host_zone(zone) {
            None => (format!("{}Z", stamp(*local_secs)), None),
            Some(tz) => (stamp(*local_secs), Some(tz)),
        },
    };
    let rule = settle_until(rule, &event.start, cx);
    // An occurrence that began before the window can still be going on in it.
    cx.zones.expand(
        &rule,
        &dtstart,
        tzid.as_deref(),
        window.lo - length - SECS_PER_DAY,
        window.hi,
        MAX_REPEATS,
    )
}

/// `20261015T093000` for the wall-clock time counted in `secs`.
fn stamp(secs: i64) -> String {
    let date = Ymd::from_days(secs.div_euclid(SECS_PER_DAY));
    let of_day = secs.rem_euclid(SECS_PER_DAY);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        date.year,
        date.month,
        date.day,
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60
    )
}

/// Rewrites a rule's `UNTIL` into the form the host's recurrence engine
/// wants, a UTC time, whatever form the calendar wrote (a bare date, or a
/// time in the event's own zone). A rule with a `COUNT` loses its `UNTIL`
/// instead, since a rule may carry only one of them.
fn settle_until(rule: &str, start: &IcsTime, cx: &Cx<'_>) -> String {
    let counted = rule.split(';').any(|part| part.starts_with("COUNT="));
    rule.split(';')
        .filter_map(|part| {
            let Some(until) = part.strip_prefix("UNTIL=") else {
                return Some(part.to_owned());
            };
            if counted {
                return None;
            }
            let Some(local) = until_secs(until) else {
                return Some(part.to_owned());
            };
            let utc = match (until.ends_with('Z'), start) {
                (true, _) | (false, IcsTime::Date(_)) => local,
                (false, IcsTime::DateTime { zone, .. }) => match cx.host_zone(zone) {
                    None => local,
                    Some(tz) => local_to_utc(cx.zones, &tz, local).unwrap_or(local),
                },
            };
            Some(format!("UNTIL={}Z", stamp(utc)))
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// The wall-clock seconds an `UNTIL` value names; a bare date means the end
/// of that day.
fn until_secs(value: &str) -> Option<i64> {
    let value = value.trim_end_matches('Z');
    let date = Ymd {
        year: value.get(0..4)?.parse().ok()?,
        month: value.get(4..6)?.parse().ok()?,
        day: value.get(6..8)?.parse().ok()?,
    };
    if !date.is_valid() {
        return None;
    }
    let day_start = date.days() * SECS_PER_DAY;
    match value.get(8..) {
        Some("") | None => Some(day_start + SECS_PER_DAY - 1),
        Some(time) => {
            let time = time.strip_prefix('T')?;
            let (h, m, s): (i64, i64, i64) = (
                time.get(0..2)?.parse().ok()?,
                time.get(2..4)?.parse().ok()?,
                time.get(4..6).unwrap_or("00").parse().ok()?,
            );
            Some(day_start + h * 3_600 + m * 60 + s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ics::{Keep, Scanner, Step};
    use crate::zones::testing::HostLike;

    const ROME: &str = "Europe/Rome";

    fn at(year: i64, month: u8, day: u8, hour: i64, minute: i64) -> i64 {
        Ymd { year, month, day }.days() * SECS_PER_DAY + hour * 3_600 + minute * 60
    }

    fn window(from: (i64, u8, u8), to: (i64, u8, u8)) -> Window {
        Window {
            lo: at(from.0, from.1, from.2, 0, 0),
            hi: at(to.0, to.1, to.2, 0, 0),
        }
    }

    fn read(ics: &str) -> Vec<RawEvent> {
        let mut scanner = Scanner::new();
        let keep = Keep {
            from_day: i64::MIN,
            to_day: i64::MAX,
        };
        assert_eq!(scanner.step(ics.as_bytes(), usize::MAX, keep), Step::Done);
        scanner.events
    }

    fn expand(ics: &str, window: Window) -> Vec<Occurrence> {
        let events = read(ics);
        let cx = Cx {
            zones: &HostLike,
            home: ROME,
        };
        let mut out = occurrences(&events, &cx, window);
        out.sort_by_key(|o| (o.start, o.title.clone()));
        out
    }

    fn october() -> Window {
        window((2026, 10, 1), (2026, 11, 1))
    }

    #[test]
    fn a_utc_event_keeps_its_instant() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20261015T093000Z\nDTEND:20261015T103000Z\nSUMMARY:Call\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            (out[0].start, out[0].end),
            (at(2026, 10, 15, 9, 30), at(2026, 10, 15, 10, 30))
        );
        assert!(!out[0].all_day);
    }

    #[test]
    fn a_zoned_event_is_placed_by_its_zone_not_the_decks() {
        // 09:00 in New York in October is four hours behind UTC.
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;TZID=America/New_York:20261015T090000\nDTEND;TZID=America/New_York:20261015T100000\nSUMMARY:NY\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out[0].start, at(2026, 10, 15, 13, 0));
        assert_eq!(out[0].end, at(2026, 10, 15, 14, 0));
    }

    #[test]
    fn an_outlook_style_windows_zone_name_is_understood() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;TZID=W. Europe Standard Time:20261015T090000\nDURATION:PT30M\nSUMMARY:Outlook\nEND:VEVENT\n",
            october(),
        );
        // Berlin, like Rome, is two hours ahead of UTC in October.
        assert_eq!(
            (out[0].start, out[0].end),
            (at(2026, 10, 15, 7, 0), at(2026, 10, 15, 7, 30))
        );
    }

    #[test]
    fn a_floating_time_or_an_unknown_zone_is_read_in_the_decks_zone() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20261015T090000\nSUMMARY:Floating\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;TZID=Customized Time Zone:20261016T090000\nSUMMARY:Odd\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out[0].start, at(2026, 10, 15, 7, 0));
        assert_eq!(out[1].start, at(2026, 10, 16, 7, 0));
    }

    #[test]
    fn a_whole_day_event_covers_its_days_and_the_end_date_is_not_included() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261017\nDTEND;VALUE=DATE:20261019\nSUMMARY:Trip\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261020\nSUMMARY:Holiday\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(
            (out[0].start, out[0].end),
            (at(2026, 10, 17, 0, 0), at(2026, 10, 19, 0, 0))
        );
        assert!(out[0].all_day);
        assert_eq!(out[1].end - out[1].start, SECS_PER_DAY);
    }

    #[test]
    fn events_outside_the_window_are_left_out_and_ones_running_into_it_stay() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20260901T090000Z\nDTEND:20260901T100000Z\nSUMMARY:Before\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20260930T220000Z\nDTEND:20261001T020000Z\nSUMMARY:Overnight\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20261201T090000Z\nSUMMARY:After\nEND:VEVENT\n",
            october(),
        );
        let titles: Vec<_> = out.iter().map(|o| o.title.as_str()).collect();
        assert_eq!(titles, ["Overnight"]);
    }

    #[test]
    fn a_weekly_event_stays_at_the_same_local_time_across_the_clock_change() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Rome:20260903T090000\nDTEND;TZID=Europe/Rome:20260903T100000\n\
             RRULE:FREQ=WEEKLY;BYDAY=TH\nSUMMARY:Weekly\nEND:VEVENT\n",
            window((2026, 10, 20), (2026, 11, 3)),
        );
        assert_eq!(
            out.iter().map(|o| o.start).collect::<Vec<_>>(),
            [at(2026, 10, 22, 7, 0), at(2026, 10, 29, 8, 0)]
        );
        assert!(out.iter().all(|o| o.end - o.start == 3_600));
    }

    #[test]
    fn exceptions_remove_single_occurrences() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Rome:20261001T090000\nRRULE:FREQ=WEEKLY;BYDAY=TH\n\
             EXDATE;TZID=Europe/Rome:20261008T090000,20261022T090000\nSUMMARY:Weekly\nEND:VEVENT\n",
            october(),
        );
        let days: Vec<_> = out
            .iter()
            .map(|o| Ymd::from_days(o.start.div_euclid(SECS_PER_DAY)).day)
            .collect();
        assert_eq!(days, [1, 15, 29]);
    }

    #[test]
    fn a_moved_occurrence_replaces_the_original() {
        let out = expand(
            "BEGIN:VEVENT\nUID:w\nDTSTART;TZID=Europe/Rome:20261001T090000\nDTEND;TZID=Europe/Rome:20261001T100000\n\
             RRULE:FREQ=WEEKLY;BYDAY=TH\nSUMMARY:Weekly\nEND:VEVENT\n\
             BEGIN:VEVENT\nUID:w\nRECURRENCE-ID;TZID=Europe/Rome:20261015T090000\nDTSTART;TZID=Europe/Rome:20261015T150000\n\
             DTEND;TZID=Europe/Rome:20261015T160000\nSUMMARY:Weekly (moved)\nEND:VEVENT\n",
            october(),
        );
        let seen: Vec<_> = out.iter().map(|o| (o.title.as_str(), o.start)).collect();
        assert_eq!(
            seen,
            [
                ("Weekly", at(2026, 10, 1, 7, 0)),
                ("Weekly", at(2026, 10, 8, 7, 0)),
                ("Weekly (moved)", at(2026, 10, 15, 13, 0)),
                ("Weekly", at(2026, 10, 22, 7, 0)),
                ("Weekly", at(2026, 10, 29, 8, 0)),
            ]
        );
    }

    #[test]
    fn a_cancelled_occurrence_disappears() {
        let out = expand(
            "BEGIN:VEVENT\nUID:w\nDTSTART:20261001T090000Z\nDTEND:20261001T100000Z\nRRULE:FREQ=WEEKLY;COUNT=4\nSUMMARY:Weekly\nEND:VEVENT\n\
             BEGIN:VEVENT\nUID:w\nRECURRENCE-ID:20261008T090000Z\nDTSTART:20261008T090000Z\nSTATUS:CANCELLED\nSUMMARY:Weekly\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|o| o.start != at(2026, 10, 8, 9, 0)));
    }

    #[test]
    fn a_yearly_whole_day_event_such_as_a_birthday_lands_on_its_date() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:19900312\nRRULE:FREQ=YEARLY\nSUMMARY:Birthday\nEND:VEVENT\n",
            window((2027, 3, 1), (2027, 4, 1)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, at(2027, 3, 12, 0, 0));
        assert!(out[0].all_day);
    }

    #[test]
    fn rules_ending_in_a_date_or_a_local_time_still_end() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261001\nRRULE:FREQ=DAILY;UNTIL=20261005\nSUMMARY:Date until\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;TZID=Europe/Rome:20261001T090000\nRRULE:FREQ=DAILY;UNTIL=20261003T090000\nSUMMARY:Local until\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;TZID=Europe/Rome:20261001T090000\nRRULE:FREQ=DAILY;UNTIL=20261006\nSUMMARY:Date until, timed\nEND:VEVENT\n",
            october(),
        );
        let count = |title: &str| out.iter().filter(|o| o.title == title).count();
        assert_eq!(count("Date until"), 5);
        assert_eq!(count("Local until"), 3);
        assert_eq!(count("Date until, timed"), 6);
    }

    #[test]
    fn a_rule_with_a_count_and_an_until_still_works() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20261001T090000Z\nRRULE:FREQ=DAILY;COUNT=3;UNTIL=20261231T000000Z\nSUMMARY:Both\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn an_event_that_started_before_the_window_and_repeats_into_it_is_found() {
        // A two-day event repeating weekly, whose Wednesday run started
        // the day before the window opens.
        let out = expand(
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:20260930\nDTEND;VALUE=DATE:20261002\nRRULE:FREQ=WEEKLY;COUNT=2\nSUMMARY:Retreat\nEND:VEVENT\n",
            window((2026, 10, 1), (2026, 10, 3)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, at(2026, 9, 30, 0, 0));
    }

    /// The UTC starts of the occurrences of a repeating event in October 2026.
    fn october_starts(start_line: &str, rule: &str) -> Vec<i64> {
        let ics =
            format!("BEGIN:VEVENT\nUID:x\n{start_line}\nRRULE:{rule}\nSUMMARY:Rule\nEND:VEVENT\n");
        expand(&ics, october()).iter().map(|o| o.start).collect()
    }

    #[test]
    fn the_kinds_of_rule_calendars_write_all_work() {
        let rome = "DTSTART;TZID=Europe/Rome:";
        // The second Tuesday of each month.
        assert_eq!(
            october_starts(&format!("{rome}20260113T100000"), "FREQ=MONTHLY;BYDAY=2TU"),
            [at(2026, 10, 13, 8, 0)]
        );
        // The last day of each month.
        assert_eq!(
            october_starts(
                &format!("{rome}20260131T180000"),
                "FREQ=MONTHLY;BYMONTHDAY=-1"
            ),
            [at(2026, 10, 31, 17, 0)]
        );
        // Every other week on Monday and Wednesday, weeks starting on Sunday.
        assert_eq!(
            october_starts(
                &format!("{rome}20261005T090000"),
                "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;WKST=SU"
            ),
            [
                at(2026, 10, 5, 7, 0),
                at(2026, 10, 7, 7, 0),
                at(2026, 10, 19, 7, 0),
                at(2026, 10, 21, 7, 0)
            ]
        );
        // Outlook's spelling, with every optional part present.
        assert_eq!(
            october_starts(
                &format!("{rome}20260901T093000"),
                "FREQ=WEEKLY;COUNT=10;INTERVAL=1;BYDAY=TU;WKST=MO"
            )
            .len(),
            4
        );
        // Google's spelling: a zoned start with an end in UTC.
        assert_eq!(
            october_starts(
                &format!("{rome}20260902T090000"),
                "FREQ=WEEKLY;UNTIL=20261014T215959Z;BYDAY=WE"
            ),
            [at(2026, 10, 7, 7, 0), at(2026, 10, 14, 7, 0)]
        );
        // A whole-day run of three days that crosses out of the window.
        assert_eq!(
            october_starts("DTSTART;VALUE=DATE:20261030", "FREQ=DAILY;COUNT=3"),
            [at(2026, 10, 30, 0, 0), at(2026, 10, 31, 0, 0)]
        );
    }

    /// What Google really sends: Italy's public holidays, cut down to eight events
    /// (LF line endings, whole days, UTF-8, folded descriptions with escapes).
    const GOOGLE_HOLIDAYS: &str = include_str!("../testdata/google-holidays-it.ics");

    #[test]
    fn a_real_google_feed_reads_as_it_should() {
        let mut scanner = Scanner::new();
        let keep = Keep {
            from_day: i64::MIN,
            to_day: i64::MAX,
        };
        assert_eq!(
            scanner.step(GOOGLE_HOLIDAYS.as_bytes(), usize::MAX, keep),
            Step::Done
        );
        assert_eq!(scanner.name.as_deref(), Some("Festivit\u{e0} in Italia"));
        assert_eq!(scanner.seen, 8);

        let in_october = expand(GOOGLE_HOLIDAYS, october());
        assert_eq!(in_october.len(), 1);
        let first = &in_october[0];
        assert_eq!(first.title, "San Francesco d'Assisi");
        assert_eq!(
            (first.start, first.all_day, first.free),
            (at(2026, 10, 4, 0, 0), true, true)
        );

        let at_the_end_of_the_year = window((2026, 12, 1), (2027, 1, 15));
        let titles: Vec<String> = expand(GOOGLE_HOLIDAYS, at_the_end_of_the_year)
            .into_iter()
            .map(|o| o.title)
            .collect();
        assert_eq!(
            titles,
            [
                "Immacolata",
                "Natale",
                "Santo Stefano",
                "San Silvestro",
                "Capodanno",
                "Epifania"
            ]
        );
    }

    /// For finding out why a real feed misbehaves: reads the file named by
    /// `ICS_FILE` and prints what the widget would show from `TODAY`
    /// (`YYYY-MM-DD`, default 2026-10-15), with timings.
    ///
    /// `ICS_FILE=feed.ics cargo test -p deck-calendar a_feed_from_a_file -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a feed file named by ICS_FILE"]
    fn a_feed_from_a_file() {
        let Ok(path) = std::env::var("ICS_FILE") else {
            return;
        };
        let data = std::fs::read(&path).expect("BUG: ICS_FILE must be readable");
        let today = std::env::var("TODAY").unwrap_or_else(|_| "2026-10-15".to_owned());
        let part = |from: usize, to: usize| {
            today[from..to]
                .parse::<i64>()
                .expect("BUG: TODAY is YYYY-MM-DD")
        };
        let (year, month, day) = (
            part(0, 4),
            u8::try_from(part(5, 7)).expect("month"),
            u8::try_from(part(8, 10)).expect("day"),
        );
        let today_day = Ymd { year, month, day }.days();
        let (from_year, from_month) = crate::civil::add_months(year, month, -1);
        let (to_year, to_month) = crate::civil::add_months(year, month, 2);
        let (lo_day, hi_day) = (
            crate::civil::month_start(from_year, from_month) - 7,
            crate::civil::month_start(to_year, to_month) + 7,
        );
        let start = std::time::Instant::now();
        let mut scanner = Scanner::new();
        let keep = Keep {
            from_day: lo_day - 2,
            to_day: hi_day + 2,
        };
        let mut steps = 0;
        while scanner.step(&data, 6_000, keep) == Step::More {
            steps += 1;
        }
        println!(
            "{} bytes, {} events, {} kept, {} frames of 6000 units, calendar name {:?}, {:?}",
            data.len(),
            scanner.seen,
            scanner.events.len(),
            steps + 1,
            scanner.name,
            start.elapsed()
        );
        let cx = Cx {
            zones: &HostLike,
            home: ROME,
        };
        let window = Window {
            lo: (lo_day - 1) * SECS_PER_DAY,
            hi: (hi_day + 1) * SECS_PER_DAY,
        };
        let mut found = occurrences(&scanner.events, &cx, window);
        found.sort_by_key(|o| o.start);
        println!("{} occurrences in the window; from today:", found.len());
        for o in found
            .iter()
            .filter(|o| o.end > today_day * SECS_PER_DAY)
            .take(25)
        {
            let local = o.start + 7_200;
            let day = Ymd::from_days(local.div_euclid(SECS_PER_DAY));
            let minute_of_day = local.rem_euclid(SECS_PER_DAY) / 60;
            let when = if o.all_day {
                "all day".to_owned()
            } else {
                format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60)
            };
            println!(
                "  {:04}-{:02}-{:02} {when:8} {}",
                day.year, day.month, day.day, o.title
            );
        }
    }

    #[test]
    fn an_event_with_no_end_has_no_length_and_no_title_gets_a_stand_in() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20261015T093000Z\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out[0].end, out[0].start);
        assert_eq!(out[0].title, "(No title)");
    }

    #[test]
    fn an_end_before_the_start_is_treated_as_no_length() {
        let out = expand(
            "BEGIN:VEVENT\nDTSTART:20261015T093000Z\nDTEND:20261015T083000Z\nSUMMARY:Odd\nEND:VEVENT\n",
            october(),
        );
        assert_eq!(out[0].end, out[0].start);
    }
}
