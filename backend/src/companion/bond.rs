//! Bond daily settlement (design/gdd/companion.md F1), as pure functions so a
//! future multi-device merge can replay merged day counters through them.

use serde::{Deserialize, Serialize};
use super::consts::*;

/// What one day added towards Bond, reset after each settlement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DayCounters {
    /// Opened the popover by hand (tray click only).
    pub g: bool,
    /// FEED / PLAY / SLEEP done while the need it fills was below CARE_NEED_BELOW.
    pub n_care: u32,
    /// Focus sessions run to the end.
    pub n_focus: u32,
}

/// Result of settling one day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    pub bond: u16,
    /// The new `overdue_applied`.
    pub overdue_applied: u32,
}

/// G: the day's gain, capped at G_CAP.
pub fn daily_gain(day: &DayCounters) -> u32 {
    let greet = if day.g { G_GREET } else { 0 };
    let care = G_CARE * day.n_care.min(N_CARE_MAX);
    let focus = G_FOCUS * day.n_focus.min(N_FOCUS_MAX);
    (greet + care + focus).min(G_CAP)
}

/// overdue(today) for `days_since_open` (m) days since the last manual open.
pub fn overdue_days(days_since_open: i64) -> u32 {
    (days_since_open - GRACE).clamp(0, M_MAX as i64) as u32
}

/// The highest tier `bond` has reached, or 0.
pub fn tier_floor(bond: u16) -> u16 {
    BOND_TIERS.iter().copied().filter(|&tier| tier <= bond).max().unwrap_or(0)
}

/// B' for one settlement. Decay is the difference from what was already
/// applied, so settling every day or once after a gap ends in the same B.
pub fn settle(bond: u16, day: &DayCounters, days_since_open: i64, overdue_applied: u32) -> Settled {
    let overdue = overdue_days(days_since_open);
    let decay = DECAY * overdue.saturating_sub(overdue_applied);
    let raw = bond as i64 + daily_gain(day) as i64 - decay as i64;
    let floored = raw.max(tier_floor(bond) as i64);
    Settled {
        bond: floored.clamp(0, BOND_MAX as i64) as u16,
        overdue_applied: overdue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(g: bool, n_care: u32, n_focus: u32) -> DayCounters {
        DayCounters { g, n_care, n_focus }
    }

    #[test]
    fn gain_adds_greet_care_and_focus() {
        assert_eq!(daily_gain(&day(true, 1, 1)), 10 + 3 + 15);
        assert_eq!(daily_gain(&DayCounters::default()), 0);
    }

    #[test]
    fn care_counts_up_to_n_care_max() {
        assert_eq!(daily_gain(&day(false, 5, 0)), 15);
        assert_eq!(daily_gain(&day(false, 9, 0)), 15);
    }

    #[test]
    fn focus_counts_up_to_n_focus_max() {
        assert_eq!(daily_gain(&day(false, 0, 3)), 30);
    }

    #[test]
    fn gain_is_capped_at_50() {
        // 10 + 15 + 30 = 55
        assert_eq!(daily_gain(&day(true, 7, 3)), 50);
    }

    #[test]
    fn overdue_starts_after_grace_and_stops_at_m_max() {
        assert_eq!(overdue_days(0), 0);
        assert_eq!(overdue_days(3), 0);
        assert_eq!(overdue_days(4), 1);
        assert_eq!(overdue_days(33), 30);
        assert_eq!(overdue_days(400), 30);
        assert_eq!(overdue_days(-2), 0);
    }

    #[test]
    fn tier_floor_is_the_highest_tier_reached() {
        assert_eq!(tier_floor(0), 0);
        assert_eq!(tier_floor(99), 0);
        assert_eq!(tier_floor(100), 100);
        assert_eq!(tier_floor(299), 100);
        assert_eq!(tier_floor(600), 600);
        assert_eq!(tier_floor(1000), 1000);
    }

    #[test]
    fn decay_is_charged_only_for_new_overdue_days() {
        // F1 example: B=280, last open d0, app running every day
        let none = DayCounters::default();
        let d3 = settle(280, &none, 3, 0);
        assert_eq!(d3, Settled { bond: 280, overdue_applied: 0 });
        let d4 = settle(d3.bond, &none, 4, d3.overdue_applied);
        assert_eq!(d4, Settled { bond: 272, overdue_applied: 1 });
        let d5 = settle(d4.bond, &none, 5, d4.overdue_applied);
        assert_eq!(d5, Settled { bond: 264, overdue_applied: 2 });
    }

    #[test]
    fn one_settlement_after_a_gap_matches_daily_settlements() {
        let once = settle(280, &DayCounters::default(), 5, 0);
        assert_eq!(once, Settled { bond: 264, overdue_applied: 2 });
    }

    #[test]
    fn decay_never_breaks_a_reached_tier() {
        let settled = settle(120, &DayCounters::default(), 365, 0);
        assert_eq!(settled.bond, 100);
        assert_eq!(settle(105, &DayCounters::default(), 30, 0).bond, 100);
    }

    #[test]
    fn below_the_first_tier_bond_can_fall_to_zero() {
        assert_eq!(settle(90, &DayCounters::default(), 365, 0).bond, 0);
    }

    #[test]
    fn bond_stops_at_1000() {
        assert_eq!(settle(990, &day(true, 5, 2), 1, 0).bond, 1000);
    }

    #[test]
    fn gain_and_decay_apply_in_the_same_settlement() {
        // 200 + 10 - 8
        assert_eq!(settle(200, &day(true, 0, 0), 4, 0).bond, 202);
    }
}
