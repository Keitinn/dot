//! Adult companion (design/gdd/companion.md). Active only while the pet is a
//! living adult. Never creates coins (R11).

pub mod bond;
pub mod consts;
pub mod lines;
pub mod mood;
pub mod open;
pub mod settings;
pub mod state;

pub use mood::Mood;
pub use open::{open, OpenResult, SaidLine};
pub use settings::{OnboardingAnswer, SettingsUpdate, SettingsView};
pub use state::{CareKind, CompanionState, Onboarding};
