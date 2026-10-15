// AUTO-GENERATED FROM ../manifest.json by `bmc-widget-codegen` v0.1.0.
// Do not edit by hand. Run `just wasm::gen <widget>` after changing the manifest.

#![allow(
    dead_code,
    reason = "fields are widget-specific; not every key is used by every render path"
)]

use bmc_wasm_sdk::params as snapshot;
use bmc_wasm_sdk::params::typed::ParamRead;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar1Color {
    Blue,
    Orange,
    Green,
    Red,
    Purple,
    Teal,
    Pink,
    Gray,
}
impl Calendar1Color {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Blue,
        Self::Orange,
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Teal,
        Self::Pink,
        Self::Gray,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Orange => "orange",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
            Self::Teal => "teal",
            Self::Pink => "pink",
            Self::Gray => "gray",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Orange => "Orange",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Teal => "Teal",
            Self::Pink => "Pink",
            Self::Gray => "Gray",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "blue" => Some(Self::Blue),
            "orange" => Some(Self::Orange),
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            "purple" => Some(Self::Purple),
            "teal" => Some(Self::Teal),
            "pink" => Some(Self::Pink),
            "gray" => Some(Self::Gray),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar1Color);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar1Provider {
    Apple,
    Google,
    Outlook,
    Proton,
    Fastmail,
    Nextcloud,
    Other,
}
impl Calendar1Provider {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Apple,
        Self::Google,
        Self::Outlook,
        Self::Proton,
        Self::Fastmail,
        Self::Nextcloud,
        Self::Other,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
            Self::Outlook => "outlook",
            Self::Proton => "proton",
            Self::Fastmail => "fastmail",
            Self::Nextcloud => "nextcloud",
            Self::Other => "other",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Apple => "Apple Calendar (iCloud)",
            Self::Google => "Google Calendar",
            Self::Outlook => "Outlook (Outlook.com or Microsoft 365)",
            Self::Proton => "Proton Calendar",
            Self::Fastmail => "Fastmail",
            Self::Nextcloud => "Nextcloud",
            Self::Other => "Other (any iCalendar link)",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "apple" => Some(Self::Apple),
            "google" => Some(Self::Google),
            "outlook" => Some(Self::Outlook),
            "proton" => Some(Self::Proton),
            "fastmail" => Some(Self::Fastmail),
            "nextcloud" => Some(Self::Nextcloud),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar1Provider);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar2Color {
    Blue,
    Orange,
    Green,
    Red,
    Purple,
    Teal,
    Pink,
    Gray,
}
impl Calendar2Color {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Blue,
        Self::Orange,
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Teal,
        Self::Pink,
        Self::Gray,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Orange => "orange",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
            Self::Teal => "teal",
            Self::Pink => "pink",
            Self::Gray => "gray",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Orange => "Orange",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Teal => "Teal",
            Self::Pink => "Pink",
            Self::Gray => "Gray",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "blue" => Some(Self::Blue),
            "orange" => Some(Self::Orange),
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            "purple" => Some(Self::Purple),
            "teal" => Some(Self::Teal),
            "pink" => Some(Self::Pink),
            "gray" => Some(Self::Gray),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar2Color);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar2Provider {
    Apple,
    Google,
    Outlook,
    Proton,
    Fastmail,
    Nextcloud,
    Other,
}
impl Calendar2Provider {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Apple,
        Self::Google,
        Self::Outlook,
        Self::Proton,
        Self::Fastmail,
        Self::Nextcloud,
        Self::Other,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
            Self::Outlook => "outlook",
            Self::Proton => "proton",
            Self::Fastmail => "fastmail",
            Self::Nextcloud => "nextcloud",
            Self::Other => "other",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Apple => "Apple Calendar (iCloud)",
            Self::Google => "Google Calendar",
            Self::Outlook => "Outlook (Outlook.com or Microsoft 365)",
            Self::Proton => "Proton Calendar",
            Self::Fastmail => "Fastmail",
            Self::Nextcloud => "Nextcloud",
            Self::Other => "Other (any iCalendar link)",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "apple" => Some(Self::Apple),
            "google" => Some(Self::Google),
            "outlook" => Some(Self::Outlook),
            "proton" => Some(Self::Proton),
            "fastmail" => Some(Self::Fastmail),
            "nextcloud" => Some(Self::Nextcloud),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar2Provider);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar3Color {
    Blue,
    Orange,
    Green,
    Red,
    Purple,
    Teal,
    Pink,
    Gray,
}
impl Calendar3Color {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Blue,
        Self::Orange,
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Teal,
        Self::Pink,
        Self::Gray,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Orange => "orange",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
            Self::Teal => "teal",
            Self::Pink => "pink",
            Self::Gray => "gray",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Orange => "Orange",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Teal => "Teal",
            Self::Pink => "Pink",
            Self::Gray => "Gray",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "blue" => Some(Self::Blue),
            "orange" => Some(Self::Orange),
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            "purple" => Some(Self::Purple),
            "teal" => Some(Self::Teal),
            "pink" => Some(Self::Pink),
            "gray" => Some(Self::Gray),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar3Color);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar3Provider {
    Apple,
    Google,
    Outlook,
    Proton,
    Fastmail,
    Nextcloud,
    Other,
}
impl Calendar3Provider {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[
        Self::Apple,
        Self::Google,
        Self::Outlook,
        Self::Proton,
        Self::Fastmail,
        Self::Nextcloud,
        Self::Other,
    ];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
            Self::Outlook => "outlook",
            Self::Proton => "proton",
            Self::Fastmail => "fastmail",
            Self::Nextcloud => "nextcloud",
            Self::Other => "other",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Apple => "Apple Calendar (iCloud)",
            Self::Google => "Google Calendar",
            Self::Outlook => "Outlook (Outlook.com or Microsoft 365)",
            Self::Proton => "Proton Calendar",
            Self::Fastmail => "Fastmail",
            Self::Nextcloud => "Nextcloud",
            Self::Other => "Other (any iCalendar link)",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "apple" => Some(Self::Apple),
            "google" => Some(Self::Google),
            "outlook" => Some(Self::Outlook),
            "proton" => Some(Self::Proton),
            "fastmail" => Some(Self::Fastmail),
            "nextcloud" => Some(Self::Nextcloud),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Calendar3Provider);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    _5,
    _15,
    _30,
    _60,
}
impl Refresh {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[Self::_5, Self::_15, Self::_30, Self::_60];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::_5 => "5",
            Self::_15 => "15",
            Self::_30 => "30",
            Self::_60 => "60",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::_5 => "Every 5 minutes",
            Self::_15 => "Every 15 minutes",
            Self::_30 => "Every 30 minutes",
            Self::_60 => "Every hour",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "5" => Some(Self::_5),
            "15" => Some(Self::_15),
            "30" => Some(Self::_30),
            "60" => Some(Self::_60),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Refresh);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    Night,
}
impl Theme {
    /// Every variant, in manifest-declaration order. Useful when a widget
    /// wants to render a "pick one" UI or audit the enum exhaustively.
    pub const ALL: &'static [Self] = &[Self::Dark, Self::Light, Self::Night];
    /// Manifest wire value for this variant.
    #[must_use]
    pub fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Night => "night",
        }
    }
    /// Human-readable label declared in the manifest's `enum_values`.
    #[must_use]
    pub fn as_manifest_label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Night => "Light by day, dark at night",
        }
    }
    #[must_use]
    pub fn from_manifest_value(s: &str) -> Option<Self> {
        match s {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "night" => Some(Self::Night),
            _ => None,
        }
    }
}
bmc_wasm_sdk::impl_manifest_str_enum!(Theme);
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub calendar_1_color: Option<Calendar1Color>,
    pub calendar_1_name: Option<String>,
    pub calendar_1_provider: Option<Calendar1Provider>,
    pub calendar_2_color: Option<Calendar2Color>,
    pub calendar_2_name: Option<String>,
    pub calendar_2_provider: Option<Calendar2Provider>,
    pub calendar_3_color: Option<Calendar3Color>,
    pub calendar_3_name: Option<String>,
    pub calendar_3_provider: Option<Calendar3Provider>,
    pub refresh: Option<Refresh>,
    pub theme: Option<Theme>,
}
impl Params {
    /// Materialise a typed snapshot from a dynamic [`snapshot::Params`].
    #[must_use]
    pub fn from_snapshot(snap: &snapshot::Params) -> Self {
        Self {
            calendar_1_color: <Calendar1Color as ParamRead>::read_optional(
                snap,
                "calendar_1_color",
            ),
            calendar_1_name: <String as ParamRead>::read_optional(snap, "calendar_1_name"),
            calendar_1_provider: <Calendar1Provider as ParamRead>::read_optional(
                snap,
                "calendar_1_provider",
            ),
            calendar_2_color: <Calendar2Color as ParamRead>::read_optional(
                snap,
                "calendar_2_color",
            ),
            calendar_2_name: <String as ParamRead>::read_optional(snap, "calendar_2_name"),
            calendar_2_provider: <Calendar2Provider as ParamRead>::read_optional(
                snap,
                "calendar_2_provider",
            ),
            calendar_3_color: <Calendar3Color as ParamRead>::read_optional(
                snap,
                "calendar_3_color",
            ),
            calendar_3_name: <String as ParamRead>::read_optional(snap, "calendar_3_name"),
            calendar_3_provider: <Calendar3Provider as ParamRead>::read_optional(
                snap,
                "calendar_3_provider",
            ),
            refresh: <Refresh as ParamRead>::read_optional(snap, "refresh"),
            theme: <Theme as ParamRead>::read_optional(snap, "theme"),
        }
    }
    /// Latest typed snapshot delivered for this widget instance.
    /// Cached per-thread; only re-parses when `snapshot::version()` changes
    /// since the last call.
    #[must_use]
    pub fn current() -> Self {
        thread_local! {
            static CACHE : core::cell::RefCell < Option < (u64, Params) >> = const {
            core::cell::RefCell::new(None) };
        }
        let v = snapshot::version();
        CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if let Some((cv, ref params)) = *cache
                && cv == v
            {
                return params.clone();
            }
            let fresh = Self::from_snapshot(&snapshot::current());
            *cache = Some((v, fresh.clone()));
            fresh
        })
    }
    /// Snapshot delivered immediately before [`current`]; `None` until at
    /// least one update has been observed (i.e. during `init` and the
    /// first `render`).
    #[must_use]
    pub fn previous() -> Option<Self> {
        let prev = snapshot::previous();
        if prev.is_empty() {
            None
        } else {
            Some(Self::from_snapshot(&prev))
        }
    }
    /// Manifest keys whose value differs between `self` and `other`.
    ///
    /// Intended for `on_params_update` diffing — pass `current()` and the
    /// inside-hook value of `previous()` to get the set of keys to react
    /// to. Field-by-field `PartialEq`; emitted in struct-field order so
    /// the result is deterministic.
    #[must_use]
    pub fn changed_keys(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.calendar_1_color != other.calendar_1_color {
            out.push("calendar_1_color");
        }
        if self.calendar_1_name != other.calendar_1_name {
            out.push("calendar_1_name");
        }
        if self.calendar_1_provider != other.calendar_1_provider {
            out.push("calendar_1_provider");
        }
        if self.calendar_2_color != other.calendar_2_color {
            out.push("calendar_2_color");
        }
        if self.calendar_2_name != other.calendar_2_name {
            out.push("calendar_2_name");
        }
        if self.calendar_2_provider != other.calendar_2_provider {
            out.push("calendar_2_provider");
        }
        if self.calendar_3_color != other.calendar_3_color {
            out.push("calendar_3_color");
        }
        if self.calendar_3_name != other.calendar_3_name {
            out.push("calendar_3_name");
        }
        if self.calendar_3_provider != other.calendar_3_provider {
            out.push("calendar_3_provider");
        }
        if self.refresh != other.refresh {
            out.push("refresh");
        }
        if self.theme != other.theme {
            out.push("theme");
        }
        out
    }
}
/// Credential slots this widget declares, one module per slot.
pub mod credentials {
    ///Calendar 1 link — a `generic-token` account. Optional.
    ///
    ///The calendar's private link, saved as a Token account. It starts with https://
    pub mod calendar_1_link {
        ///Placeholder for this slot's `token` field.
        pub const TOKEN: &str = "{{ credential.calendar_1_link.token }}";
    }
    ///Calendar 2 link — a `generic-token` account. Optional.
    ///
    ///Optional. A second calendar's private link, saved as a Token account
    pub mod calendar_2_link {
        ///Placeholder for this slot's `token` field.
        pub const TOKEN: &str = "{{ credential.calendar_2_link.token }}";
    }
    ///Calendar 3 link — a `generic-token` account. Optional.
    ///
    ///Optional. A third calendar's private link, saved as a Token account
    pub mod calendar_3_link {
        ///Placeholder for this slot's `token` field.
        pub const TOKEN: &str = "{{ credential.calendar_3_link.token }}";
    }
}
