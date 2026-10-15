//! The calendar services the widget knows how to guide a user through.
//!
//! The Deck can't sign in to an account, so every service is read through the
//! private link it publishes for its calendar (an iCalendar `.ics` feed).
//! What differs between services is where that link is found, which is what
//! the widget shows on screen until a calendar is connected.

/// A calendar service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Apple,
    Google,
    Outlook,
    Proton,
    Fastmail,
    Nextcloud,
    /// Any other service that offers an iCalendar link.
    Other,
}

impl Provider {
    pub const ALL: [Self; 7] = [
        Self::Apple,
        Self::Google,
        Self::Outlook,
        Self::Proton,
        Self::Fastmail,
        Self::Nextcloud,
        Self::Other,
    ];

    /// The service from its manifest value.
    #[must_use]
    pub fn from_manifest_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.manifest_value() == value)
    }

    #[must_use]
    pub fn manifest_value(self) -> &'static str {
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

    /// The service's name, as the calendar is called when it has no name of its own.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Apple => "Apple Calendar",
            Self::Google => "Google Calendar",
            Self::Outlook => "Outlook",
            Self::Proton => "Proton Calendar",
            Self::Fastmail => "Fastmail",
            Self::Nextcloud => "Nextcloud",
            Self::Other => "Calendar",
        }
    }

    /// The steps that find the calendar's link, in a few words each.
    #[must_use]
    pub fn steps(self) -> &'static [&'static str] {
        match self {
            Self::Apple => &[
                "On your iPhone open Calendar, tap Calendars, then the (i) beside the calendar.",
                "Turn on Public Calendar, tap Share Link and Copy.",
                "In the link, change webcal:// at the start to https://",
            ],
            Self::Google => &[
                "On a computer open calendar.google.com, then Settings (the gear).",
                "Pick the calendar under Settings for my calendars, then Integrate calendar.",
                "Copy the Secret address in iCal format.",
            ],
            Self::Outlook => &[
                "Open Outlook on the web, then Settings (the gear), Calendar, Shared calendars.",
                "Under Publish a calendar pick the calendar, choose Can view all details, tap Publish.",
                "Copy the ICS link (not the HTML one).",
            ],
            Self::Proton => &[
                "Open Proton Calendar, then Settings, Calendars, and the calendar.",
                "Under Share with anyone create a link.",
                "Copy the link.",
            ],
            Self::Fastmail => &[
                "Open Fastmail, then Settings, Calendars, and the calendar.",
                "Find its iCalendar subscription link (under sharing).",
                "Copy the link.",
            ],
            Self::Nextcloud => &[
                "Open Calendar in Nextcloud and the calendar's menu (the three dots).",
                "Choose Copy subscription link (or Copy private link, then add ?export at its end).",
                "Copy the link.",
            ],
            Self::Other => &[
                "Open the calendar's sharing or export settings.",
                "Look for an iCalendar, ICS or subscription link.",
                "Copy the link.",
            ],
        }
    }

    /// What to try when the link isn't where the steps say it is, for a tile
    /// with room to say so. Only some services need one.
    #[must_use]
    pub fn missing_link_note(self) -> Option<&'static str> {
        match self {
            Self::Google => Some(
                "Not there? A work or custom domain Google account's admin has turned it off. Ask the admin, or see the README.",
            ),
            _ => None,
        }
    }

    /// The same steps, used when the link was copied but doesn't work.
    #[must_use]
    pub fn mistake_hint(self) -> &'static str {
        match self {
            Self::Apple => "It must start with https:// and be the Public Calendar link.",
            Self::Google => "It must be the Secret address in iCal format, ending in .ics.",
            Self::Outlook => "It must be the ICS link, ending in .ics, not the HTML one.",
            Self::Proton | Self::Fastmail | Self::Nextcloud | Self::Other => {
                "It must be a link to an .ics calendar file."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_service_round_trips_through_its_manifest_value() {
        for provider in Provider::ALL {
            assert_eq!(
                Provider::from_manifest_value(provider.manifest_value()),
                Some(provider)
            );
        }
        assert_eq!(Provider::from_manifest_value("yahoo"), None);
    }

    #[test]
    fn only_google_has_a_note_for_a_missing_link_and_it_has_no_dashes() {
        for provider in Provider::ALL {
            match provider.missing_link_note() {
                Some(note) => {
                    assert_eq!(provider, Provider::Google);
                    assert!(!note.contains('\u{2014}') && !note.contains('\u{2013}'));
                }
                None => assert_ne!(provider, Provider::Google),
            }
        }
    }

    #[test]
    fn every_service_has_steps_and_no_dashes() {
        for provider in Provider::ALL {
            assert_eq!(provider.steps().len(), 3, "{provider:?}");
            let text: String =
                provider.steps().concat() + provider.mistake_hint() + provider.name();
            assert!(
                !text.contains('\u{2014}') && !text.contains('\u{2013}'),
                "{provider:?}"
            );
        }
    }
}
