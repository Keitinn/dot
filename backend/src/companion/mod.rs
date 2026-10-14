//! Adult companion (design/gdd/companion.md). Active only while the pet is a
//! living adult. Never creates coins (R11).

pub mod bond;
pub mod consts;
pub mod state;

pub use state::{CareKind, CompanionState, Onboarding};
