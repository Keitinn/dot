//! Save sync between Macs (design/gdd/companion.md「複数端末とセーブ同期」).
//! Each device writes only its own `devices/<device_id>.json`; reading merges
//! all of them with rules that are commutative and idempotent, so late,
//! repeated or conflicting copies of a file all end in the same state.
//! Stage 1 (#38): the folder is one the user names in SYNC; whatever keeps it
//! in step between Macs is outside dot. Off by default.

pub mod device;
pub mod folder;
pub mod merge;
pub mod passive;
pub mod runtime;
pub mod settings;
pub mod snapshot;

pub use device::DeviceRecord;
pub use folder::SyncCache;
pub use merge::{merge, BondReplay, Merged};
pub use passive::PassiveLedger;
pub use runtime::{Hardware, PetChoice, SyncRuntime, SyncSettingsView, SyncStatusView};
pub use snapshot::DeviceSnapshot;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wiring_tests;
