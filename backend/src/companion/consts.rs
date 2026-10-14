//! Tuning knobs of the companion (design/gdd/companion.md, Tuning Knobs and F1).
//! Keep every number here instead of scattering them through the code.

/// Version written to `companion.json`.
pub const SCHEMA_VERSION: u32 = 1;

/// Bond points for opening the popover by hand that day.
pub const G_GREET: u32 = 10;
/// Bond points per counted care (FEED / PLAY / SLEEP while the need was low).
pub const G_CARE: u32 = 3;
/// Cares counted per day.
pub const N_CARE_MAX: u32 = 5;
/// Bond points per finished focus session.
pub const G_FOCUS: u32 = 15;
/// Focus sessions counted per day.
pub const N_FOCUS_MAX: u32 = 2;
/// Most Bond one day can add.
pub const G_CAP: u32 = 50;

/// Days without a manual open before Bond starts to fall. 3 so a weekend
/// (Friday → Monday is m=3) costs nothing.
pub const GRACE: i64 = 3;
/// Bond lost per overdue day.
pub const DECAY: u32 = 8;
/// Most overdue days ever counted.
pub const M_MAX: u32 = 30;

pub const BOND_MAX: u16 = 1000;
/// Bond never falls below the highest tier it has reached.
pub const BOND_TIERS: [u16; 4] = [100, 300, 600, 1000];

/// A care counts only when the need it fills was below this.
pub const CARE_NEED_BELOW: u8 = 80;

/// Default quiet hours (local hour, start inclusive, end exclusive).
pub const QUIET_START_DEFAULT: u8 = 22;
pub const QUIET_END_DEFAULT: u8 = 8;

/// Line IDs remembered so they are not repeated.
pub const LINE_HISTORY: usize = 20;
