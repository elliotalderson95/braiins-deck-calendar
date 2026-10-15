//! The widget's settings in plain terms, read from the generated params.

use bmc_wasm_sdk::{
    BLUE_50, Color, GRAY_40, GREEN_40, MAGENTA_50, ORANGE_40, PURPLE_50, RED_50, TEAL_40,
};

use crate::manifest_params::{
    Calendar1Color, Calendar1Provider, Calendar2Color, Calendar2Provider, Calendar3Color,
    Calendar3Provider, Params, Refresh, Theme,
};
use crate::model::MAX_CALENDARS;
use crate::providers::Provider;

/// The colour a calendar's events carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    Blue,
    Orange,
    Green,
    Red,
    Purple,
    Teal,
    Pink,
    Gray,
}

impl Accent {
    #[must_use]
    pub fn from_manifest_value(value: &str) -> Option<Self> {
        Some(match value {
            "blue" => Self::Blue,
            "orange" => Self::Orange,
            "green" => Self::Green,
            "red" => Self::Red,
            "purple" => Self::Purple,
            "teal" => Self::Teal,
            "pink" => Self::Pink,
            "gray" => Self::Gray,
            _ => return None,
        })
    }

    #[must_use]
    pub fn color(self) -> Color {
        match self {
            Self::Blue => BLUE_50,
            Self::Orange => ORANGE_40,
            Self::Green => GREEN_40,
            Self::Red => RED_50,
            Self::Purple => PURPLE_50,
            Self::Teal => TEAL_40,
            Self::Pink => MAGENTA_50,
            Self::Gray => GRAY_40,
        }
    }
}

/// When the widget is light and when dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Dark,
    Light,
    /// Light, but dark while the Deck is in night mode.
    ByNight,
}

/// One calendar's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarSettings {
    pub provider: Provider,
    /// The name the owner gave it, if any.
    pub name: Option<String>,
    pub accent: Accent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub calendars: [CalendarSettings; MAX_CALENDARS],
    pub look: Look,
    /// How often to ask the calendars for news.
    pub refresh_minutes: u32,
}

impl Settings {
    #[must_use]
    pub fn from_params(params: &Params) -> Self {
        let calendar = |provider: Option<&str>,
                        name: &Option<String>,
                        accent: Option<&str>,
                        fallback: (Provider, Accent)| {
            CalendarSettings {
                provider: provider
                    .and_then(Provider::from_manifest_value)
                    .unwrap_or(fallback.0),
                name: name
                    .as_ref()
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty()),
                accent: accent
                    .and_then(Accent::from_manifest_value)
                    .unwrap_or(fallback.1),
            }
        };
        Self {
            calendars: [
                calendar(
                    params
                        .calendar_1_provider
                        .map(Calendar1Provider::as_manifest_value),
                    &params.calendar_1_name,
                    params
                        .calendar_1_color
                        .map(Calendar1Color::as_manifest_value),
                    (Provider::Apple, Accent::Blue),
                ),
                calendar(
                    params
                        .calendar_2_provider
                        .map(Calendar2Provider::as_manifest_value),
                    &params.calendar_2_name,
                    params
                        .calendar_2_color
                        .map(Calendar2Color::as_manifest_value),
                    (Provider::Google, Accent::Orange),
                ),
                calendar(
                    params
                        .calendar_3_provider
                        .map(Calendar3Provider::as_manifest_value),
                    &params.calendar_3_name,
                    params
                        .calendar_3_color
                        .map(Calendar3Color::as_manifest_value),
                    (Provider::Outlook, Accent::Green),
                ),
            ],
            look: match params.theme {
                Some(Theme::Light) => Look::Light,
                Some(Theme::Night) => Look::ByNight,
                Some(Theme::Dark) | None => Look::Dark,
            },
            refresh_minutes: params
                .refresh
                .map(Refresh::as_manifest_value)
                .and_then(|minutes| minutes.parse().ok())
                .unwrap_or(15),
        }
    }
}
