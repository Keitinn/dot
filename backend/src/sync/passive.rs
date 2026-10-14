//! Passive income counted by time slot (design「accrue_passive の二重加算」).
//! Slot k covers [k * 600, (k + 1) * 600). Each device records the slots it
//! paid; the merged income is the size of the union, so two Macs paying for
//! the same ten minutes count once.

use serde::{Deserialize, Serialize};
use crate::pet::stats::PASSIVE_INTERVAL_SECS;

/// The slot containing unix second `t`.
pub fn slot_of(t: i64) -> i64 {
    t.div_euclid(PASSIVE_INTERVAL_SECS)
}

/// The first slot starting at or after `t`: ceil(t / 600).
pub fn first_slot_from(t: i64) -> i64 {
    -(-t).div_euclid(PASSIVE_INTERVAL_SECS)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PassiveLedger {
    /// Paid slots as half-open ranges [start, end), sorted, disjoint and not touching.
    pub ranges: Vec<(i64, i64)>,
    /// Slots before this one were paid by the old unaligned payout and are not
    /// recorded. Set once, when the ledger starts.
    pub first_slot: Option<i64>,
}

impl PassiveLedger {
    /// A ledger for a save that so far paid up to `last_passive_time`. The
    /// first slot is rounded up, dropping less than one coin.
    pub fn starting_at(last_passive_time: i64) -> Self {
        Self { ranges: Vec::new(), first_slot: Some(first_slot_from(last_passive_time)) }
    }

    /// Records slots [start, end) as paid.
    pub fn insert(&mut self, start: i64, end: i64) {
        if start < end {
            self.ranges.push((start, end));
            self.normalize();
        }
    }

    /// Joins another version of the same ledger, or another device's ledger.
    pub fn absorb(&mut self, other: &PassiveLedger) {
        self.ranges.extend_from_slice(&other.ranges);
        self.normalize();
        self.first_slot = match (self.first_slot, other.first_slot) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }

    /// Drops every slot outside [lo, hi), so a file with a made-up range
    /// cannot pay for time that has not passed.
    pub fn clamp_to(&mut self, lo: i64, hi: i64) {
        for range in &mut self.ranges {
            *range = (range.0.max(lo), range.1.min(hi));
        }
        self.normalize();
    }

    pub fn slot_count(&self) -> u64 {
        self.ranges.iter().map(|&(start, end)| end.abs_diff(start)).sum()
    }

    /// Records what one `GameStats::accrue_passive` call paid. `before` and
    /// `after` are `last_passive_time` around the call, `earned` its result.
    /// The `earned` slots that ended by `after` are marked; slots already in
    /// the ledger are not counted twice.
    pub fn record_accrual(&mut self, before: Option<i64>, after: Option<i64>, earned: u64) {
        let Some(after) = after else { return };
        if before.is_none() {
            self.first_slot.get_or_insert(first_slot_from(after));
        }
        if earned == 0 {
            return;
        }
        let end = slot_of(after);
        let count = i64::try_from(earned).unwrap_or(i64::MAX);
        let start = end.saturating_sub(count).max(self.first_slot.unwrap_or(i64::MIN));
        self.insert(start, end);
    }

    /// Sorts and joins overlapping or touching ranges; drops empty ones a
    /// hand-edited file might hold.
    fn normalize(&mut self) {
        self.ranges.retain(|&(start, end)| start < end);
        self.ranges.sort_unstable();
        let mut joined: Vec<(i64, i64)> = Vec::with_capacity(self.ranges.len());
        for &(start, end) in &self.ranges {
            match joined.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => joined.push((start, end)),
            }
        }
        self.ranges = joined;
    }
}

/// The number of slots paid by any of `ledgers`.
pub fn union_count<'a>(ledgers: impl IntoIterator<Item = &'a PassiveLedger>) -> u64 {
    let mut all = PassiveLedger::default();
    for ledger in ledgers {
        all.absorb(ledger);
    }
    all.slot_count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::GameStats;

    const K0: i64 = 2_833_333;

    #[test]
    fn slot_boundaries() {
        assert_eq!(slot_of(0), 0);
        assert_eq!(slot_of(599), 0);
        assert_eq!(slot_of(600), 1);
        assert_eq!(slot_of(-1), -1);
        assert_eq!(first_slot_from(1_000), 2);
        assert_eq!(first_slot_from(1_200), 2);
        assert_eq!(first_slot_from(1_201), 3);
    }

    #[test]
    fn migration_rounds_the_start_up() {
        let ledger = PassiveLedger::starting_at(1_000);
        assert_eq!(ledger.first_slot, Some(2));
        assert_eq!(ledger.slot_count(), 0);
    }

    #[test]
    fn ranges_join_and_count_once() {
        let mut ledger = PassiveLedger::default();
        ledger.insert(10, 20);
        ledger.insert(15, 25);
        ledger.insert(25, 30);
        ledger.insert(40, 41);
        ledger.insert(5, 5);
        assert_eq!(ledger.ranges, vec![(10, 30), (40, 41)]);
        assert_eq!(ledger.slot_count(), 21);
    }

    #[test]
    fn two_devices_paying_the_same_two_hours_count_twelve() {
        let mut a = PassiveLedger::default();
        let mut b = PassiveLedger::default();
        a.record_accrual(Some(K0 * 600), Some((K0 + 12) * 600), 12);
        b.record_accrual(Some(K0 * 600), Some((K0 + 12) * 600), 12);
        assert_eq!(a.ranges, vec![(K0, K0 + 12)]);
        assert_eq!(union_count([&a, &b]), 12);
    }

    /// Runs `accrue_passive` every minute for `minutes` and records each call.
    fn accrue_every_minute(stats: &mut GameStats, ledger: &mut PassiveLedger, from: i64, minutes: i64) -> u64 {
        (1..=minutes)
            .map(|minute| {
                let before = stats.last_passive_time;
                let earned = stats.accrue_passive(from + minute * 60, true);
                ledger.record_accrual(before, stats.last_passive_time, earned);
                earned
            })
            .sum()
    }

    #[test]
    fn records_what_accrue_passive_paid() {
        let mut stats = GameStats::new();
        let mut ledger = PassiveLedger::default();
        let start = K0 * 600;
        let before = stats.last_passive_time;
        assert_eq!(stats.accrue_passive(start, true), 0);
        ledger.record_accrual(before, stats.last_passive_time, 0);
        assert_eq!(ledger.first_slot, Some(K0));

        assert_eq!(accrue_every_minute(&mut stats, &mut ledger, start, 120), 12);
        assert_eq!(ledger.ranges, vec![(K0, K0 + 12)]);
    }

    #[test]
    fn unaligned_old_clock_loses_at_most_one_slot() {
        let start = K0 * 600 + 400;
        let mut stats = GameStats::new();
        stats.last_passive_time = Some(start);
        let mut ledger = PassiveLedger::starting_at(start);

        assert_eq!(accrue_every_minute(&mut stats, &mut ledger, start, 120), 12);
        assert!((11..=12).contains(&ledger.slot_count()), "{:?}", ledger);
    }

    #[test]
    fn unaligned_first_run_loses_at_most_one_slot() {
        let start = K0 * 600 + 250;
        let mut stats = GameStats::new();
        let mut ledger = PassiveLedger::default();
        let before = stats.last_passive_time;
        stats.accrue_passive(start, true);
        ledger.record_accrual(before, stats.last_passive_time, 0);
        assert_eq!(ledger.first_slot, Some(K0 + 1));

        assert_eq!(accrue_every_minute(&mut stats, &mut ledger, start, 120), 12);
        assert!((11..=12).contains(&ledger.slot_count()), "{:?}", ledger);
    }

    #[test]
    fn a_long_gap_records_the_capped_slots_once() {
        let mut ledger = PassiveLedger::default();
        let now = (K0 + 1_000) * 600;
        ledger.record_accrual(Some(K0 * 600), Some(now), 72);
        assert_eq!(ledger.ranges, vec![(K0 + 1_000 - 72, K0 + 1_000)]);
        ledger.record_accrual(Some(K0 * 600), Some(now), 72);
        assert_eq!(ledger.slot_count(), 72);
    }

    #[test]
    fn absorb_is_commutative_idempotent_and_keeps_the_earliest_start() {
        let mut a = PassiveLedger::starting_at(6_000);
        a.insert(10, 14);
        let mut b = PassiveLedger::starting_at(3_000);
        b.insert(12, 20);
        let mut ab = a.clone();
        ab.absorb(&b);
        let mut ba = b.clone();
        ba.absorb(&a);
        assert_eq!(ab, ba);
        let mut abb = ab.clone();
        abb.absorb(&b);
        assert_eq!(abb, ab);
        assert_eq!(ab.first_slot, Some(5));
        assert_eq!(ab.slot_count(), 10);
    }
}
