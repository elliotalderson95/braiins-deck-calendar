//! Timezones and recurrence rules, which the Deck's widget host works out
//! (it carries the timezone database and the RRULE engine, so the widget
//! stays small). The widget asks through [`Zones`]; native tests answer with
//! the same crates the host uses.

use std::borrow::Cow;

/// What the host can tell the widget about timezones and recurring events.
pub trait Zones {
    /// The UTC offset in seconds that the IANA zone `tz` has at the instant
    /// `utc` (seconds since the epoch), or `None` for a zone it doesn't know.
    fn offset(&self, tz: &str, utc: i64) -> Option<i32>;

    /// The starts, as UTC seconds, of the occurrences of an event that begins
    /// at `dtstart` (`YYYYMMDDTHHMMSS`, with a trailing `Z` for UTC) in the
    /// zone `tzid` (UTC when none) and repeats by `rrule`, between `lo` and
    /// `hi`, at most `max` of them. A rule the host can't read gives none.
    fn expand(
        &self,
        rrule: &str,
        dtstart: &str,
        tzid: Option<&str>,
        lo: i64,
        hi: i64,
        max: u16,
    ) -> Vec<i64>;
}

/// The UTC instant at which the wall clocks of zone `tz` read `local_secs`
/// (the wall-clock time counted in seconds since 1970 as if it were UTC).
///
/// A wall-clock time that happens twice (the clocks go back) means its first
/// time, and one that never happens (the clocks go forward) is read with the
/// offset from before the jump, as RFC 5545 says. `None` for an unknown zone.
#[must_use]
pub fn local_to_utc(zones: &dyn Zones, tz: &str, local_secs: i64) -> Option<i64> {
    let before = zones.offset(tz, local_secs - 86_400)?;
    let after = zones.offset(tz, local_secs + 86_400)?;
    let fits = |offset: i32| zones.offset(tz, local_secs - i64::from(offset)) == Some(offset);
    let offset = match (fits(before), fits(after)) {
        (true, true) => before.max(after),
        (true | false, false) => before,
        (false, true) => after,
    };
    Some(local_secs - i64::from(offset))
}

/// The name the host's timezone database knows a calendar's `TZID` by.
///
/// Calendars name zones in three ways: IANA names (`Europe/Rome`), which pass
/// through, the same behind a library's path (`/freeassociation.sourceforge.net/Tzfile/Europe/Rome`),
/// and Windows names (`W. Europe Standard Time`), which Outlook uses.
#[must_use]
pub fn iana_name(tzid: &str) -> Cow<'_, str> {
    let tzid = tzid.trim().trim_matches('"');
    if let Some((_, rest)) = tzid.split_once("/Tzfile/") {
        return Cow::Borrowed(rest);
    }
    if tzid.starts_with("tzone://Microsoft/") {
        return Cow::Borrowed(if tzid.ends_with("/Utc") { "UTC" } else { "" });
    }
    match WINDOWS_ZONES
        .iter()
        .find(|(windows, _)| windows.eq_ignore_ascii_case(tzid))
    {
        Some((_, iana)) => Cow::Borrowed(iana),
        None => Cow::Borrowed(tzid),
    }
}

/// Windows zone names with the IANA zone Microsoft pairs each with (the
/// Unicode CLDR `windowsZones` table, one zone per name).
const WINDOWS_ZONES: &[(&str, &str)] = &[
    ("Dateline Standard Time", "Etc/GMT+12"),
    ("UTC-11", "Etc/GMT+11"),
    ("Aleutian Standard Time", "America/Adak"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Marquesas Standard Time", "Pacific/Marquesas"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("UTC-09", "Etc/GMT+9"),
    ("Pacific Standard Time (Mexico)", "America/Tijuana"),
    ("UTC-08", "Etc/GMT+8"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Mountain Standard Time (Mexico)", "America/Mazatlan"),
    ("Mountain Standard Time", "America/Denver"),
    ("Yukon Standard Time", "America/Whitehorse"),
    ("Central America Standard Time", "America/Guatemala"),
    ("Central Standard Time", "America/Chicago"),
    ("Easter Island Standard Time", "Pacific/Easter"),
    ("Central Standard Time (Mexico)", "America/Mexico_City"),
    ("Canada Central Standard Time", "America/Regina"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Eastern Standard Time (Mexico)", "America/Cancun"),
    ("Eastern Standard Time", "America/New_York"),
    ("Haiti Standard Time", "America/Port-au-Prince"),
    ("Cuba Standard Time", "America/Havana"),
    ("US Eastern Standard Time", "America/Indiana/Indianapolis"),
    ("Turks And Caicos Standard Time", "America/Grand_Turk"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Venezuela Standard Time", "America/Caracas"),
    ("Central Brazilian Standard Time", "America/Cuiaba"),
    ("SA Western Standard Time", "America/La_Paz"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("Paraguay Standard Time", "America/Asuncion"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("Tocantins Standard Time", "America/Araguaina"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("SA Eastern Standard Time", "America/Cayenne"),
    ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ("Greenland Standard Time", "America/Nuuk"),
    ("Montevideo Standard Time", "America/Montevideo"),
    ("Magallanes Standard Time", "America/Punta_Arenas"),
    ("Saint Pierre Standard Time", "America/Miquelon"),
    ("Bahia Standard Time", "America/Bahia"),
    ("UTC-02", "Etc/GMT+2"),
    ("Azores Standard Time", "Atlantic/Azores"),
    ("Cape Verde Standard Time", "Atlantic/Cape_Verde"),
    ("UTC", "UTC"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("Sao Tome Standard Time", "Africa/Sao_Tome"),
    ("Morocco Standard Time", "Africa/Casablanca"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("W. Central Africa Standard Time", "Africa/Lagos"),
    ("Jordan Standard Time", "Asia/Amman"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Middle East Standard Time", "Asia/Beirut"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("Syria Standard Time", "Asia/Damascus"),
    ("West Bank Standard Time", "Asia/Hebron"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("South Sudan Standard Time", "Africa/Juba"),
    ("Kaliningrad Standard Time", "Europe/Kaliningrad"),
    ("Sudan Standard Time", "Africa/Khartoum"),
    ("Libya Standard Time", "Africa/Tripoli"),
    ("Namibia Standard Time", "Africa/Windhoek"),
    ("Arabic Standard Time", "Asia/Baghdad"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Arab Standard Time", "Asia/Riyadh"),
    ("Belarus Standard Time", "Europe/Minsk"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("E. Africa Standard Time", "Africa/Nairobi"),
    ("Iran Standard Time", "Asia/Tehran"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("Astrakhan Standard Time", "Europe/Astrakhan"),
    ("Azerbaijan Standard Time", "Asia/Baku"),
    ("Russia Time Zone 3", "Europe/Samara"),
    ("Mauritius Standard Time", "Indian/Mauritius"),
    ("Saratov Standard Time", "Europe/Saratov"),
    ("Georgian Standard Time", "Asia/Tbilisi"),
    ("Volgograd Standard Time", "Europe/Volgograd"),
    ("Caucasus Standard Time", "Asia/Yerevan"),
    ("Afghanistan Standard Time", "Asia/Kabul"),
    ("West Asia Standard Time", "Asia/Tashkent"),
    ("Ekaterinburg Standard Time", "Asia/Yekaterinburg"),
    ("Pakistan Standard Time", "Asia/Karachi"),
    ("Qyzylorda Standard Time", "Asia/Qyzylorda"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Sri Lanka Standard Time", "Asia/Colombo"),
    ("Nepal Standard Time", "Asia/Kathmandu"),
    ("Central Asia Standard Time", "Asia/Almaty"),
    ("Bangladesh Standard Time", "Asia/Dhaka"),
    ("Omsk Standard Time", "Asia/Omsk"),
    ("Myanmar Standard Time", "Asia/Yangon"),
    ("SE Asia Standard Time", "Asia/Bangkok"),
    ("Altai Standard Time", "Asia/Barnaul"),
    ("W. Mongolia Standard Time", "Asia/Hovd"),
    ("North Asia Standard Time", "Asia/Krasnoyarsk"),
    ("N. Central Asia Standard Time", "Asia/Novosibirsk"),
    ("Tomsk Standard Time", "Asia/Tomsk"),
    ("China Standard Time", "Asia/Shanghai"),
    ("North Asia East Standard Time", "Asia/Irkutsk"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("W. Australia Standard Time", "Australia/Perth"),
    ("Taipei Standard Time", "Asia/Taipei"),
    ("Ulaanbaatar Standard Time", "Asia/Ulaanbaatar"),
    ("Aus Central W. Standard Time", "Australia/Eucla"),
    ("Transbaikal Standard Time", "Asia/Chita"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("North Korea Standard Time", "Asia/Pyongyang"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("Yakutsk Standard Time", "Asia/Yakutsk"),
    ("Cen. Australia Standard Time", "Australia/Adelaide"),
    ("AUS Central Standard Time", "Australia/Darwin"),
    ("E. Australia Standard Time", "Australia/Brisbane"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("West Pacific Standard Time", "Pacific/Port_Moresby"),
    ("Tasmania Standard Time", "Australia/Hobart"),
    ("Vladivostok Standard Time", "Asia/Vladivostok"),
    ("Lord Howe Standard Time", "Australia/Lord_Howe"),
    ("Bougainville Standard Time", "Pacific/Bougainville"),
    ("Russia Time Zone 10", "Asia/Srednekolymsk"),
    ("Magadan Standard Time", "Asia/Magadan"),
    ("Norfolk Standard Time", "Pacific/Norfolk"),
    ("Sakhalin Standard Time", "Asia/Sakhalin"),
    ("Central Pacific Standard Time", "Pacific/Guadalcanal"),
    ("Russia Time Zone 11", "Asia/Kamchatka"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("UTC+12", "Etc/GMT-12"),
    ("Fiji Standard Time", "Pacific/Fiji"),
    ("Kamchatka Standard Time", "Asia/Kamchatka"),
    ("Chatham Islands Standard Time", "Pacific/Chatham"),
    ("UTC+13", "Etc/GMT-13"),
    ("Tonga Standard Time", "Pacific/Tongatapu"),
    ("Samoa Standard Time", "Pacific/Apia"),
    ("Line Islands Standard Time", "Pacific/Kiritimati"),
];

/// Native stand-ins for the host, built on the crates the host uses, so the
/// tests see recurrence and timezone behaviour as it is on the Deck.
#[cfg(test)]
pub mod testing {
    use chrono::{DateTime, Offset, TimeZone};
    use rrule::RRuleSet;

    use super::Zones;

    pub struct HostLike;

    impl Zones for HostLike {
        fn offset(&self, tz: &str, utc: i64) -> Option<i32> {
            let tz: chrono_tz::Tz = tz.parse().ok()?;
            let at = DateTime::from_timestamp(utc, 0)?;
            Some(
                tz.from_utc_datetime(&at.naive_utc())
                    .offset()
                    .fix()
                    .local_minus_utc(),
            )
        }

        fn expand(
            &self,
            rrule: &str,
            dtstart: &str,
            tzid: Option<&str>,
            lo: i64,
            hi: i64,
            max: u16,
        ) -> Vec<i64> {
            // The same text the host builds before handing it to the rrule crate.
            let input = match tzid.filter(|tz| !tz.is_empty()) {
                Some(tz) => format!("DTSTART;TZID={tz}:{dtstart}\nRRULE:{rrule}"),
                None if dtstart.ends_with('Z') => format!("DTSTART:{dtstart}\nRRULE:{rrule}"),
                None => format!("DTSTART:{dtstart}Z\nRRULE:{rrule}"),
            };
            let Ok(set) = input.parse::<RRuleSet>() else {
                return Vec::new();
            };
            let (Some(after), Some(before)) = (
                DateTime::from_timestamp(lo, 0),
                DateTime::from_timestamp(hi, 0),
            ) else {
                return Vec::new();
            };
            set.after(after.with_timezone(&rrule::Tz::UTC))
                .before(before.with_timezone(&rrule::Tz::UTC))
                .all(max)
                .dates
                .into_iter()
                .map(|at| at.timestamp())
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::HostLike;
    use super::*;
    use crate::civil::Ymd;

    fn local(year: i64, month: u8, day: u8, hour: i64, minute: i64) -> i64 {
        Ymd { year, month, day }.days() * 86_400 + hour * 3_600 + minute * 60
    }

    #[test]
    fn iana_names_pass_through() {
        assert_eq!(iana_name("Europe/Rome"), "Europe/Rome");
        assert_eq!(iana_name("\"America/New_York\""), "America/New_York");
        assert_eq!(
            iana_name("/freeassociation.sourceforge.net/Tzfile/Europe/Paris"),
            "Europe/Paris"
        );
    }

    #[test]
    fn windows_names_become_iana_names() {
        assert_eq!(iana_name("W. Europe Standard Time"), "Europe/Berlin");
        assert_eq!(iana_name("romance standard time"), "Europe/Paris");
        assert_eq!(iana_name("Pacific Standard Time"), "America/Los_Angeles");
        assert_eq!(iana_name("tzone://Microsoft/Utc"), "UTC");
        assert_eq!(iana_name("tzone://Microsoft/Custom"), "");
    }

    #[test]
    fn every_windows_name_is_a_zone_the_host_knows() {
        for (windows, iana) in WINDOWS_ZONES {
            assert!(
                HostLike.offset(iana, 1_700_000_000).is_some(),
                "{windows} -> {iana}"
            );
        }
    }

    #[test]
    fn a_summer_time_in_rome_is_two_hours_ahead_of_utc() {
        let utc = local_to_utc(&HostLike, "Europe/Rome", local(2026, 7, 1, 9, 30));
        assert_eq!(utc, Some(local(2026, 7, 1, 7, 30)));
        let utc = local_to_utc(&HostLike, "Europe/Rome", local(2026, 12, 1, 9, 30));
        assert_eq!(utc, Some(local(2026, 12, 1, 8, 30)));
    }

    #[test]
    fn a_time_that_happens_twice_means_the_first() {
        // The clocks in Rome go back at 03:00 on 25 October 2026, so 02:30
        // is read twice. The first time is still summer time.
        let utc = local_to_utc(&HostLike, "Europe/Rome", local(2026, 10, 25, 2, 30));
        assert_eq!(utc, Some(local(2026, 10, 25, 0, 30)));
    }

    #[test]
    fn a_time_that_never_happens_uses_the_offset_from_before() {
        // The clocks go forward at 02:00 on 29 March 2026, so 02:30 does not
        // exist; the winter offset puts it at 01:30 UTC.
        let utc = local_to_utc(&HostLike, "Europe/Rome", local(2026, 3, 29, 2, 30));
        assert_eq!(utc, Some(local(2026, 3, 29, 1, 30)));
    }

    #[test]
    fn an_unknown_zone_is_no_answer() {
        assert_eq!(
            local_to_utc(&HostLike, "Mars/Olympus", local(2026, 7, 1, 9, 0)),
            None
        );
    }

    #[test]
    fn the_host_stand_in_expands_a_weekly_rule_in_local_time() {
        // Weekly on Thursdays at 09:00 in Rome, across the October clock change.
        let lo = local(2026, 10, 20, 0, 0);
        let hi = local(2026, 11, 3, 0, 0);
        let starts = HostLike.expand(
            "FREQ=WEEKLY;BYDAY=TH",
            "20261001T090000",
            Some("Europe/Rome"),
            lo,
            hi,
            50,
        );
        assert_eq!(
            starts,
            [local(2026, 10, 22, 7, 0), local(2026, 10, 29, 8, 0)]
        );
    }
}
