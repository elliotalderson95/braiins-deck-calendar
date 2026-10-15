//! Deck Calendar: the events from your calendars on the Deck.
//!
//! Each calendar is read from the private link its service gives out. The
//! Deck can't sign in to Apple, Google or Microsoft, but all of them (and
//! most others) publish a read-only `.ics` link. Everything but `live` is
//! plain Rust that builds and tests natively.

pub mod civil;
pub mod expand;
pub mod ics;
#[cfg(target_arch = "wasm32")]
mod live;
mod manifest_params;
pub mod model;
pub mod nav;
pub mod providers;
pub mod settings;
pub mod view;
pub mod zones;

pub use manifest_params::Params;
