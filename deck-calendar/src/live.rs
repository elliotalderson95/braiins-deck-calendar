//! The entry points the host calls: fetching each calendar's link now and
//! then, reading what comes back a little at a time (a frame has about ten
//! million instructions to spend, a busy calendar can be megabytes), and the
//! taps that move around the calendar.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[expect(
    clippy::wildcard_imports,
    reason = "runtime code uses the SDK's builders, host shims and macros throughout"
)]
use bmc_wasm_sdk::*;

use crate::Params;
use crate::civil::{self, SECS_PER_DAY, Ymd};
use crate::expand::{self, Cx, Occurrence, Window};
use crate::ics::{self, Keep, RawEvent, Scanner, Step};
#[cfg(not(any(
    feature = "preview-links",
    feature = "preview-errors",
    feature = "preview-failed",
    feature = "preview-big"
)))]
use crate::manifest_params::credentials::{calendar_1_link, calendar_2_link, calendar_3_link};
use crate::model::{self, DateStyle, Event, Fault, MAX_CALENDARS, Schedule, SourceState};
use crate::nav::{Nav, Tap};
use crate::settings::{Look, Settings};
use crate::view::{self, CalendarView, Palette, View};
use crate::zones::Zones;

/// The credential slots that hold the calendars' links, and the placeholders
/// the host swaps for them as a request leaves.
const SLOT_KEYS: [&str; MAX_CALENDARS] = ["calendar_1_link", "calendar_2_link", "calendar_3_link"];
#[cfg(not(any(
    feature = "preview-links",
    feature = "preview-errors",
    feature = "preview-failed",
    feature = "preview-big"
)))]
const LINKS: [&str; MAX_CALENDARS] = [
    calendar_1_link::TOKEN,
    calendar_2_link::TOKEN,
    calendar_3_link::TOKEN,
];

/// What stands in for the saved accounts in the desktop preview.
#[cfg(feature = "preview-links")]
const LINKS: [&str; MAX_CALENDARS] = [
    "http://127.0.0.1:8777/apple.ics",
    "http://127.0.0.1:8777/google.ics",
    "http://127.0.0.1:8777/outlook.ics",
];
#[cfg(feature = "preview-errors")]
const LINKS: [&str; MAX_CALENDARS] = [
    "http://127.0.0.1:8777/apple.ics",
    "http://127.0.0.1:8777/web.ics",
    "http://127.0.0.1:8777/gone.ics",
];
#[cfg(feature = "preview-failed")]
const LINKS: [&str; MAX_CALENDARS] = [
    "http://127.0.0.1:8777/gone.ics",
    "http://127.0.0.1:8777/web.ics",
    "http://127.0.0.1:8777/gone.ics",
];
#[cfg(feature = "preview-big")]
const LINKS: [&str; MAX_CALENDARS] = [
    "http://127.0.0.1:8777/big.ics",
    "http://127.0.0.1:8777/google.ics",
    "http://127.0.0.1:8777/outlook.ics",
];

/// Work units of a calendar file read per frame (see `Scanner::step`; a unit
/// is a few hundred instructions, and a frame has ten million to spend).
const SCAN_UNITS: usize = 6_000;
/// Repeating events worked out per frame.
const EXPAND_EVENTS: usize = 16;
/// The most occurrences one calendar may give, all calendars together may
/// show, and the most kept between runs. More is no calendar anyone reads, and
/// each occurrence costs a few thousand instructions every time the screen is
/// rebuilt or the events are read back, in a frame that has ten million.
const MAX_OCCURRENCES: usize = 800;
const MAX_TOTAL: usize = 1_500;
const MAX_SAVED: usize = 1_000;
/// How long a downloaded calendar may take to arrive.
const TIMEOUT: Duration = Duration::from_mins(1);
/// Waits after a failed try, getting longer, in seconds.
const BACKOFF: [i64; 5] = [60, 120, 300, 600, 900];
const CACHE_KEY: &str = "events";

/// The host, as [`Zones`].
struct Host;

impl Zones for Host {
    fn offset(&self, tz: &str, utc: i64) -> Option<i32> {
        let local = calendar::tz_convert(utc, tz)?;
        let date = Ymd {
            year: i64::from(local.year),
            month: local.month,
            day: local.day,
        };
        let wall = date.days() * SECS_PER_DAY
            + i64::from(local.hour) * 3_600
            + i64::from(local.minute) * 60
            + i64::from(local.second);
        i32::try_from(wall - utc).ok()
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
        calendar::expand_rrule(rrule, dtstart, tzid, lo, hi, max)
    }
}

/// A calendar file being read.
enum Phase {
    Scanning {
        body: Vec<u8>,
        scanner: Scanner,
        window: Window,
        keep: Keep,
    },
    Expanding {
        events: Vec<RawEvent>,
        replaced: HashSet<(String, i64)>,
        next: usize,
        found: Vec<Occurrence>,
        name: Option<String>,
        window: Window,
    },
}

struct Slot {
    state: SourceState,
    /// The calendar's own name, from its file.
    name: Option<String>,
    events: Vec<Occurrence>,
    request: Option<FetchRequestId>,
    /// When to ask again, in seconds since the epoch.
    due: i64,
    failures: usize,
    phase: Option<Phase>,
    bound: bool,
}

impl Slot {
    const fn new() -> Self {
        Self {
            state: SourceState::Waiting,
            name: None,
            events: Vec::new(),
            request: None,
            due: 0,
            failures: 0,
            phase: None,
            bound: false,
        }
    }

    fn failed(&mut self, fault: Fault, now: i64) {
        log_warn!(
            "deck-calendar: a calendar could not be read: {}",
            fault.headline()
        );
        self.state = SourceState::Failed(fault);
        self.phase = None;
        self.due = if fault == Fault::Refused {
            i64::MAX
        } else {
            now + BACKOFF[self.failures.min(BACKOFF.len() - 1)]
        };
        self.failures += 1;
    }

    fn succeeded(
        &mut self,
        mut found: Vec<Occurrence>,
        name: Option<String>,
        now: i64,
        every: i64,
    ) {
        if found.len() > MAX_OCCURRENCES {
            // Keep the ones nearest to now.
            found.select_nth_unstable_by_key(MAX_OCCURRENCES, |o| (o.start - now).abs());
            found.truncate(MAX_OCCURRENCES);
        }
        self.events = found;
        self.name = name;
        self.state = SourceState::Ready { at: now };
        self.failures = 0;
        self.due = now + every;
        self.phase = None;
    }
}

struct Live {
    settings: Settings,
    slots: [Slot; MAX_CALENDARS],
    schedule: Schedule,
    /// Whether the schedule must be rebuilt from the slots' events.
    dirty: bool,
    /// Whether the slots' events must be kept for the next run.
    unsaved: bool,
    /// The Deck's timezone.
    home: String,
    /// Today, the day shown and the month shown.
    nav: Nav,
}

thread_local! {
    /// The state, taken out while a call uses it. A call that traps (the host
    /// stops a frame that spends too many instructions) never puts it back,
    /// and the next call starts afresh from what was kept, where a borrow
    /// left hanging would break the widget for good.
    static LIVE: Cell<Option<Box<Live>>> = const { Cell::new(None) };
    static PENDING: RefCell<HashMap<FetchRequestId, usize>> = RefCell::new(HashMap::new());
}

fn with_live<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    let mut live = LIVE.with(Cell::take).unwrap_or_else(|| {
        let mut fresh = Box::new(Live::new());
        fresh.restore();
        fresh
    });
    let out = f(&mut live);
    LIVE.with(|slot| slot.set(Some(live)));
    out
}

impl Live {
    fn new() -> Self {
        Self {
            settings: Settings::from_params(&Params::current()),
            slots: [Slot::new(), Slot::new(), Slot::new()],
            schedule: Schedule::default(),
            dirty: false,
            unsaved: false,
            home: String::new(),
            // A guess at today (UTC's); the first frame knows the Deck's zone.
            nav: Nav::new(SystemTime::now().unix_secs.div_euclid(SECS_PER_DAY)),
        }
    }

    /// Notices what changed since the last frame: the day, the timezone, the
    /// links saved for the calendars; and asks for what is due.
    fn sync(&mut self, now: i64, bound: [bool; MAX_CALENDARS]) {
        let home = system::current().timezone().unwrap_or("UTC").to_owned();
        let zone_changed = home != self.home;
        if zone_changed {
            self.home = home;
        }
        let offset = i64::from(Host.offset(&self.home, now).unwrap_or(0));
        let today = (now + offset).div_euclid(SECS_PER_DAY);
        if today != self.nav.today {
            let (was, is) = (Ymd::from_days(self.nav.today), Ymd::from_days(today));
            self.nav.new_day(today);
            // A month on needs events from further ahead, so ask again.
            if (was.year, was.month) != (is.year, is.month) || zone_changed {
                self.ask_everyone_again();
            }
            self.dirty = true;
        } else if zone_changed {
            self.ask_everyone_again();
            self.dirty = true;
        }
        self.nav.expire(now);

        for (i, slot) in self.slots.iter_mut().enumerate() {
            match (bound[i], slot.bound) {
                (true, false) => slot.due = 0,
                (false, _)
                    if slot.bound
                        || !slot.events.is_empty()
                        || slot.state != SourceState::Waiting =>
                {
                    *slot = Slot::new();
                    self.dirty = true;
                    self.unsaved = true;
                }
                _ => {}
            }
            slot.bound = bound[i];
        }
        let every = i64::from(self.settings.refresh_minutes) * 60;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.bound && slot.request.is_none() && slot.phase.is_none() && now >= slot.due {
                let sent = FetchRequest::get(LINKS[i])
                    .headers("Accept: text/calendar, text/plain;q=0.5, */*;q=0.1")
                    .timeout(TIMEOUT)
                    .send(on_reply);
                match sent {
                    Some(id) => {
                        slot.request = Some(id);
                        PENDING.with(|p| p.borrow_mut().insert(id, i));
                        // Until the answer comes, don't ask again.
                        slot.due = now + every.min(300);
                    }
                    None => slot.failed(Fault::Network, now),
                }
            }
        }
    }

    fn ask_everyone_again(&mut self) {
        for slot in &mut self.slots {
            if slot.bound {
                slot.due = 0;
            }
        }
    }

    /// The time span events are worked out for: the grid of last month
    /// through that of next month.
    fn window(&self) -> (Window, Keep) {
        let date = Ymd::from_days(self.nav.today);
        let (from_year, from_month) = civil::add_months(date.year, date.month, -1);
        let (to_year, to_month) = civil::add_months(date.year, date.month, 2);
        let (lo_day, hi_day) = (
            civil::month_start(from_year, from_month) - 7,
            civil::month_start(to_year, to_month) + 7,
        );
        (
            Window {
                lo: (lo_day - 1) * SECS_PER_DAY,
                hi: (hi_day + 1) * SECS_PER_DAY,
            },
            Keep {
                from_day: lo_day - 2,
                to_day: hi_day + 2,
            },
        )
    }

    /// Reads a bit more of the first calendar being read, so a frame stays
    /// short. `true` while any calendar still has more to read.
    fn advance(&mut self, now: i64) -> bool {
        let home = self.home.clone();
        let every = i64::from(self.settings.refresh_minutes) * 60;
        let cx = Cx {
            zones: &Host,
            home: &home,
        };
        let Some(slot) = self.slots.iter_mut().find(|s| s.phase.is_some()) else {
            return false;
        };
        match slot.phase.take() {
            Some(Phase::Scanning {
                body,
                mut scanner,
                window,
                keep,
            }) => {
                if scanner.step(&body, SCAN_UNITS, keep) == Step::Done {
                    let events = std::mem::take(&mut scanner.events);
                    let replaced = expand::replaced_occurrences(&events, &cx);
                    log_info!(
                        "deck-calendar: read {} events, kept {}",
                        scanner.seen,
                        events.len()
                    );
                    slot.phase = Some(Phase::Expanding {
                        events,
                        replaced,
                        next: 0,
                        found: Vec::new(),
                        name: scanner.name.take(),
                        window,
                    });
                } else {
                    slot.phase = Some(Phase::Scanning {
                        body,
                        scanner,
                        window,
                        keep,
                    });
                }
            }
            Some(Phase::Expanding {
                events,
                replaced,
                mut next,
                mut found,
                name,
                window,
            }) => {
                let stop = (next + EXPAND_EVENTS).min(events.len());
                while next < stop {
                    expand::occurrences_of(&events[next], &replaced, &cx, window, &mut found);
                    next += 1;
                }
                if next >= events.len() {
                    slot.succeeded(found, name, now, every);
                    self.dirty = true;
                    self.unsaved = true;
                } else {
                    slot.phase = Some(Phase::Expanding {
                        events,
                        replaced,
                        next,
                        found,
                        name,
                        window,
                    });
                }
            }
            None => {}
        }
        self.slots.iter().any(|s| s.phase.is_some())
    }

    fn rebuild(&mut self, now: i64) {
        let mut all: Vec<(u8, &Occurrence)> = Vec::new();
        for (position, slot) in self.slots.iter().filter(|s| s.bound).enumerate() {
            let position = u8::try_from(position).unwrap_or(u8::MAX);
            all.extend(slot.events.iter().map(|o| (position, o)));
        }
        if all.len() > MAX_TOTAL {
            all.select_nth_unstable_by_key(MAX_TOTAL, |(_, o)| (o.start - now).abs());
            all.truncate(MAX_TOTAL);
        }
        let events = all
            .into_iter()
            .map(|(position, o)| Event::place(o, position, &Host, &self.home))
            .collect();
        self.schedule = Schedule::new(events);
        self.dirty = false;
    }

    fn save(&mut self, now: i64) {
        let mut items = Vec::new();
        for (i, slot) in self.slots.iter().enumerate().filter(|(_, s)| s.bound) {
            let source = u8::try_from(i).unwrap_or(0);
            items.extend(slot.events.iter().cloned().map(|o| (source, o)));
        }
        if items.len() > MAX_SAVED {
            items.select_nth_unstable_by_key(MAX_SAVED, |(_, o)| (o.start - now).abs());
            items.truncate(MAX_SAVED);
        }
        let mut text = String::with_capacity(items.len() * 72 + 24);
        text.push_str("v1|");
        text.push_str(&fmt!("{}", now));
        text.push('\n');
        for (i, slot) in self.slots.iter().enumerate().filter(|(_, s)| s.bound) {
            if let Some(name) = &slot.name {
                text.push_str(&fmt!("#name|{}|", i));
                text.push_str(&name.replace('\n', " "));
                text.push('\n');
            }
        }
        text.push_str(&model::encode(&items));
        kv::set(CACHE_KEY, text.as_bytes());
        self.unsaved = false;
    }

    /// Puts back what the last run kept, so the events show before the first download ends.
    fn restore(&mut self) {
        let _span = profile::span("restore");
        let Some(text) = kv::get_string(CACHE_KEY) else {
            return;
        };
        let (head, body) = text.split_once('\n').unwrap_or((&text, ""));
        let Some(saved_at) = head
            .strip_prefix("v1|")
            .and_then(|at| at.parse::<i64>().ok())
        else {
            return;
        };
        for line in body.lines() {
            let Some(rest) = line.strip_prefix("#name|") else {
                continue;
            };
            if let Some((slot, name)) = rest.split_once('|')
                && let Some(slot) = slot
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| self.slots.get_mut(i))
            {
                slot.name = Some(name.to_owned());
            }
        }
        for (source, occurrence) in model::decode(body) {
            if let Some(slot) = self.slots.get_mut(usize::from(source)) {
                slot.events.push(occurrence);
                slot.state = SourceState::Ready { at: saved_at };
            }
        }
        self.dirty = true;
    }

    fn calendars(&self) -> Vec<CalendarView> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.bound)
            .map(|(i, slot)| {
                let settings = &self.settings.calendars[i];
                CalendarView {
                    name: settings
                        .name
                        .clone()
                        .or_else(|| slot.name.clone())
                        .unwrap_or_else(|| settings.provider.name().to_owned()),
                    color: settings.accent.color(),
                    provider: settings.provider,
                    state: slot.state,
                }
            })
            .collect()
    }

    /// Seconds until something changes on its own.
    fn next_change_in(&self, now: i64) -> i64 {
        let minute = 60 - now.rem_euclid(60);
        let fetch = self
            .slots
            .iter()
            .filter(|s| s.bound && s.request.is_none())
            .map(|s| s.due.saturating_sub(now))
            .min()
            .unwrap_or(i64::MAX);
        let reset = if self.nav.reset_at == 0 {
            i64::MAX
        } else {
            self.nav.reset_at - now
        };
        minute.min(fetch).min(reset).max(1)
    }
}

fn bound_slots() -> [bool; MAX_CALENDARS] {
    if cfg!(any(
        feature = "preview-links",
        feature = "preview-errors",
        feature = "preview-failed",
        feature = "preview-big"
    )) {
        return [true; MAX_CALENDARS];
    }
    let bound = credentials::current();
    std::array::from_fn(|i| bound.is_bound(SLOT_KEYS[i]))
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    with_live(|_| ());
    request_frame();
}

#[unsafe(no_mangle)]
pub extern "C" fn render(_delta_ms: u32) {
    let _whole = profile::span("render");
    let now = SystemTime::now().unix_secs;
    let size = widget_size();
    let bound = bound_slots();
    let sys = system::current();
    let twelve_hour = sys.time_format() == Some(system::TimeFormat::Hour12);
    let first_weekday = sys
        .first_day_of_week()
        .map_or(0, |d| (d as u8).saturating_sub(1) % 7);
    let month_first = sys.date_format() == Some(system::DateFormat::MDYyyySlash);
    let night = sys.night_mode().unwrap_or(false);

    let busy = with_live(|live| {
        {
            let _span = profile::span("sync");
            live.sync(now, bound);
        }
        let busy = {
            let _span = profile::span("advance");
            live.advance(now)
        };
        // One heavy job a frame: rebuilding the schedule, or else keeping the events.
        if live.dirty {
            let _rebuild = profile::span("rebuild");
            live.rebuild(now);
        } else if live.unsaved && !busy {
            let _save = profile::span("save");
            live.save(now);
        }

        let calendars = live.calendars();
        let palette =
            if live.settings.look == Look::Dark || (live.settings.look == Look::ByNight && night) {
                Palette::dark()
            } else {
                Palette::light()
            };
        let offset = i64::from(Host.offset(&live.home, now).unwrap_or(0));
        let local = now + offset;
        let view = View {
            width: size.width,
            height: size.height,
            palette,
            schedule: &live.schedule,
            calendars: &calendars,
            guide: live.settings.calendars[0].provider,
            now,
            offset,
            today: live.nav.today,
            now_min: u16::try_from(local.rem_euclid(SECS_PER_DAY) / 60).unwrap_or(0),
            selected: live.nav.selected,
            month: live.nav.month,
            first_weekday,
            twelve_hour,
            dates: DateStyle { month_first },
        };
        let root = {
            let _span = profile::span("tree");
            #[cfg(feature = "preview-grid")]
            let root = view::preview_grid(&view);
            #[cfg(not(feature = "preview-grid"))]
            let root = view::widget(&view);
            root
        };
        let result = {
            let _span = profile::span("submit");
            render_ui(size.width, size.height, root)
        };

        let mut taps = Vec::new();
        for (key, hit) in &result.clicks {
            match key.as_str() {
                view::KEY_PREV => taps.push(Tap::Back),
                view::KEY_NEXT => taps.push(Tap::Forward),
                view::KEY_TODAY => taps.push(Tap::Today),
                view::KEY_CYCLE => taps.push(Tap::Cycle),
                view::KEY_GRID => taps.extend(view::grid_day(&view, hit).map(Tap::Pick)),
                view::KEY_WEEK => taps.extend(view::week_day(&view, hit).map(Tap::Pick)),
                _ => {}
            }
        }
        let browses = view::browses_months(size.width, size.height);
        let tapped = !taps.is_empty();
        for tap in taps {
            live.nav
                .apply(tap, now, browses, |from| live.schedule.next_busy_day(from));
        }
        if tapped {
            request_frame();
        }
        busy || tapped
    });

    if busy || cfg!(feature = "preview-spin") {
        request_frame();
    } else {
        let wait = with_live(|live| live.next_change_in(now));
        request_frame_after(u32::try_from(wait.saturating_mul(1_000)).unwrap_or(u32::MAX));
    }
}

fn on_reply(response: &FetchResponse) {
    let now = SystemTime::now().unix_secs;
    let Some(i) = PENDING.with(|p| p.borrow_mut().remove(&response.request_id)) else {
        return;
    };
    with_live(|live| {
        let (window, keep) = live.window();
        let slot = &mut live.slots[i];
        slot.request = None;
        if !slot.bound {
            // The link was taken away while the answer was on its way.
            return;
        }
        match response.outcome() {
            Some(FetchOutcome::Http(status)) if (200..300).contains(&status) => {
                let body = response.body();
                if ics::looks_like_calendar(body) {
                    slot.phase = Some(Phase::Scanning {
                        body: body.to_vec(),
                        scanner: Scanner::new(),
                        window,
                        keep,
                    });
                } else {
                    slot.failed(Fault::NotCalendar, now);
                }
            }
            Some(FetchOutcome::Http(status)) => slot.failed(Fault::Http(u32::from(status)), now),
            Some(FetchOutcome::Refused) => slot.failed(Fault::Refused, now),
            Some(FetchOutcome::BodyTooLarge) => slot.failed(Fault::TooLarge, now),
            Some(FetchOutcome::Aborted) => {}
            Some(FetchOutcome::Network) | None => slot.failed(Fault::Network, now),
        }
    });
    request_frame();
}

/// The host renders on touch only when asked; taps are read in `render`.
#[unsafe(no_mangle)]
pub extern "C" fn on_touch() {
    request_frame();
}

/// New settings apply from now. A shorter refresh time also brings the next
/// check forward, rather than waiting out the old one.
#[unsafe(no_mangle)]
pub extern "C" fn on_params_update() {
    with_live(|live| {
        live.settings = Settings::from_params(&Params::current());
        let soonest = SystemTime::now().unix_secs + i64::from(live.settings.refresh_minutes) * 60;
        for slot in live.slots.iter_mut().filter(|s| s.due != i64::MAX) {
            slot.due = slot.due.min(soonest);
        }
    });
    request_frame();
}

/// A link saved or removed shows up at once.
#[unsafe(no_mangle)]
pub extern "C" fn on_credentials_update() {
    request_frame();
}

/// The timezone, clock style or first day of the week may have changed.
#[unsafe(no_mangle)]
pub extern "C" fn on_system_update() {
    request_frame();
}
