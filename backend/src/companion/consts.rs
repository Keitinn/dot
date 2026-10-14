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

// Mood (R2 / F2)

/// Shortest time a mood stays while the popover is open, so the eyes do not flicker.
pub const MOOD_MIN_DWELL_SECS: i64 = 600;
/// `concerned` starts when the lowest need is below this…
pub const CONCERNED_ENTER_NEEDS: u8 = 20;
/// …or health is below this.
pub const CONCERNED_ENTER_HEALTH: u8 = 30;
/// `concerned` ends only once every need and health are at least this.
pub const CONCERNED_EXIT: u8 = 30;
pub const SLEEPY_ENERGY: u8 = 35;
/// Sleepy hours, local. Start inclusive, end exclusive (23:00–05:59).
pub const SLEEPY_START_HOUR: u32 = 23;
pub const SLEEPY_END_HOUR: u32 = 6;
/// Below this lowest need the mood is `calm` rather than `cheerful`.
pub const CALM_NEEDS: u8 = 60;

// Being away (R4, MVP: time since the last open, no idle reading)

pub const AWAY_RETURN_H: i64 = 4;
pub const MISSED_YOU_H: i64 = 24;

// Lines (R3)

/// Chance that `time_of_day` / `mood` lines speak when they are reached.
pub const LINE_SPEAK_P: f64 = 0.5;
pub const LINE_CATEGORY_COOLDOWN_H: i64 = 24;
/// `mood_concerned` speaks about the same need at most once in this many hours.
pub const CONCERNED_VALVE_H: i64 = 6;
/// Characters per line, not counting the row break.
pub const LINE_MAX_CHARS: usize = 24;
pub const LINE_MAX_ROWS: usize = 2;
pub const LINES_MIN: usize = 40;
pub const LINES_PER_CATEGORY_MIN: usize = 6;

// Onboarding and settings (R1)

/// LATER asks once more after this many days.
pub const LATER_REPROMPT_D: i64 = 7;
pub const QUIET_1H_SECS: i64 = 3600;
