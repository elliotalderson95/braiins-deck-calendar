//! Reads events out of an iCalendar (`.ics`) file, a slice at a time.
//!
//! A widget gets about ten million instructions per frame, and a busy
//! calendar's file can be megabytes, so [`Scanner::step`] reads as much as a
//! work budget allows and stops, to continue on the next frame. Most of a
//! big file is events that are long over, so the scanner is built to pass them
//! cheaply: it looks for line ends a word at a time, branches on a property's
//! length and first letter before comparing names, skips what it doesn't need
//! (alarms, attendees, long descriptions) without copying it, and decodes
//! an event's text only once it is known to be wanted. It works on bytes, so
//! a stray byte that isn't UTF-8 can't spoil a whole feed.

use crate::civil::{SECS_PER_DAY, Ymd};

/// The most events one file may give. A feed that big is something else.
const MAX_EVENTS: usize = 4_000;
/// Longest title and place kept, in characters.
const MAX_TITLE: usize = 120;
const MAX_PLACE: usize = 80;
/// Longest rule kept, in bytes; longer ones are not real rules.
const MAX_RULE: usize = 400;

/// A logical line is cut off after this many bytes (a call must stay short even
/// when a file is one endless folded line); the rest reads as lines of its own.
const MAX_LOGICAL_LINE: usize = 16_384;

/// Work units a line costs, besides one per 32 bytes: passing it by, and
/// reading what it says.
const UNITS_PER_LINE: usize = 1;
const UNITS_PER_PROPERTY: usize = 8;

/// The zone a time is written in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Zone {
    /// A `Z` suffix.
    Utc,
    /// No zone: the same wall-clock time wherever the calendar is read.
    Floating,
    /// A `TZID`, as the calendar spelled it.
    Named(String),
}

/// A start, end or exception date as the calendar wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IcsTime {
    /// A whole day, as a day number.
    Date(i64),
    /// A time of day. `local_secs` is the wall-clock time counted in seconds
    /// since 1970 as if it were UTC.
    DateTime { local_secs: i64, zone: Zone },
}

impl IcsTime {
    /// The day number of the date as written, zone not applied.
    #[must_use]
    pub fn day(&self) -> i64 {
        match self {
            IcsTime::Date(day) => *day,
            IcsTime::DateTime { local_secs, .. } => local_secs.div_euclid(SECS_PER_DAY),
        }
    }
}

/// One `VEVENT`, with what the widget uses of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawEvent {
    pub uid: String,
    pub title: String,
    pub place: String,
    pub start: IcsTime,
    pub end: Option<IcsTime>,
    /// `DURATION`, in seconds, for events that give one instead of an end.
    pub duration: Option<i64>,
    pub rrule: Option<String>,
    pub exdates: Vec<IcsTime>,
    /// Set on an event that replaces one occurrence of a repeating event:
    /// the occurrence it replaces.
    pub recurrence_id: Option<IcsTime>,
    pub cancelled: bool,
    /// Marked "free" (`TRANSP:TRANSPARENT`).
    pub free: bool,
}

/// The days worth keeping, as inclusive day numbers (as written in the file,
/// so the range should carry a day or two of slack for zones).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keep {
    pub from_day: i64,
    pub to_day: i64,
}

impl Keep {
    fn wants(self, b: &Builder, start: &IcsTime, rule: Option<&[u8]>) -> bool {
        if b.cancelled && b.recurrence_id.is_none() {
            return false;
        }
        let in_range = |day: i64| (self.from_day..=self.to_day).contains(&day);
        let first = start.day();
        if let Some(replaced) = &b.recurrence_id
            && in_range(replaced.day())
        {
            return true;
        }
        if first > self.to_day {
            return false;
        }
        if let Some(rule) = rule {
            return until_day(rule).is_none_or(|until| until >= self.from_day);
        }
        let last = match (&b.end, b.duration) {
            (Some(end), _) => end.day(),
            (None, Some(secs)) => first + secs / SECS_PER_DAY,
            (None, None) => first,
        };
        last >= self.from_day
    }
}

/// The day of a rule's `UNTIL`, as written.
fn until_day(rule: &[u8]) -> Option<i64> {
    let at = rule.windows(6).position(|w| w == b"UNTIL=")? + 6;
    parse_date(rule.get(at..at + 8)?).map(Ymd::days)
}

/// Whether a scanner has read the whole file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    More,
    Done,
}

/// A text value waiting to be decoded: where it is in the data (without a
/// copy), or its own bytes when the line was folded.
#[derive(Default)]
enum Text {
    #[default]
    None,
    Span(usize, usize),
    Owned(Vec<u8>),
}

impl Text {
    fn bytes<'a>(&'a self, data: &'a [u8]) -> &'a [u8] {
        match self {
            Text::None => &[],
            Text::Span(from, to) => &data[*from..*to],
            Text::Owned(bytes) => bytes,
        }
    }
}

#[derive(Default)]
struct Builder {
    uid: Text,
    title: Text,
    place: Text,
    start: Option<IcsTime>,
    end: Option<IcsTime>,
    duration: Option<i64>,
    rrule: Text,
    exdates: Vec<IcsTime>,
    recurrence_id: Option<IcsTime>,
    cancelled: bool,
    free: bool,
}

/// What a property line is, as far as the scanner cares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Other,
    Begin,
    End,
    Uid,
    Summary,
    Location,
    Start,
    EndTime,
    Duration,
    Rule,
    Exdate,
    RecurrenceId,
    Status,
    Transp,
    CalName,
}

/// Reads a file's events across as many calls as it takes.
///
/// Every call must be given the same data: the scanner keeps positions in it.
#[derive(Default)]
pub struct Scanner {
    pos: usize,
    in_event: bool,
    /// How deep inside an event's sub-components, such as alarms.
    nested: u32,
    builder: Builder,
    /// The events worth keeping.
    pub events: Vec<RawEvent>,
    /// The calendar's own name (`X-WR-CALNAME`), when it has one.
    pub name: Option<String>,
    /// How many events the file holds, kept or not.
    pub seen: usize,
}

impl Scanner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads on from where the last call stopped, line by line until `budget`
    /// units of work are spent or the data ends. A unit is about a hundred
    /// instructions; a line costs one, plus one for each 32 bytes, plus eight
    /// if it says something that is kept.
    pub fn step(&mut self, data: &[u8], budget: usize, keep: Keep) -> Step {
        let mut spent = 0;
        while self.pos < data.len() {
            if spent >= budget {
                return Step::More;
            }
            spent += self.line(data, keep);
        }
        Step::Done
    }

    /// How much of `data` has been read, from 0 to 1.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a progress fraction")]
    pub fn progress(&self, data: &[u8]) -> f32 {
        if data.is_empty() {
            1.0
        } else {
            self.pos as f32 / data.len() as f32
        }
    }

    /// Reads one logical line (with its continuations) and returns its cost in work units.
    fn line(&mut self, data: &[u8], keep: Keep) -> usize {
        let start = self.pos;
        let first_end = end_of_line(data, start);
        let first = strip_cr(&data[start..first_end]);
        let mut next = (first_end + 1).min(data.len());

        let name_len = first
            .iter()
            .take(32)
            .position(|&b| b == b':' || b == b';')
            .unwrap_or(first.len().min(32));
        let kind = self.classify(&first[..name_len]);

        // Continuation lines start with a space or tab; only a wanted
        // property pays for joining them up.
        let mut joined: Option<Vec<u8>> = None;
        while next < data.len()
            && (data[next] == b' ' || data[next] == b'\t')
            && next - start < MAX_LOGICAL_LINE
        {
            let end = end_of_line(data, next);
            if kind != Kind::Other {
                joined
                    .get_or_insert_with(|| first.to_vec())
                    .extend_from_slice(strip_cr(&data[next + 1..end]));
            }
            next = (end + 1).min(data.len());
        }
        self.pos = next;
        let cost = UNITS_PER_LINE + (next - start) / 32;
        if kind == Kind::Other {
            return cost;
        }
        match joined {
            Some(bytes) => self.property(kind, &bytes, None, data, keep),
            None => self.property(kind, first, Some(start), data, keep),
        }
        cost + UNITS_PER_PROPERTY
    }

    /// What a line with this property name is, in this place in the file.
    fn classify(&self, name: &[u8]) -> Kind {
        let is = |wanted: &[u8]| name.eq_ignore_ascii_case(wanted);
        let Some(&first) = name.first() else {
            return Kind::Other;
        };
        let kind = match (name.len(), first.to_ascii_uppercase()) {
            (5, b'B') if is(b"BEGIN") => Kind::Begin,
            (3, b'E') if is(b"END") => Kind::End,
            (12, b'X') if is(b"X-WR-CALNAME") => Kind::CalName,
            (3, b'U') if is(b"UID") => Kind::Uid,
            (7, b'S') if is(b"SUMMARY") => Kind::Summary,
            (8, b'L') if is(b"LOCATION") => Kind::Location,
            (7, b'D') if is(b"DTSTART") => Kind::Start,
            (5, b'D') if is(b"DTEND") => Kind::EndTime,
            (8, b'D') if is(b"DURATION") => Kind::Duration,
            (5, b'R') if is(b"RRULE") => Kind::Rule,
            (6, b'E') if is(b"EXDATE") => Kind::Exdate,
            (13, b'R') if is(b"RECURRENCE-ID") => Kind::RecurrenceId,
            (6, b'S') if is(b"STATUS") => Kind::Status,
            (6, b'T') if is(b"TRANSP") => Kind::Transp,
            _ => Kind::Other,
        };
        match kind {
            Kind::Begin | Kind::End => kind,
            Kind::CalName if !self.in_event => kind,
            _ if !self.in_event || self.nested > 0 => Kind::Other,
            _ => kind,
        }
    }

    /// Takes in a property line. `at` is where the line starts in `data`
    /// when `line` is a slice of it, so values can be kept as positions
    /// instead of copies.
    fn property(&mut self, kind: Kind, line: &[u8], at: Option<usize>, data: &[u8], keep: Keep) {
        let Some((params_from, value_from)) = split_property(line) else {
            return;
        };
        let (params, value) = (&line[params_from..value_from - 1], &line[value_from..]);
        let text = |from: usize| match at {
            Some(line_start) => Text::Span(line_start + from, line_start + line.len()),
            None => Text::Owned(line[from..].to_vec()),
        };
        match kind {
            Kind::Begin => {
                if value.eq_ignore_ascii_case(b"VEVENT") && !self.in_event {
                    self.in_event = true;
                    self.nested = 0;
                    self.builder = Builder::default();
                } else if self.in_event {
                    self.nested += 1;
                }
            }
            Kind::End => {
                if self.nested > 0 {
                    self.nested -= 1;
                } else if self.in_event && value.eq_ignore_ascii_case(b"VEVENT") {
                    self.finish(data, keep);
                }
            }
            Kind::CalName => {
                self.name = Some(decode(value, MAX_TITLE)).filter(|name| !name.is_empty());
            }
            Kind::Uid => self.builder.uid = text(value_from),
            Kind::Summary => self.builder.title = text(value_from),
            Kind::Location => self.builder.place = text(value_from),
            Kind::Start => self.builder.start = parse_time(params, value),
            Kind::EndTime => self.builder.end = parse_time(params, value),
            Kind::Duration => self.builder.duration = parse_duration(value),
            Kind::Rule => {
                if value.len() <= MAX_RULE {
                    self.builder.rrule = text(value_from);
                }
            }
            Kind::Exdate => {
                self.builder.exdates.extend(
                    value
                        .split(|&c| c == b',')
                        .filter_map(|part| parse_time(params, part)),
                );
            }
            Kind::RecurrenceId => self.builder.recurrence_id = parse_time(params, value),
            Kind::Status => self.builder.cancelled = value.eq_ignore_ascii_case(b"CANCELLED"),
            Kind::Transp => self.builder.free = value.eq_ignore_ascii_case(b"TRANSPARENT"),
            Kind::Other => {}
        }
    }

    fn finish(&mut self, data: &[u8], keep: Keep) {
        self.in_event = false;
        self.seen += 1;
        let b = std::mem::take(&mut self.builder);
        let Some(start) = b.start.clone() else {
            return;
        };
        let rule = b.rrule.bytes(data);
        if !keep.wants(&b, &start, (!rule.is_empty()).then_some(rule))
            || self.events.len() >= MAX_EVENTS
        {
            return;
        }
        self.events.push(RawEvent {
            uid: decode(b.uid.bytes(data), 200),
            title: decode(b.title.bytes(data), MAX_TITLE),
            place: decode(b.place.bytes(data), MAX_PLACE),
            start,
            end: b.end,
            duration: b.duration,
            rrule: (!rule.is_empty()).then(|| String::from_utf8_lossy(rule).into_owned()),
            exdates: b.exdates,
            recurrence_id: b.recurrence_id,
            cancelled: b.cancelled,
            free: b.free,
        });
    }
}

/// Whether the data is an iCalendar file at all, rather than, say, the web
/// page a wrong link opens.
#[must_use]
pub fn looks_like_calendar(data: &[u8]) -> bool {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let start = data
        .iter()
        .take(64)
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(data.len().min(64));
    data[start..]
        .get(..15)
        .is_some_and(|head| head.eq_ignore_ascii_case(b"BEGIN:VCALENDAR"))
}

/// The index of the next newline at or after `from`, or the end of the data.
/// Looks at four bytes at a time.
fn end_of_line(data: &[u8], from: usize) -> usize {
    let mut at = from;
    while let Some(word) = data.get(at..at + 4) {
        let word = u32::from_le_bytes([word[0], word[1], word[2], word[3]]) ^ 0x0A0A_0A0A;
        // Some byte of the word is zero, that is, was a newline.
        if word.wrapping_sub(0x0101_0101) & !word & 0x8080_8080 != 0 {
            break;
        }
        at += 4;
    }
    while at < data.len() && data[at] != b'\n' {
        at += 1;
    }
    at
}

fn strip_cr(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Splits `NAME;PARAM=x:value` and returns where the parameters start and
/// where the value starts (just after the first colon that isn't inside
/// quotes). The parameters end one byte before the value starts.
fn split_property(line: &[u8]) -> Option<(usize, usize)> {
    let mut quoted = false;
    let colon = line.iter().position(|&b| {
        if b == b'"' {
            quoted = !quoted;
        }
        b == b':' && !quoted
    })?;
    match line[..colon].iter().position(|&b| b == b';') {
        Some(semi) => Some((semi + 1, colon + 1)),
        None => Some((colon, colon + 1)),
    }
}

/// The value of parameter `key` in `A=1;B="two";C=3`.
fn param<'a>(params: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut quoted = false;
    let mut start = 0;
    let mut found = None;
    for at in 0..=params.len() {
        if at < params.len() {
            if params[at] == b'"' {
                quoted = !quoted;
            }
            if params[at] != b';' || quoted {
                continue;
            }
        }
        let part = &params[start..at];
        if let Some(eq) = part.iter().position(|&b| b == b'=')
            && part[..eq].eq_ignore_ascii_case(key)
        {
            found = Some(part[eq + 1..].trim_ascii());
            break;
        }
        start = at + 1;
    }
    found.map(|value| {
        value
            .strip_prefix(b"\"")
            .and_then(|v| v.strip_suffix(b"\""))
            .unwrap_or(value)
    })
}

fn digits(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(
        bytes
            .iter()
            .fold(0_i64, |n, &d| n * 10 + i64::from(d - b'0')),
    )
}

fn parse_date(bytes: &[u8]) -> Option<Ymd> {
    if bytes.len() != 8 {
        return None;
    }
    let date = Ymd {
        year: digits(&bytes[..4])?,
        month: u8::try_from(digits(&bytes[4..6])?).ok()?,
        day: u8::try_from(digits(&bytes[6..8])?).ok()?,
    };
    date.is_valid().then_some(date)
}

/// `20261015`, `20261015T093000` or `20261015T093000Z`, with the zone from
/// the property's `TZID`.
fn parse_time(params: &[u8], value: &[u8]) -> Option<IcsTime> {
    let value = value.trim_ascii();
    let date = parse_date(value.get(..8)?)?;
    let day = date.days();
    let whole_day = !params.is_empty()
        && param(params, b"VALUE").is_some_and(|kind| kind.eq_ignore_ascii_case(b"DATE"));
    if value.len() == 8 || whole_day {
        return Some(IcsTime::Date(day));
    }
    if value.get(8) != Some(&b'T') {
        return None;
    }
    let hour = digits(value.get(9..11)?)?;
    let minute = digits(value.get(11..13)?)?;
    let second = value.get(13..15).map_or(Some(0), digits)?;
    if hour > 24 || minute > 59 || second > 60 {
        return None;
    }
    let local_secs = day * SECS_PER_DAY + hour * 3_600 + minute * 60 + second;
    let zone = if value.ends_with(b"Z") {
        Zone::Utc
    } else if let Some(tzid) = param(params, b"TZID") {
        Zone::Named(String::from_utf8_lossy(tzid).into_owned())
    } else {
        Zone::Floating
    };
    Some(IcsTime::DateTime { local_secs, zone })
}

/// `P1DT2H30M`, `PT45M`, `P2W` and the like, in seconds.
fn parse_duration(value: &[u8]) -> Option<i64> {
    let value = value.trim_ascii();
    let (negative, value) = match value.first()? {
        b'-' => (true, &value[1..]),
        b'+' => (false, &value[1..]),
        _ => (false, value),
    };
    let mut rest = value.strip_prefix(b"P")?;
    let (mut total, mut in_time, mut any) = (0_i64, false, false);
    while let Some(&next) = rest.first() {
        if next == b'T' {
            in_time = true;
            rest = &rest[1..];
            continue;
        }
        let end = rest.iter().position(|b| !b.is_ascii_digit())?;
        let n = digits(&rest[..end])?;
        let unit = match (rest[end], in_time) {
            (b'W', false) => 7 * SECS_PER_DAY,
            (b'D', false) => SECS_PER_DAY,
            (b'H', true) => 3_600,
            (b'M', true) => 60,
            (b'S', true) => 1,
            _ => return None,
        };
        total += n * unit;
        any = true;
        rest = &rest[end + 1..];
    }
    any.then_some(if negative { -total } else { total })
}

/// A text value: the bytes as UTF-8 (anything else becomes �), with
/// iCalendar's escapes undone and line breaks turned into spaces.
fn decode(value: &[u8], max_chars: usize) -> String {
    let raw = String::from_utf8_lossy(value);
    let raw = raw.trim();
    if !raw.contains('\\') {
        return match raw.char_indices().nth(max_chars) {
            Some((cut, _)) => format!("{}…", &raw[..cut]),
            None => raw.to_owned(),
        };
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let mut count = 0;
    while let Some(c) = chars.next() {
        if count >= max_chars {
            out.push('…');
            break;
        }
        let c = if c == '\\' {
            match chars.next() {
                Some('n' | 'N') | None => ' ',
                Some(other) => other,
            }
        } else {
            c
        };
        out.push(c);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    const EVERYTHING: Keep = Keep {
        from_day: i64::MIN,
        to_day: i64::MAX,
    };

    fn scan(ics: &str, keep: Keep) -> Scanner {
        let mut scanner = Scanner::new();
        assert_eq!(scanner.step(ics.as_bytes(), usize::MAX, keep), Step::Done);
        scanner
    }

    fn day(year: i64, month: u8, day: u8) -> i64 {
        Ymd { year, month, day }.days()
    }

    #[test]
    fn reads_a_utc_event() {
        let s = scan(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a@x\r\nDTSTART:20261015T093000Z\r\n\
             DTEND:20261015T103000Z\r\nSUMMARY:Standup\r\nLOCATION:Room 4\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            EVERYTHING,
        );
        let [e] = &s.events[..] else {
            panic!("one event, got {:?}", s.events)
        };
        assert_eq!(e.title, "Standup");
        assert_eq!(e.place, "Room 4");
        assert_eq!(e.uid, "a@x");
        let secs = day(2026, 10, 15) * 86_400;
        assert_eq!(
            e.start,
            IcsTime::DateTime {
                local_secs: secs + 9 * 3_600 + 1_800,
                zone: Zone::Utc
            }
        );
        assert_eq!(
            e.end,
            Some(IcsTime::DateTime {
                local_secs: secs + 10 * 3_600 + 1_800,
                zone: Zone::Utc
            })
        );
    }

    #[test]
    fn reads_zoned_floating_and_whole_day_times() {
        let s = scan(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Rome:20261015T090000\nSUMMARY:A\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20261016T090000\nSUMMARY:B\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261017\nDTEND;VALUE=DATE:20261019\nSUMMARY:C\nEND:VEVENT\n",
            EVERYTHING,
        );
        let at = |d: i64, secs: i64| d * 86_400 + secs;
        assert_eq!(
            s.events[0].start,
            IcsTime::DateTime {
                local_secs: at(day(2026, 10, 15), 9 * 3_600),
                zone: Zone::Named("Europe/Rome".into())
            }
        );
        assert_eq!(
            s.events[1].start,
            IcsTime::DateTime {
                local_secs: at(day(2026, 10, 16), 9 * 3_600),
                zone: Zone::Floating
            }
        );
        assert_eq!(s.events[2].start, IcsTime::Date(day(2026, 10, 17)));
        assert_eq!(s.events[2].end, Some(IcsTime::Date(day(2026, 10, 19))));
    }

    #[test]
    fn a_quoted_zone_may_hold_a_colon_and_a_windows_name_a_space() {
        let s = scan(
            "BEGIN:VEVENT\nDTSTART;TZID=\"W. Europe: Standard Time\":20261015T090000\nSUMMARY:A\nEND:VEVENT\n",
            EVERYTHING,
        );
        let IcsTime::DateTime { zone, .. } = &s.events[0].start else {
            panic!()
        };
        assert_eq!(zone, &Zone::Named("W. Europe: Standard Time".into()));
    }

    #[test]
    fn joins_folded_lines_and_undoes_escapes() {
        let s = scan(
            "BEGIN:VEVENT\r\nDTSTART:20261015T093000Z\r\nSUMMARY:Lunch\\, with a very long\r\n  title\\nover two lines\r\nEND:VEVENT\r\n",
            EVERYTHING,
        );
        assert_eq!(
            s.events[0].title,
            "Lunch, with a very long title over two lines"
        );
    }

    #[test]
    fn property_names_match_whatever_the_case() {
        let s = scan(
            "begin:vevent\ndtstart:20261015T093000Z\nSummary:Lower\nend:vevent\n",
            EVERYTHING,
        );
        assert_eq!(s.events[0].title, "Lower");
    }

    #[test]
    fn a_fold_may_split_a_multibyte_character() {
        let ics = "BEGIN:VEVENT\nDTSTART:20261015T093000Z\nSUMMARY:Caff\u{e8}\nEND:VEVENT\n";
        let bytes = ics.as_bytes();
        let at = bytes
            .windows(2)
            .position(|w| w == [0xC3, 0xA8])
            .expect("the accent");
        let mut folded = bytes[..=at].to_vec();
        folded.extend_from_slice(b"\n ");
        folded.extend_from_slice(&bytes[at + 1..]);
        let mut scanner = Scanner::new();
        scanner.step(&folded, usize::MAX, EVERYTHING);
        assert_eq!(scanner.events[0].title, "Caff\u{e8}");
    }

    #[test]
    fn bytes_that_are_not_utf8_do_not_spoil_the_feed() {
        let mut data =
            b"BEGIN:VEVENT\nDTSTART:20261015T093000Z\nSUMMARY:Bad \xFF byte\nEND:VEVENT\n".to_vec();
        data.extend_from_slice(
            b"BEGIN:VEVENT\nDTSTART:20261016T093000Z\nSUMMARY:Fine\nEND:VEVENT\n",
        );
        let mut scanner = Scanner::new();
        scanner.step(&data, usize::MAX, EVERYTHING);
        assert_eq!(scanner.events.len(), 2);
        assert_eq!(scanner.events[0].title, "Bad \u{fffd} byte");
    }

    #[test]
    fn long_titles_are_cut_with_an_ellipsis() {
        let long = "x".repeat(300);
        let s = scan(
            &format!("BEGIN:VEVENT\nDTSTART:20261015T093000Z\nSUMMARY:{long}\nEND:VEVENT\n"),
            EVERYTHING,
        );
        assert_eq!(s.events[0].title.chars().count(), MAX_TITLE + 1);
        assert!(s.events[0].title.ends_with('…'));
    }

    #[test]
    fn an_alarm_does_not_overwrite_the_events_summary() {
        let s = scan(
            "BEGIN:VEVENT\nDTSTART:20261015T093000Z\nSUMMARY:Real\nBEGIN:VALARM\nSUMMARY:Alarm text\n\
             DESCRIPTION:x\nEND:VALARM\nLOCATION:Here\nEND:VEVENT\n",
            EVERYTHING,
        );
        assert_eq!(s.events[0].title, "Real");
        assert_eq!(s.events[0].place, "Here");
    }

    #[test]
    fn reads_rules_exceptions_overrides_and_status() {
        let s = scan(
            "BEGIN:VEVENT\nUID:r\nDTSTART;TZID=Europe/Rome:20261001T090000\nRRULE:FREQ=WEEKLY;BYDAY=TH\n\
             EXDATE;TZID=Europe/Rome:20261008T090000,20261015T090000\nSUMMARY:Weekly\nTRANSP:TRANSPARENT\nEND:VEVENT\n\
             BEGIN:VEVENT\nUID:r\nRECURRENCE-ID;TZID=Europe/Rome:20261022T090000\nDTSTART;TZID=Europe/Rome:20261022T150000\n\
             SUMMARY:Weekly (moved)\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20261020T090000Z\nSTATUS:CANCELLED\nSUMMARY:Gone\nEND:VEVENT\n",
            EVERYTHING,
        );
        assert_eq!(s.events.len(), 2, "the cancelled event is dropped");
        let weekly = &s.events[0];
        assert_eq!(weekly.rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=TH"));
        assert_eq!(weekly.exdates.len(), 2);
        assert!(weekly.free);
        let moved = &s.events[1];
        assert_eq!(moved.uid, "r");
        assert_eq!(
            moved.recurrence_id.as_ref().map(IcsTime::day),
            Some(day(2026, 10, 22))
        );
    }

    #[test]
    fn a_cancelled_occurrence_is_kept_so_it_can_hide_the_original() {
        let s = scan(
            "BEGIN:VEVENT\nUID:r\nRECURRENCE-ID:20261022T070000Z\nDTSTART:20261022T070000Z\nSTATUS:CANCELLED\nSUMMARY:x\nEND:VEVENT\n",
            EVERYTHING,
        );
        assert_eq!(s.events.len(), 1);
        assert!(s.events[0].cancelled);
    }

    #[test]
    fn reads_durations() {
        assert_eq!(parse_duration(b"PT1H"), Some(3_600));
        assert_eq!(parse_duration(b"P1DT2H30M"), Some(86_400 + 9_000));
        assert_eq!(parse_duration(b"P2W"), Some(14 * 86_400));
        assert_eq!(parse_duration(b"-PT15M"), Some(-900));
        assert_eq!(parse_duration(b"PT"), None);
        assert_eq!(parse_duration(b"1H"), None);
    }

    #[test]
    fn only_events_in_range_are_kept() {
        let keep = Keep {
            from_day: day(2026, 10, 10),
            to_day: day(2026, 11, 10),
        };
        let s = scan(
            "BEGIN:VEVENT\nDTSTART:20260101T090000Z\nSUMMARY:old\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20261015T090000Z\nSUMMARY:now\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20270101T090000Z\nSUMMARY:later\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261008\nDTEND;VALUE=DATE:20261012\nSUMMARY:spans in\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20200101T090000Z\nRRULE:FREQ=WEEKLY\nSUMMARY:rule forever\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20200101T090000Z\nRRULE:FREQ=WEEKLY;UNTIL=20210101T000000Z\nSUMMARY:rule over\nEND:VEVENT\n\
             BEGIN:VEVENT\nDTSTART:20200101T090000Z\nRRULE:FREQ=YEARLY;UNTIL=20261231\nSUMMARY:rule date until\nEND:VEVENT\n",
            keep,
        );
        let titles: Vec<&str> = s.events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            ["now", "spans in", "rule forever", "rule date until"]
        );
        assert_eq!(s.seen, 7);
    }

    #[test]
    fn reading_in_small_steps_gives_the_same_events() {
        let mut ics = String::from("BEGIN:VCALENDAR\r\nX-WR-CALNAME:Family\r\n");
        for n in 0..300 {
            write!(
                ics,
                "BEGIN:VEVENT\r\nUID:{n}\r\nDTSTART:20261015T0930{:02}Z\r\nSUMMARY:Event {n} with a longer title that gets folded\r\n over two lines\r\n\
                 DESCRIPTION:{}\r\nEND:VEVENT\r\n",
                n % 60,
                "x".repeat(200),
            )
            .expect("BUG: writing to a String cannot fail");
        }
        ics.push_str("END:VCALENDAR\r\n");
        let whole = scan(&ics, EVERYTHING);
        for budget in [1, 7, 100, 1_000] {
            let mut scanner = Scanner::new();
            let mut steps = 0;
            while scanner.step(ics.as_bytes(), budget, EVERYTHING) == Step::More {
                steps += 1;
            }
            assert!(steps > 0);
            assert_eq!(scanner.events, whole.events, "budget {budget}");
            assert_eq!(scanner.name.as_deref(), Some("Family"));
        }
        assert_eq!(whole.events.len(), 300);
    }

    #[test]
    fn a_truncated_last_event_is_left_out() {
        let s = scan(
            "BEGIN:VEVENT\nDTSTART:20261015T090000Z\nSUMMARY:ok\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:2026",
            EVERYTHING,
        );
        assert_eq!(s.events.len(), 1);
    }

    #[test]
    fn events_without_a_start_are_skipped() {
        let s = scan(
            "BEGIN:VEVENT\nSUMMARY:No start\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:garbage\nSUMMARY:Bad\nEND:VEVENT\n",
            EVERYTHING,
        );
        assert!(s.events.is_empty());
        assert_eq!(s.seen, 2);
    }

    #[test]
    fn finds_line_ends_a_word_at_a_time() {
        let data = b"abcdefghijkl\nmnopqrstuvwx\r\nyz";
        assert_eq!(end_of_line(data, 0), 12);
        assert_eq!(end_of_line(data, 13), 26);
        assert_eq!(end_of_line(data, 27), 29);
        for from in 0..data.len() {
            let expected = data[from..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(data.len(), |i| from + i);
            assert_eq!(end_of_line(data, from), expected, "from {from}");
        }
        assert_eq!(end_of_line(b"\n\n\n\n\n", 0), 0);
        assert_eq!(end_of_line(b"", 0), 0);
    }

    #[test]
    fn tells_a_calendar_from_a_web_page() {
        assert!(looks_like_calendar(b"BEGIN:VCALENDAR\r\nVERSION:2.0"));
        assert!(looks_like_calendar(b"\xEF\xBB\xBF\n begin:vcalendar\n"));
        assert!(!looks_like_calendar(b"<!DOCTYPE html><html>"));
        assert!(!looks_like_calendar(b""));
    }
}
