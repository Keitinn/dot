//! Save sync between Macs, stage 0 (design/gdd/companion.md「複数端末とセーブ同期」).
//! Each device writes only its own `devices/<device_id>.json`; reading merges
//! all of them with rules that are commutative and idempotent, so late,
//! repeated or conflicting copies of a file all end in the same state.
//! Nothing here is wired into the game yet: the sync folder setting, the
//! wiring and `WHICH DOT?` come with #38.

pub mod device;
pub mod folder;
pub mod merge;
pub mod passive;
pub mod snapshot;

pub use device::DeviceRecord;
pub use folder::SyncCache;
pub use merge::{merge, BondReplay, Merged};
pub use passive::PassiveLedger;
pub use snapshot::DeviceSnapshot;

#[cfg(test)]
mod tests;
