//! The render tree. The Deck's tiles come in four shapes and each gets its own
//! arrangement of the same parts: a month grid, the list of a day and the
//! days after it, and, until a calendar is connected, the steps that connect one.

#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "view code uses the SDK's builders, colours and macros throughout"
    )
)]
use bmc_wasm_sdk::*;

use crate::civil::{self, Ymd};
use crate::model::{self, DateStyle, Event, Fault, MonthGrid, Schedule, SourceState};
use crate::providers::Provider;

/// The tap keys the widget listens for.
pub const KEY_PREV: &str = "prev";
pub const KEY_NEXT: &str = "next";
pub const KEY_TODAY: &str = "today";
/// The month grid; the tap's position says which day.
pub const KEY_GRID: &str = "grid";
/// The strip of a week's days; the tap's position says which day.
pub const KEY_WEEK: &str = "week";
/// The list; a tap moves on to the next day with events.
pub const KEY_CYCLE: &str = "cycle";

/// How far ahead the list of days looks for events.
const LOOK_AHEAD_DAYS: i64 = 14;
/// Text never shrinks below this to fit its box.
const MIN_TEXT: u16 = 10;

/// A small count as a length in pixels' units.
#[expect(
    clippy::cast_precision_loss,
    reason = "a handful of cells or calendars"
)]
fn count(n: usize) -> f32 {
    n as f32
}

/// The colours of one look.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color,
    pub panel: Color,
    pub line: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    /// Marks today.
    pub accent: Color,
    pub warn: Color,
    /// Days of the neighbouring months in the grid.
    pub outside: Color,
}

impl Palette {
    #[must_use]
    pub fn dark() -> Self {
        Self {
            bg: GRAY_100,
            panel: GRAY_90,
            line: GRAY_80,
            text: GRAY_10,
            dim: GRAY_40,
            faint: GRAY_60,
            accent: RED_50,
            warn: ORANGE_40,
            outside: Color::from_hex(0x10_10_10),
        }
    }

    #[must_use]
    pub fn light() -> Self {
        Self {
            bg: GRAY_10,
            panel: WHITE,
            line: GRAY_20,
            text: GRAY_100,
            dim: GRAY_60,
            faint: GRAY_40,
            accent: RED_60,
            warn: ORANGE_60,
            outside: Color::from_hex(0xEB_EB_EB),
        }
    }
}

/// A connected calendar, as the screen needs to know it.
#[derive(Clone, Debug)]
pub struct CalendarView {
    pub name: String,
    pub color: Color,
    pub provider: Provider,
    pub state: SourceState,
}

pub struct View<'a> {
    pub width: u32,
    pub height: u32,
    pub palette: Palette,
    pub schedule: &'a Schedule,
    /// The connected calendars; empty before any is.
    pub calendars: &'a [CalendarView],
    /// The service whose steps the screen shows while nothing is connected.
    pub guide: Provider,
    /// UTC seconds.
    pub now: i64,
    /// How far the Deck's clock is ahead of UTC, in seconds.
    pub offset: i64,
    /// Today, and the minute of it, on the Deck's calendar.
    pub today: i64,
    pub now_min: u16,
    /// The day the list starts from.
    pub selected: i64,
    /// The month the grid shows.
    pub month: (i64, u8),
    /// 0 for a week that starts on Monday up to 6 for Sunday.
    pub first_weekday: u8,
    pub twelve_hour: bool,
    pub dates: DateStyle,
}

/// What the tile is about to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    /// No calendar is connected.
    Guide,
    /// Connected, nothing back yet.
    Loading,
    /// Nothing could be read and nothing is kept from before.
    Failed,
    Events,
}

impl View<'_> {
    fn screen(&self) -> Screen {
        if self.calendars.is_empty() {
            Screen::Guide
        } else if !self.schedule.events().is_empty()
            || self
                .calendars
                .iter()
                .any(|c| matches!(c.state, SourceState::Ready { .. }))
        {
            Screen::Events
        } else if self
            .calendars
            .iter()
            .all(|c| matches!(c.state, SourceState::Failed(_)))
        {
            Screen::Failed
        } else {
            Screen::Loading
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "a viewport is at most a few thousand pixels"
    )]
    fn size(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }
}

/// The four arrangements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// Month and list side by side.
    Full,
    /// A week strip above the list.
    Tall,
    /// The day's date beside the list.
    Wide,
    /// A short list.
    Compact,
}

fn shape(width: u32, height: u32) -> Shape {
    match (width >= 900, height >= 380, width >= 520) {
        (true, true, _) => Shape::Full,
        (_, true, _) => Shape::Tall,
        (_, false, true) => Shape::Wide,
        _ => Shape::Compact,
    }
}

/// Whether a tile of this size shows a month grid that the arrows page through
/// (the others page by weeks).
#[must_use]
pub fn browses_months(width: u32, height: u32) -> bool {
    shape(width, height) == Shape::Full
}

#[must_use]
pub fn widget(view: &View<'_>) -> Node {
    let (w, h) = view.size();
    col(
        props!(width: w, height: h, background: view.palette.bg),
        [body(view, w, h, shape(view.width, view.height))],
    )
}

/// What the tile shows, laid out for a `w` by `h` tile of this shape.
fn body(view: &View<'_>, w: f32, h: f32, shape: Shape) -> Node {
    match view.screen() {
        Screen::Guide => guide(view, w, h),
        Screen::Loading => message(view, w, h, "Loading your calendar…", None),
        Screen::Failed => failed(view, w, h),
        Screen::Events => match shape {
            Shape::Full => full(view, w, h),
            Shape::Tall => tall(view, w, h),
            Shape::Wide => wide(view, w, h),
            Shape::Compact => compact(view, w, h),
        },
    }
}

// ── Drawing helpers ─────────────────────────────────────────────────

/// Collects the draw commands of one canvas.
#[derive(Default)]
struct Pen {
    draws: Vec<Draw>,
}

impl Pen {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.draws.push(Draw::rect(x, y, w, h, color));
    }

    fn dot(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.draws.push(Draw::circle(cx, cy, r, color));
    }

    /// Text in the box at `(x, y)` of `w` by `h`, centred vertically, shrinking
    /// to fit and cut short beyond that.
    #[expect(
        clippy::too_many_arguments,
        reason = "a text box is its position, size, text and look"
    )]
    fn text(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        size: u32,
        weight: FontWeight,
        color: Color,
        align: TextAlign,
    ) {
        if text.is_empty() || w < 4.0 || h < 4.0 {
            return;
        }
        #[expect(clippy::cast_precision_loss, reason = "font sizes are small")]
        let cut = model::fit(text, w, size as f32 * 0.82);
        let min = u16::try_from(size)
            .unwrap_or(u16::MAX)
            .clamp(1, MIN_TEXT.max(1));
        self.draws.push(Draw::autofit_text_ranged(
            x,
            y,
            w,
            h,
            cut,
            style!(size: size, weight: weight, color: color, align: align, valign: VerticalAlign::Center),
            AutoFit::Shrink,
            min,
            u16::try_from(size).unwrap_or(u16::MAX),
        ));
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a text box is its position, size, text and look"
    )]
    fn left(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        size: u32,
        weight: FontWeight,
        color: Color,
    ) {
        self.text(x, y, w, h, text, size, weight, color, TextAlign::Left);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a text box is its position, size, text and look"
    )]
    fn centred(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        size: u32,
        weight: FontWeight,
        color: Color,
    ) {
        self.text(x, y, w, h, text, size, weight, color, TextAlign::Center);
    }

    /// A line of `thickness` along a box edge.
    fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
        self.rect(x, y, w, thickness, color);
        self.rect(x, y + h - thickness, w, thickness, color);
        self.rect(x, y, thickness, h, color);
        self.rect(x + w - thickness, y, thickness, h, color);
    }

    fn finish(self, w: f32, h: f32) -> Node {
        canvas(props!(width: w, height: h), self.draws)
    }

    fn touch(self, key: &str, w: f32, h: f32) -> Node {
        touchable(key, props!(width: w, height: h), self.draws)
    }
}

/// A round button with a chevron (`left` or right) or a word.
fn round_button(p: &Palette, key: &str, size: f32, chevron_left: bool) -> Node {
    let mut pen = Pen::default();
    let r = size / 2.0;
    pen.dot(r, r, r, p.panel);
    let (reach, tip) = (size * 0.12, size * 0.22);
    let (x0, x1) = if chevron_left {
        (r + reach, r - reach)
    } else {
        (r - reach, r + reach)
    };
    pen.draws.push(Draw::path(
        vec![(x0, r - tip), (x1, r), (x0, r + tip)],
        2.5,
        p.text,
        false,
        Interpolation::Linear,
    ));
    pen.touch(key, size, size)
}

fn pill_button(p: &Palette, key: &str, label: &str, w: f32, h: f32) -> Node {
    let mut pen = Pen::default();
    let r = h / 2.0;
    pen.dot(r, r, r, p.panel);
    pen.dot(w - r, r, r, p.panel);
    pen.rect(r, 0.0, w - h, h, p.panel);
    pen.centred(0.0, 0.0, w, h, label, 15, FontWeight::SEMIBOLD, p.accent);
    pen.touch(key, w, h)
}

// ── Screens without events ──────────────────────────────────────────

/// A line or two, centred.
fn message(view: &View<'_>, w: f32, h: f32, headline: &str, detail: Option<&str>) -> Node {
    let p = &view.palette;
    let mut pen = Pen::default();
    let (big, small) = if w < 400.0 { (20, 14) } else { (28, 18) };
    #[expect(clippy::cast_precision_loss, reason = "font sizes are small")]
    let (big_h, small_h) = (big as f32 * 1.6, small as f32 * 1.5);
    let total = big_h + detail.map_or(0.0, |_| small_h * 3.0);
    let top = (h - total) / 2.0;
    pen.centred(
        16.0,
        top,
        w - 32.0,
        big_h,
        headline,
        big,
        FontWeight::BOLD,
        p.text,
    );
    if let Some(detail) = detail {
        pen.centred(
            24.0,
            top + big_h,
            w - 48.0,
            small_h * 3.0,
            detail,
            small,
            FontWeight::REGULAR,
            p.dim,
        );
    }
    pen.finish(w, h)
}

/// Why nothing could be read.
fn failed(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let broken: Vec<(&CalendarView, Fault)> = view
        .calendars
        .iter()
        .filter_map(|c| match c.state {
            SourceState::Failed(fault) => Some((c, fault)),
            _ => None,
        })
        .collect();
    let Some(&(first, fault)) = broken.first() else {
        return message(view, w, h, "Nothing to show yet", None);
    };
    let compact = w < 400.0;
    let (heading, body) = if compact { (20, 14) } else { (28, 18) };
    let mut lines = Vec::new();
    let title = if broken.len() == 1 {
        lines.push(fault.headline());
        if fault.is_the_link() {
            lines.push(first.provider.mistake_hint().to_owned());
        } else if fault == Fault::Network {
            lines.push(
                "Check the Deck's Wi-Fi, and that the link starts with https:// and not webcal://."
                    .to_owned(),
            );
        }
        if compact {
            fmt!("{}: can't read", first.name)
        } else {
            fmt!("Can't read {}", first.name)
        }
    } else {
        for (calendar, fault) in &broken {
            lines.push(fmt!("{}: {}", calendar.name, fault.headline()));
        }
        "Can't read your calendars".to_owned()
    };
    let mut children = vec![text(
        title,
        style!(size: heading, weight: FontWeight::BOLD, color: p.text),
    )];
    for line in lines {
        children.push(text(line, style!(size: body, color: p.dim)));
    }
    col(
        props!(width: w, height: h, padding: if compact { 16.0 } else { 32.0 }, gap: 10.0, justify_content: Justify::Center),
        children,
    )
}

/// The steps that connect a calendar.
fn guide(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let service = view.guide;
    if w < 400.0 {
        return col(
            props!(width: w, height: h, padding: 16.0, gap: 8.0, justify_content: Justify::Center),
            [
                text(
                    "Connect your calendar",
                    style!(size: 20, weight: FontWeight::BOLD, color: p.text),
                ),
                text(
                    fmt!(
                        "Open this widget's settings, pick {} and follow the steps.",
                        service.name()
                    ),
                    style!(size: 14, color: p.dim),
                ),
            ],
        );
    }
    let (pad, title, body, badge) = if h < 400.0 {
        (20.0, 24, 15, 24.0)
    } else {
        (36.0, 32, 19, 32.0)
    };
    let mut children = vec![
        text(
            "Connect your calendar",
            style!(size: title, weight: FontWeight::BOLD, color: p.text),
        ),
        text(
            fmt!("{}: a private link, in three steps", service.name()),
            style!(size: body, color: p.dim),
        ),
    ];
    for (i, step) in service.steps().iter().enumerate() {
        let mut pen = Pen::default();
        pen.dot(badge / 2.0, badge / 2.0, badge / 2.0, p.accent);
        pen.centred(
            0.0,
            0.0,
            badge,
            badge,
            &fmt!("{}", i + 1),
            body,
            FontWeight::BOLD,
            WHITE,
        );
        children.push(row(
            props!(gap: 12.0, cross_align: CrossAlign::Start),
            [
                pen.finish(badge, badge),
                text(*step, style!(size: body, color: p.text, flex: 1.0)),
            ],
        ));
    }
    children.push(text(
        "Then add the link under Connected Accounts as a Token, and pick that account for Calendar 1 link in this widget's settings.",
        style!(size: body - 2, color: p.dim),
    ));
    col(
        props!(width: w, height: h, padding: pad, gap: if h < 400.0 { 8.0 } else { 14.0 }),
        children,
    )
}

// ── The agenda: a day and the days after it ─────────────────────────

/// One line of the list.
enum Item<'a> {
    Heading(i64),
    /// A line of text standing in for events that aren't there.
    Note(&'static str),
    Event(&'a Event, i64),
}

/// Whether an event is over: it was today, and it ended before now.
fn has_ended(view: &View<'_>, event: &Event, day: i64) -> bool {
    day == view.today
        && !event.all_day
        && day == event.last_day
        && event.minutes_on(day).1 <= view.now_min
}

/// The list from `start`: that day's events (or a note that there are none),
/// then each later day that has events, under a heading of its own. The day
/// `start` needs none: the screen already names it. With `skip_ended`, today's
/// events that are over are left out, to make room for what is still to come.
fn items<'a>(view: &'a View<'_>, start: i64, skip_ended: bool) -> Vec<Item<'a>> {
    let mut out: Vec<Item<'a>> = view
        .schedule
        .on(start)
        .filter(|e| !(skip_ended && has_ended(view, e, start)))
        .map(|e| Item::Event(e, start))
        .collect();
    if out.is_empty() {
        out.push(Item::Note(if view.schedule.count_on(start) == 0 {
            "Nothing planned"
        } else {
            "Nothing more today"
        }));
    }
    let mut day = start + 1;
    while let Some(next) = view.schedule.next_busy_day(day) {
        if next > start + LOOK_AHEAD_DAYS || out.len() > 80 {
            break;
        }
        out.push(Item::Heading(next));
        out.extend(view.schedule.on(next).map(|e| Item::Event(e, next)));
        day = next + 1;
    }
    out
}

/// Sizes of one list style.
#[derive(Clone, Copy)]
struct Rows {
    heading: f32,
    event: f32,
    time_w: f32,
    title: u32,
    small: u32,
    head_size: u32,
    /// Whether a second line (place or calendar) is shown.
    second_line: bool,
}

impl Rows {
    const ROOMY: Self = Self {
        heading: 36.0,
        event: 52.0,
        time_w: 76.0,
        title: 19,
        small: 14,
        head_size: 16,
        second_line: true,
    };
    const SNUG: Self = Self {
        heading: 30.0,
        event: 44.0,
        time_w: 66.0,
        title: 17,
        small: 13,
        head_size: 14,
        second_line: true,
    };
    const TIGHT: Self = Self {
        heading: 26.0,
        event: 36.0,
        time_w: 52.0,
        title: 15,
        small: 12,
        head_size: 13,
        second_line: false,
    };

    fn height_of(self, item: &Item<'_>) -> f32 {
        match item {
            Item::Heading(_) => self.heading,
            Item::Note(_) | Item::Event(..) => self.event,
        }
    }
}

/// The list drawn into a `w` by `h` canvas; touchable under `tap` when given.
fn agenda(view: &View<'_>, w: f32, h: f32, rows: Rows, tap: Option<&str>) -> Node {
    let mut list = items(view, view.selected, false);
    if view.selected == view.today && list.iter().map(|i| rows.height_of(i)).sum::<f32>() > h {
        list = items(view, view.selected, true);
    }
    let fits = |room: f32| {
        let mut used = 0.0;
        let mut count = 0;
        for item in &list {
            used += rows.height_of(item);
            if used > room {
                break;
            }
            count += 1;
        }
        count
    };
    let mut shown = fits(h);
    let mut hidden = list[shown..]
        .iter()
        .filter(|i| matches!(i, Item::Event(..)))
        .count();
    if hidden > 0 {
        // Room for the "more" line; a heading left last on its own goes too.
        shown = fits(h - 24.0);
        while shown > 1 && matches!(list[shown - 1], Item::Heading(_)) {
            shown -= 1;
        }
        hidden = list[shown..]
            .iter()
            .filter(|i| matches!(i, Item::Event(..)))
            .count();
    }

    let mut pen = Pen::default();
    let mut y = 0.0;
    for item in &list[..shown] {
        match item {
            Item::Heading(day) => heading(&mut pen, view, rows, *day, y, w),
            Item::Note(note) => {
                let p = &view.palette;
                pen.left(
                    0.0,
                    y,
                    w,
                    rows.event,
                    note,
                    rows.title,
                    FontWeight::REGULAR,
                    p.faint,
                );
            }
            Item::Event(event, day) => event_row(&mut pen, view, rows, event, *day, y, w),
        }
        y += rows.height_of(item);
    }
    if hidden > 0 {
        let p = &view.palette;
        let more = if hidden == 1 {
            "1 more event".to_owned()
        } else {
            fmt!("{} more events", hidden)
        };
        pen.left(
            0.0,
            h - 22.0,
            w,
            22.0,
            &more,
            rows.small,
            FontWeight::SEMIBOLD,
            p.faint,
        );
    }
    match tap {
        Some(key) => pen.touch(key, w, h),
        None => pen.finish(w, h),
    }
}

fn heading(pen: &mut Pen, view: &View<'_>, rows: Rows, day: i64, y: f32, w: f32) {
    let p = &view.palette;
    let label = view.dates.heading(day, view.today);
    let colour = if day == view.today { p.accent } else { p.dim };
    pen.left(
        0.0,
        y + 2.0,
        w,
        rows.heading - 6.0,
        &label.to_uppercase(),
        rows.head_size,
        FontWeight::BOLD,
        colour,
    );
    pen.rect(0.0, y + rows.heading - 3.0, w, 1.0, p.line);
}

fn event_row(pen: &mut Pen, view: &View<'_>, rows: Rows, event: &Event, day: i64, y: f32, w: f32) {
    let p = &view.palette;
    let colour = view
        .calendars
        .get(usize::from(event.source))
        .map_or(p.dim, |c| c.color);
    let ended = has_ended(view, event, day);
    let on_now = event.is_on_now(view.now);
    let ink = if ended { p.faint } else { p.text };
    if on_now {
        pen.rect(0.0, y + 1.0, w, rows.event - 2.0, colour.with_alpha(0.16));
    }
    let time_ink = if on_now { colour } else { ink };
    event_time(pen, view, rows, event, day, y, time_ink);

    let bar = if ended {
        colour.with_alpha(0.4)
    } else {
        colour
    };
    pen.rect(rows.time_w + 2.0, y + 6.0, 4.0, rows.event - 12.0, bar);
    let text_x = rows.time_w + 16.0;
    let text_w = w - text_x - 4.0;
    let line = rows.event;
    if rows.second_line {
        let second = second_line(view, event);
        let title = &event.title;
        let weight = FontWeight::SEMIBOLD;
        pen.left(
            text_x,
            y + 4.0,
            text_w,
            line * 0.55,
            title,
            rows.title,
            weight,
            ink,
        );
        let (y2, h2) = (y + line * 0.52, line * 0.4);
        pen.left(
            text_x,
            y2,
            text_w,
            h2,
            &second,
            rows.small,
            FontWeight::REGULAR,
            p.dim,
        );
    } else {
        let (title, size, weight) = (&event.title, rows.title, FontWeight::SEMIBOLD);
        pen.left(text_x, y, text_w, line, title, size, weight, ink);
    }
}

/// The time column of an event's row: when it starts and, beneath it, when it ends.
fn event_time(
    pen: &mut Pen,
    view: &View<'_>,
    rows: Rows,
    event: &Event,
    day: i64,
    y: f32,
    ink: Color,
) {
    let p = &view.palette;
    let (when, until) = model::when(event, day, view.twelve_hour);
    let (pad, line) = (6.0, rows.event);
    let width = rows.time_w - pad;
    if let (true, Some(until)) = (rows.second_line, until) {
        let (size, weight) = (rows.title - 3, FontWeight::BOLD);
        pen.left(
            pad,
            y + 5.0,
            width,
            line * 0.5 - 4.0,
            &when,
            size,
            weight,
            ink,
        );
        let (size, weight) = (rows.small - 1, FontWeight::REGULAR);
        pen.left(
            pad,
            y + line * 0.5,
            width,
            line * 0.4,
            &until,
            size,
            weight,
            p.dim,
        );
    } else if when == "All day" {
        pen.left(
            pad,
            y,
            width,
            line,
            &when,
            rows.small,
            FontWeight::REGULAR,
            p.dim,
        );
    } else {
        pen.left(
            pad,
            y,
            width,
            line,
            &when,
            rows.title - 3,
            FontWeight::BOLD,
            ink,
        );
    }
}

/// Beneath a title: where it is, or which calendar it is in.
fn second_line(view: &View<'_>, event: &Event) -> String {
    if !event.place.is_empty() {
        return event.place.clone();
    }
    if view.calendars.len() > 1 {
        return view
            .calendars
            .get(usize::from(event.source))
            .map_or_else(String::new, |c| c.name.clone());
    }
    String::new()
}

// ── Month grid ──────────────────────────────────────────────────────

/// Where the parts of the full tile go.
#[derive(Clone, Copy)]
struct FullGeo {
    pad: f32,
    gap: f32,
    left_w: f32,
    right_w: f32,
    inner_h: f32,
    bar_h: f32,
    names_h: f32,
    grid_h: f32,
    weeks: usize,
}

fn full_geo(view: &View<'_>, w: f32, h: f32) -> FullGeo {
    let (pad, gap) = (16.0, 18.0);
    let left_w = ((w - 2.0 * pad - gap) * 0.55).floor();
    let inner_h = h - 2.0 * pad;
    let (bar_h, names_h) = (46.0, 24.0);
    FullGeo {
        pad,
        gap,
        left_w,
        right_w: w - 2.0 * pad - gap - left_w,
        inner_h,
        bar_h,
        names_h,
        grid_h: inner_h - bar_h - names_h,
        weeks: MonthGrid::new(view.month.0, view.month.1, view.first_weekday)
            .weeks
            .len(),
    }
}

/// The day under a tap on the month grid.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a cell index from a tap inside the grid"
)]
pub fn grid_day(view: &View<'_>, hit: &TouchHit) -> Option<i64> {
    let (w, h) = view.size();
    let geo = full_geo(view, w, h);
    let grid = MonthGrid::new(view.month.0, view.month.1, view.first_weekday);
    let (cell_w, cell_h) = (geo.left_w / 7.0, geo.grid_h / count(geo.weeks));
    if hit.x < 0.0 || hit.y < 0.0 {
        return None;
    }
    let (col, row) = ((hit.x / cell_w) as usize, (hit.y / cell_h) as usize);
    grid.weeks.get(row)?.get(col).copied()
}

/// The day under a tap on the week strip.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "a cell index from a tap inside the strip"
)]
pub fn week_day(view: &View<'_>, hit: &TouchHit) -> Option<i64> {
    let (w, _) = view.size();
    let cell_w = (w - 32.0) / 7.0;
    if hit.x < 0.0 {
        return None;
    }
    let col = (hit.x / cell_w) as i64;
    (0..7).contains(&col).then(|| week_start(view) + col)
}

/// The first day of the week that holds the selected day.
fn week_start(view: &View<'_>) -> i64 {
    let back = (civil::weekday(view.selected) + 7 - view.first_weekday % 7) % 7;
    view.selected - i64::from(back)
}

fn full(view: &View<'_>, w: f32, h: f32) -> Node {
    let geo = full_geo(view, w, h);
    let left = col(
        props!(width: geo.left_w, height: geo.inner_h),
        [
            month_bar(view, &geo),
            weekday_names(view, &geo),
            month_grid(view, &geo),
        ],
    );
    let right = agenda_pane(view, geo.right_w, geo.inner_h);
    row(
        props!(width: w, height: h, padding: geo.pad, gap: geo.gap),
        [left, right],
    )
}

fn month_bar(view: &View<'_>, geo: &FullGeo) -> Node {
    let p = &view.palette;
    let (year, month) = view.month;
    let mut title = Pen::default();
    let name = fmt!("{} {}", civil::long_month(month), year);
    title.left(
        0.0,
        0.0,
        geo.left_w,
        geo.bar_h,
        &name,
        28,
        FontWeight::BOLD,
        p.text,
    );
    let away = view.month != current_month(view) || view.selected != view.today;
    let btn = 38.0;
    let mut children = vec![title.finish(
        geo.left_w - 2.0 * btn - 8.0 - if away { 84.0 } else { 0.0 },
        geo.bar_h,
    )];
    if away {
        children.push(pill_button(p, KEY_TODAY, "Today", 76.0, 34.0));
        children.push(canvas(props!(width: 8.0, height: 1.0), []));
    }
    children.push(round_button(p, KEY_PREV, btn, true));
    children.push(round_button(p, KEY_NEXT, btn, false));
    row(
        props!(width: geo.left_w, height: geo.bar_h, gap: 4.0, cross_align: CrossAlign::Center),
        children,
    )
}

fn current_month(view: &View<'_>) -> (i64, u8) {
    let today = Ymd::from_days(view.today);
    (today.year, today.month)
}

fn weekday_names(view: &View<'_>, geo: &FullGeo) -> Node {
    let p = &view.palette;
    let cell_w = geo.left_w / 7.0;
    let mut pen = Pen::default();
    for i in 0..7_u8 {
        let weekday = (view.first_weekday + i) % 7;
        pen.centred(
            f32::from(i) * cell_w,
            0.0,
            cell_w,
            geo.names_h,
            &civil::short_weekday(weekday).to_uppercase(),
            12,
            FontWeight::BOLD,
            p.faint,
        );
    }
    pen.finish(geo.left_w, geo.names_h)
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a handful of weeks and lines"
)]
fn month_grid(view: &View<'_>, geo: &FullGeo) -> Node {
    let grid = MonthGrid::new(view.month.0, view.month.1, view.first_weekday);
    let (cell_w, cell_h) = (geo.left_w / 7.0, geo.grid_h / count(geo.weeks));
    let lines = ((cell_h - 26.0) / 15.0).floor().clamp(0.0, 4.0) as usize;
    let mut pen = Pen::default();
    for (r, week) in grid.weeks.iter().enumerate() {
        for (c, &day) in week.iter().enumerate() {
            let cell = CellBox {
                x: c as f32 * cell_w,
                y: r as f32 * cell_h,
                w: cell_w,
                h: cell_h,
            };
            draw_cell(&mut pen, view, &grid, day, &cell, lines);
        }
    }
    pen.touch(KEY_GRID, geo.left_w, geo.grid_h)
}

/// Where a cell of the month grid is.
struct CellBox {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// One day of the month grid: its number, a mark if it is today or picked,
/// and its first few events.
fn draw_cell(
    pen: &mut Pen,
    view: &View<'_>,
    grid: &MonthGrid,
    day: i64,
    cell: &CellBox,
    lines: usize,
) {
    let p = &view.palette;
    let CellBox { x, y, w, h } = *cell;
    let inside = grid.holds(day);
    pen.rect(
        x + 0.5,
        y + 0.5,
        w - 1.0,
        h - 1.0,
        if inside { p.panel } else { p.outside },
    );
    if day == view.selected {
        pen.outline(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 2.0, p.accent);
    }
    let number = fmt!("{}", Ymd::from_days(day).day);
    if day == view.today {
        pen.dot(x + 19.0, y + 17.0, 12.0, p.accent);
        pen.centred(
            x + 7.0,
            y + 5.0,
            24.0,
            24.0,
            &number,
            14,
            FontWeight::BOLD,
            WHITE,
        );
    } else {
        let shade = if inside { p.text } else { p.faint };
        pen.centred(
            x + 7.0,
            y + 5.0,
            24.0,
            24.0,
            &number,
            14,
            FontWeight::SEMIBOLD,
            shade,
        );
    }
    cell_events(pen, view, day, x, y + 28.0, w, lines);
}

/// The first few events of a day as small bars of text, and how many more there are.
#[expect(clippy::cast_precision_loss, reason = "a handful of lines")]
fn cell_events(pen: &mut Pen, view: &View<'_>, day: i64, x: f32, y: f32, w: f32, lines: usize) {
    let p = &view.palette;
    let total = view.schedule.count_on(day);
    if lines == 0 || total == 0 {
        return;
    }
    let shown = if total > lines { lines - 1 } else { total };
    for (i, event) in view.schedule.on(day).take(shown).enumerate() {
        let colour = view
            .calendars
            .get(usize::from(event.source))
            .map_or(p.dim, |c| c.color);
        let top = y + i as f32 * 15.0;
        if event.all_day {
            pen.rect(x + 4.0, top, w - 8.0, 14.0, colour.with_alpha(0.30));
        }
        pen.rect(x + 4.0, top, 3.0, 14.0, colour);
        pen.left(
            x + 10.0,
            top,
            w - 14.0,
            14.0,
            &event.title,
            11,
            FontWeight::SEMIBOLD,
            p.text,
        );
    }
    if total > shown {
        pen.left(
            x + 6.0,
            y + shown as f32 * 15.0,
            w - 10.0,
            14.0,
            &fmt!("+{} more", total - shown),
            11,
            FontWeight::SEMIBOLD,
            p.dim,
        );
    }
}

// ── Panes ───────────────────────────────────────────────────────────

/// The selected day's name and date, the calendars' key, and the list.
fn agenda_pane(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let head_h = 58.0;
    let foot_h = 20.0;
    let mut head = Pen::default();
    let name = view.dates.heading(view.selected, view.today);
    head.left(0.0, 0.0, w, 34.0, &name, 28, FontWeight::BOLD, p.text);
    head.left(
        0.0,
        34.0,
        w,
        22.0,
        &view.dates.long(view.selected),
        15,
        FontWeight::REGULAR,
        p.dim,
    );
    legend(&mut head, view, w);

    let mut foot = Pen::default();
    status_line(&mut foot, view, w, foot_h);

    col(
        props!(width: w, height: h),
        [
            head.finish(w, head_h),
            agenda(
                view,
                w,
                h - head_h - foot_h - 6.0,
                Rows::ROOMY,
                Some(KEY_CYCLE),
            ),
            foot.finish(w, foot_h),
        ],
    )
}

/// Coloured dots with the calendars' names, at the right of a header.
fn legend(pen: &mut Pen, view: &View<'_>, w: f32) {
    if view.calendars.len() < 2 {
        return;
    }
    let p = &view.palette;
    let slot = 112.0;
    let mut x = w - slot * count(view.calendars.len());
    for calendar in view.calendars {
        pen.dot(x + 6.0, 17.0, 5.0, calendar.color);
        pen.left(
            x + 16.0,
            8.0,
            slot - 22.0,
            18.0,
            &calendar.name,
            13,
            FontWeight::REGULAR,
            p.dim,
        );
        x += slot;
    }
}

/// Whether some calendar couldn't be read at the last try.
fn has_problem(view: &View<'_>) -> bool {
    first_problem(view).is_some()
}

/// The calendars' key, small, at the left of a footer (not while a problem is reported there).
fn footer_legend(pen: &mut Pen, view: &View<'_>, h: f32) {
    if view.calendars.len() < 2 || has_problem(view) {
        return;
    }
    let p = &view.palette;
    let slot = 104.0;
    for (i, calendar) in view.calendars.iter().enumerate() {
        let x = slot * f32::from(u8::try_from(i).unwrap_or(0));
        pen.dot(x + 5.0, h / 2.0, 4.0, calendar.color);
        pen.left(
            x + 14.0,
            0.0,
            slot - 18.0,
            h,
            &calendar.name,
            12,
            FontWeight::REGULAR,
            p.dim,
        );
    }
}

/// The first calendar that couldn't be read, and why.
fn first_problem<'a>(view: &'a View<'_>) -> Option<(&'a CalendarView, Fault)> {
    view.calendars.iter().find_map(|c| match c.state {
        SourceState::Failed(fault) => Some((c, fault)),
        _ => None,
    })
}

/// A warning about the first calendar that couldn't be read, `short` for a
/// narrow space. Draws nothing, and says so, when there is no problem.
fn problem_line(
    pen: &mut Pen,
    view: &View<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    short: bool,
) -> bool {
    let palette = &view.palette;
    let Some((calendar, fault)) = first_problem(view) else {
        return false;
    };
    let note = match (short, fault.is_the_link()) {
        (true, _) => "Can't update".to_owned(),
        (false, true) => fmt!("{} needs a new link", calendar.name),
        (false, false) => fmt!("{} couldn't be reached", calendar.name),
    };
    pen.dot(x + 5.0, y + h / 2.0, 4.0, palette.warn);
    pen.left(
        x + 16.0,
        y,
        w - 16.0,
        h,
        &note,
        13,
        FontWeight::SEMIBOLD,
        palette.warn,
    );
    true
}

/// The footer: a problem with a calendar, else when the events were last read.
fn status_line(pen: &mut Pen, view: &View<'_>, w: f32, h: f32) {
    if problem_line(pen, view, 0.0, 0.0, w, h, false) {
        return;
    }
    let p = &view.palette;
    let newest = view
        .calendars
        .iter()
        .filter_map(|c| match c.state {
            SourceState::Ready { at } => Some(at),
            _ => None,
        })
        .max();
    if let Some(at) = newest {
        let local = at + view.offset;
        let minute = u16::try_from(local.rem_euclid(86_400) / 60).unwrap_or(0);
        pen.text(
            0.0,
            0.0,
            w,
            h,
            &fmt!("Updated {}", model::clock(minute, view.twelve_hour)),
            12,
            FontWeight::REGULAR,
            p.faint,
            TextAlign::Right,
        );
    }
}

fn tall(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let pad = 16.0;
    let inner_w = w - 2.0 * pad;
    let head_h = 56.0;
    let strip_h = 64.0;
    let foot_h = 20.0;

    let name = view.dates.heading(view.selected, view.today);
    let mut head_text = Pen::default();
    head_text.left(0.0, 0.0, inner_w, 34.0, &name, 26, FontWeight::BOLD, p.text);
    head_text.left(
        0.0,
        33.0,
        inner_w,
        22.0,
        &view.dates.long(view.selected),
        15,
        FontWeight::REGULAR,
        p.dim,
    );
    let btn = 36.0;
    let away = view.selected != view.today;
    let mut head_children = vec![head_text.finish(
        inner_w - 2.0 * btn - 8.0 - if away { 84.0 } else { 0.0 },
        head_h,
    )];
    if away {
        head_children.push(pill_button(p, KEY_TODAY, "Today", 76.0, 34.0));
        head_children.push(canvas(props!(width: 8.0, height: 1.0), []));
    }
    head_children.push(round_button(p, KEY_PREV, btn, true));
    head_children.push(round_button(p, KEY_NEXT, btn, false));
    let head = row(
        props!(width: inner_w, height: head_h, gap: 4.0, cross_align: CrossAlign::Center),
        head_children,
    );

    let mut foot = Pen::default();
    footer_legend(&mut foot, view, foot_h);
    status_line(&mut foot, view, inner_w, foot_h);

    col(
        props!(width: w, height: h, padding: pad, gap: 6.0),
        [
            head,
            week_strip(view, inner_w, strip_h),
            agenda(
                view,
                inner_w,
                h - 2.0 * pad - head_h - strip_h - foot_h - 18.0,
                Rows::SNUG,
                None,
            ),
            foot.finish(inner_w, foot_h),
        ],
    )
}

/// Seven days with a dot for each calendar that has something on.
#[expect(clippy::cast_precision_loss, reason = "seven cells")]
fn week_strip(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let start = week_start(view);
    let cell_w = w / 7.0;
    let mut pen = Pen::default();
    for i in 0..7_i64 {
        let day = start + i;
        let x = i as f32 * cell_w;
        let weekday = civil::weekday(day);
        pen.centred(
            x,
            2.0,
            cell_w,
            16.0,
            &civil::short_weekday(weekday).to_uppercase(),
            11,
            FontWeight::BOLD,
            p.faint,
        );
        let (cx, cy) = (x + cell_w / 2.0, 36.0);
        let number = fmt!("{}", Ymd::from_days(day).day);
        if day == view.today {
            pen.dot(cx, cy, 15.0, p.accent);
            pen.centred(
                cx - 15.0,
                cy - 15.0,
                30.0,
                30.0,
                &number,
                17,
                FontWeight::BOLD,
                WHITE,
            );
        } else {
            if day == view.selected {
                pen.dot(cx, cy, 15.0, p.panel);
                pen.dot(cx, cy, 13.0, p.bg);
            }
            pen.centred(
                cx - 15.0,
                cy - 15.0,
                30.0,
                30.0,
                &number,
                17,
                FontWeight::SEMIBOLD,
                p.text,
            );
        }
        let mut seen: Vec<u8> = view.schedule.on(day).map(|e| e.source).collect();
        seen.sort_unstable();
        seen.dedup();
        let count = seen.len().min(3);
        for (n, source) in seen.iter().take(3).enumerate() {
            let colour = view
                .calendars
                .get(usize::from(*source))
                .map_or(p.dim, |c| c.color);
            pen.dot(
                cx + (n as f32 - (count as f32 - 1.0) / 2.0) * 9.0,
                h - 8.0,
                3.0,
                colour,
            );
        }
    }
    pen.touch(KEY_WEEK, w, h)
}

/// The date of the selected day, big, beside the list.
fn wide(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let pad = 14.0;
    let side_w = 150.0;
    let gap = 14.0;
    let date = Ymd::from_days(view.selected);
    let weekday = civil::long_weekday(civil::weekday(view.selected));
    let count = view.schedule.count_on(view.selected);

    let mut side = Pen::default();
    let colour = if view.selected == view.today {
        p.accent
    } else {
        p.dim
    };
    side.left(
        0.0,
        0.0,
        side_w,
        24.0,
        &weekday.to_uppercase(),
        15,
        FontWeight::BOLD,
        colour,
    );
    side.left(
        0.0,
        22.0,
        side_w,
        84.0,
        &fmt!("{}", date.day),
        76,
        FontWeight::BOLD,
        p.text,
    );
    side.left(
        0.0,
        104.0,
        side_w,
        28.0,
        &format!("{} {}", civil::long_month(date.month), date.year),
        17,
        FontWeight::SEMIBOLD,
        p.dim,
    );
    let busy = match count {
        0 => "Nothing planned".to_owned(),
        1 => "1 event".to_owned(),
        n => fmt!("{} events", n),
    };
    side.left(
        0.0,
        134.0,
        side_w,
        22.0,
        &busy,
        14,
        FontWeight::REGULAR,
        p.faint,
    );
    let side_h = h - 2.0 * pad;
    problem_line(&mut side, view, 0.0, side_h - 22.0, side_w, 22.0, true);
    let list_w = w - 2.0 * pad - side_w - gap;
    row(
        props!(width: w, height: h, padding: pad, gap: gap),
        [
            side.touch(KEY_CYCLE, side_w, side_h),
            agenda(view, list_w, side_h, Rows::SNUG, Some(KEY_CYCLE)),
        ],
    )
}

fn compact(view: &View<'_>, w: f32, h: f32) -> Node {
    let p = &view.palette;
    let pad = 12.0;
    let inner_w = w - 2.0 * pad;
    let mut head = Pen::default();
    let name = view.dates.heading(view.selected, view.today);
    let colour = if view.selected == view.today {
        p.accent
    } else {
        p.text
    };
    head.left(0.0, 0.0, inner_w, 28.0, &name, 22, FontWeight::BOLD, colour);
    if has_problem(view) {
        head.dot(inner_w - 5.0, 14.0, 4.0, p.warn);
    }
    col(
        props!(width: w, height: h, padding: pad, gap: 4.0),
        [
            head.finish(inner_w, 30.0),
            agenda(
                view,
                inner_w,
                h - 2.0 * pad - 34.0,
                Rows::TIGHT,
                Some(KEY_CYCLE),
            ),
        ],
    )
}

/// Development only: the three smaller layouts side by side, drawn inside the
/// full tile so the desktop testbed shows them whole. The last cell shows what
/// a small tile says while nothing is connected.
#[cfg(feature = "preview-grid")]
#[must_use]
pub fn preview_grid(view: &View<'_>) -> Node {
    let framed = |w: f32, h: f32, inner: Node| {
        col(
            props!(width: w, height: h, background: view.palette.bg),
            [inner],
        )
    };
    let right = col(
        props!(width: 638.0, height: 480.0, gap: 4.0),
        [
            framed(638.0, 238.0, body(view, 638.0, 238.0, Shape::Wide)),
            row(
                props!(gap: 4.0),
                [
                    framed(317.0, 238.0, body(view, 317.0, 238.0, Shape::Compact)),
                    framed(
                        317.0,
                        238.0,
                        guide(
                            &View {
                                calendars: &[],
                                ..*view
                            },
                            317.0,
                            238.0,
                        ),
                    ),
                ],
            ),
        ],
    );
    row(
        props!(width: 1280.0, height: 480.0, gap: 4.0, background: Color::from_hex(0x00_00_00)),
        [
            framed(638.0, 480.0, body(view, 638.0, 480.0, Shape::Tall)),
            right,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::Occurrence;
    use crate::model::Event;
    use crate::zones::testing::HostLike;

    const ROME: &str = "Europe/Rome";

    fn day(year: i64, month: u8, day: u8) -> i64 {
        Ymd { year, month, day }.days()
    }

    fn event(title: &str, d: i64, hour: i64, minutes: i64, source: u8) -> Event {
        let start = d * 86_400 + hour * 3_600 - 7_200;
        Event::place(
            &Occurrence {
                title: title.into(),
                place: "Somewhere".into(),
                start,
                end: start + minutes * 60,
                all_day: false,
                free: false,
            },
            source,
            &HostLike,
            ROME,
        )
    }

    fn calendars() -> Vec<CalendarView> {
        vec![
            CalendarView {
                name: "Home".into(),
                color: BLUE_50,
                provider: Provider::Apple,
                state: SourceState::Ready { at: 1_000 },
            },
            CalendarView {
                name: "Work".into(),
                color: ORANGE_40,
                provider: Provider::Outlook,
                state: SourceState::Ready { at: 1_000 },
            },
        ]
    }

    fn build(width: u32, height: u32, calendars: &[CalendarView], schedule: &Schedule) -> Node {
        let today = day(2026, 10, 15);
        widget(&View {
            width,
            height,
            palette: Palette::dark(),
            schedule,
            calendars,
            guide: Provider::Google,
            now: today * 86_400 + 9 * 3_600,
            offset: 7_200,
            today,
            now_min: 11 * 60,
            selected: today,
            month: (2026, 10),
            first_weekday: 0,
            twelve_hour: false,
            dates: DateStyle { month_first: false },
        })
    }

    #[test]
    fn every_tile_size_builds_in_every_state() {
        let today = day(2026, 10, 15);
        let busy = Schedule::new(
            (0..40)
                .map(|n| {
                    event(
                        &format!("Event number {n} with a rather long title"),
                        today + n % 9,
                        8 + n % 10,
                        45,
                        u8::try_from(n % 2).unwrap_or(0),
                    )
                })
                .collect(),
        );
        let empty = Schedule::default();
        let broken = vec![CalendarView {
            name: "Home".into(),
            color: BLUE_50,
            provider: Provider::Apple,
            state: SourceState::Failed(Fault::Http(404)),
        }];
        let waiting = vec![CalendarView {
            name: "Home".into(),
            color: BLUE_50,
            provider: Provider::Apple,
            state: SourceState::Waiting,
        }];
        for (w, h) in [
            (317, 238),
            (638, 238),
            (638, 480),
            (1280, 480),
            (400, 300),
            (1000, 300),
        ] {
            for (cals, schedule) in [
                (calendars(), &busy),
                (calendars(), &empty),
                (Vec::new(), &empty),
                (broken.clone(), &empty),
                (waiting.clone(), &empty),
            ] {
                let _ = build(w, h, &cals, schedule);
            }
        }
    }

    #[test]
    #[expect(clippy::cast_precision_loss, reason = "a handful of weeks")]
    fn a_tap_on_the_grid_finds_its_day() {
        let today = day(2026, 10, 15);
        let schedule = Schedule::default();
        let cals = calendars();
        let view = View {
            width: 1280,
            height: 480,
            palette: Palette::dark(),
            schedule: &schedule,
            calendars: &cals,
            guide: Provider::Apple,
            now: today * 86_400,
            offset: 7_200,
            today,
            now_min: 0,
            selected: today,
            month: (2026, 10),
            first_weekday: 0,
            twelve_hour: false,
            dates: DateStyle { month_first: false },
        };
        let geo = full_geo(&view, 1280.0, 480.0);
        let (cell_w, cell_h) = (geo.left_w / 7.0, geo.grid_h / geo.weeks as f32);
        let hit = |col: f32, row: f32| TouchHit {
            x: (col + 0.5) * cell_w,
            y: (row + 0.5) * cell_h,
            width: geo.left_w,
            height: geo.grid_h,
        };
        // October 2026 starts on a Thursday: the first row is 28 Sep to 4 Oct.
        assert_eq!(grid_day(&view, &hit(0.0, 0.0)), Some(day(2026, 9, 28)));
        assert_eq!(grid_day(&view, &hit(3.0, 0.0)), Some(day(2026, 10, 1)));
        assert_eq!(grid_day(&view, &hit(3.0, 2.0)), Some(day(2026, 10, 15)));
        assert_eq!(grid_day(&view, &hit(6.0, 99.0)), None);
        assert_eq!(
            week_day(
                &view,
                &TouchHit {
                    x: 1.0,
                    y: 5.0,
                    width: 600.0,
                    height: 60.0
                }
            ),
            Some(day(2026, 10, 12))
        );
    }

    #[test]
    fn tiles_are_told_apart_by_size() {
        assert_eq!(shape(1280, 480), Shape::Full);
        assert_eq!(shape(638, 480), Shape::Tall);
        assert_eq!(shape(638, 238), Shape::Wide);
        assert_eq!(shape(317, 238), Shape::Compact);
    }
}
