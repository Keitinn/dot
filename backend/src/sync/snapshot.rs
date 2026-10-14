//! One device's file, `devices/<device_id>.json` (design「保存の形」). A device
//! writes only its own file, and only what it did itself: counters, coins and
//! Bond day counters are this device's share, so adding the files up never
//! counts anything twice.

use std::collections::{BTreeMap, BTreeSet};
use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};
use crate::companion::bond::DayCounters;
use crate::companion::{CareKind, CompanionState, Onboarding};
use crate::pet::{GameStats, PetState};
use super::merge::compare_pets;
use super::passive::PassiveLedger;

pub const SCHEMA_VERSION: u32 = 1;
/// Care dates kept for the streak, counting back from the newest.
pub const STREAK_WINDOW_D: u64 = 60;

/// Every field falls back to its default and unknown fields are ignored, so
/// files from older and newer versions still load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSnapshot {
    pub schema_version: u32,
    pub device_id: String,
    /// The latest snapshot of each pet this device has had, by pet id.
    pub pets: BTreeMap<String, PetState>,
    /// Pets not chosen at `WHICH DOT?` (#38). Left out of the merge.
    pub retired_pets: BTreeSet<String>,
    pub counters: Counters,
    pub coins: CoinLedger,
    pub achievements: BTreeSet<String>,
    pub streak: StreakLedger,
    /// Bond day counters by pet id.
    pub bond: BTreeMap<String, BondLedger>,
}

/// This device's lifetime counts (a G-Counter: max per device, sum across devices).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counters {
    pub feeds: u64,
    pub plays: u64,
    pub sleeps: u64,
    /// Revives on this device. `total_pets_raised` is 1 plus the sum.
    pub revives: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoinLedger {
    /// Coins earned on this device, other than passive income and the daily bonus.
    pub earned: u64,
    pub spent: u64,
    pub passive: PassiveLedger,
    /// Daily care bonus paid on each date. Merged by date, so two devices
    /// paying the same day count once.
    pub daily_bonus: BTreeMap<NaiveDate, u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreakLedger {
    /// Dates with care, the last STREAK_WINDOW_D days up to the newest.
    pub care_dates: BTreeSet<NaiveDate>,
    /// The run of consecutive days this device knows of, so a streak longer
    /// than the window (or one from an old save) is not cut short.
    pub run: Option<StreakRun>,
    pub longest: u32,
}

/// `len` consecutive days of care ending on `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StreakRun {
    pub end: NaiveDate,
    pub len: u32,
}

/// Bond for one pet, as day counters to replay through F1.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BondLedger {
    /// Bond already settled when the ledger started (an old `companion.json`,
    /// or the first adult day at 0). Days before `settled_on` are inside it.
    pub base: Option<BondBase>,
    pub days: BTreeMap<NaiveDate, DayCounters>,
    /// Answered YES to `STAY WITH YOU?`.
    pub onboarding_yes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BondBase {
    pub settled_on: NaiveDate,
    pub bond: u16,
    pub overdue_applied: u32,
    pub last_open_date: NaiveDate,
    /// Only the first adult day seen on a device, at Bond 0; not a settlement.
    #[serde(default)]
    pub start: bool,
}

impl BondBase {
    /// A settlement beats a start, the latest settlement wins, and of two
    /// starts the earlier one: a Mac that joins later must not reset Bond.
    fn rank(&self) -> (bool, i32, u16, u32, NaiveDate) {
        let day = self.settled_on.num_days_from_ce();
        (!self.start, if self.start { -day } else { day }, self.bond, self.overdue_applied, self.last_open_date)
    }
}

/// The base to keep of two. Every field is in the rank, so it is a total order.
pub(crate) fn join_base(a: Option<BondBase>, b: Option<BondBase>) -> Option<BondBase> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b.rank() > a.rank() { b } else { a }),
        (a, b) => a.or(b),
    }
}

impl Default for DeviceSnapshot {
    fn default() -> Self {
        Self::new("")
    }
}

impl DeviceSnapshot {
    pub fn new(device_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            device_id: device_id.to_string(),
            pets: BTreeMap::new(),
            retired_pets: BTreeSet::new(),
            counters: Counters::default(),
            coins: CoinLedger::default(),
            achievements: BTreeSet::new(),
            streak: StreakLedger::default(),
            bond: BTreeMap::new(),
        }
    }

    /// This device's share from a save made before sync: everything it holds
    /// was done on this device. Merging the result alone gives back the same
    /// counters, coins, achievements, streak, pet and Bond.
    pub fn from_legacy(
        device_id: &str,
        pet: &PetState,
        stats: &GameStats,
        companion: Option<&CompanionState>,
    ) -> Self {
        let mut snapshot = Self::new(device_id);
        snapshot.record_pet(pet);
        snapshot.counters = Counters {
            feeds: stats.total_feeds,
            plays: stats.total_plays,
            sleeps: stats.total_sleeps,
            revives: stats.total_pets_raised.saturating_sub(1),
        };
        // Passive income and the daily bonus paid so far are not told apart: all of it is `earned`
        let earned = stats.total_coins_earned.max(stats.coins);
        snapshot.coins.earned = earned;
        snapshot.coins.spent = earned - stats.coins;
        if let Some(last) = stats.last_passive_time {
            snapshot.coins.passive = PassiveLedger::starting_at(last);
        }
        snapshot.achievements = stats.unlocked.iter().cloned().collect();
        snapshot.streak = StreakLedger::from_legacy(stats);
        if let Some(companion) = companion {
            snapshot.record_legacy_bond(pet, companion);
        }
        snapshot
    }

    fn record_legacy_bond(&mut self, pet: &PetState, companion: &CompanionState) {
        let Some(settled_on) = companion.last_settle_date else { return };
        let pet_id = companion.pet_id.clone().unwrap_or_else(|| pet.id.clone());
        let ledger = self.bond.entry(pet_id).or_default();
        let base = BondBase {
            settled_on,
            bond: companion.bond,
            overdue_applied: companion.overdue_applied,
            last_open_date: companion.last_open_date.unwrap_or(settled_on),
            start: false,
        };
        ledger.base = join_base(ledger.base, Some(base));
        if companion.today != DayCounters::default() {
            ledger.days.insert(settled_on, companion.today);
        }
        ledger.onboarding_yes = companion.onboarding == Onboarding::Yes;
    }

    /// Keeps `pet` if it is newer than what this device recorded for its id.
    pub fn record_pet(&mut self, pet: &PetState) {
        let device_id = self.device_id.as_str();
        match self.pets.get(&pet.id) {
            Some(kept) if compare_pets(device_id, kept, device_id, pet).is_ge() => {}
            _ => {
                self.pets.insert(pet.id.clone(), pet.clone());
            }
        }
    }

    /// A successful FEED / PLAY / SLEEP on `date` (the date the streak uses).
    pub fn record_care(&mut self, kind: CareKind, date: NaiveDate) {
        let counter = match kind {
            CareKind::Feed => &mut self.counters.feeds,
            CareKind::Play => &mut self.counters.plays,
            CareKind::Sleep => &mut self.counters.sleeps,
        };
        *counter = counter.saturating_add(1);
        self.streak.record(date);
    }

    pub fn record_revive(&mut self) {
        self.counters.revives = self.counters.revives.saturating_add(1);
    }

    /// Coins earned other than passive income and the daily bonus.
    pub fn record_earned(&mut self, amount: u64) {
        self.coins.earned = self.coins.earned.saturating_add(amount);
    }

    pub fn record_spent(&mut self, amount: u64) {
        self.coins.spent = self.coins.spent.saturating_add(amount);
    }

    pub fn record_daily_bonus(&mut self, date: NaiveDate, amount: u64) {
        let paid = self.coins.daily_bonus.entry(date).or_default();
        *paid = (*paid).max(amount);
    }

    /// See `PassiveLedger::record_accrual`.
    pub fn record_passive(&mut self, before: Option<i64>, after: Option<i64>, earned: u64) {
        self.coins.passive.record_accrual(before, after, earned);
    }

    pub fn unlock(&mut self, achievement: &str) {
        self.achievements.insert(achievement.to_string());
    }

    /// The day counters this device has for `pet_id` on `date` so far. Counts
    /// only go up within a day, so a later call replaces an earlier one.
    pub fn record_bond_day(&mut self, pet_id: &str, date: NaiveDate, counters: DayCounters) {
        let day = self.bond.entry(pet_id.to_string()).or_default().days.entry(date).or_default();
        *day = join_day(*day, counters);
    }

    /// Where Bond for `pet_id` starts: the first adult day, at Bond 0.
    pub fn record_bond_start(&mut self, pet_id: &str, date: NaiveDate) {
        let ledger = self.bond.entry(pet_id.to_string()).or_default();
        let start = BondBase { settled_on: date, bond: 0, overdue_applied: 0, last_open_date: date, start: true };
        ledger.base = join_base(ledger.base, Some(start));
    }

    pub fn retire_pet(&mut self, pet_id: &str) {
        self.retired_pets.insert(pet_id.to_string());
    }

    /// Joins another version of this device's file (an older copy, a conflict
    /// copy). Every field keeps the larger value or the union, so joining is
    /// commutative and joining the same file twice changes nothing.
    pub fn absorb(&mut self, other: &DeviceSnapshot) {
        for pet in other.pets.values() {
            self.record_pet(pet);
        }
        self.retired_pets.extend(other.retired_pets.iter().cloned());
        self.counters = Counters {
            feeds: self.counters.feeds.max(other.counters.feeds),
            plays: self.counters.plays.max(other.counters.plays),
            sleeps: self.counters.sleeps.max(other.counters.sleeps),
            revives: self.counters.revives.max(other.counters.revives),
        };
        self.coins.earned = self.coins.earned.max(other.coins.earned);
        self.coins.spent = self.coins.spent.max(other.coins.spent);
        self.coins.passive.absorb(&other.coins.passive);
        for (&date, &amount) in &other.coins.daily_bonus {
            self.record_daily_bonus(date, amount);
        }
        self.achievements.extend(other.achievements.iter().cloned());
        self.streak.absorb(&other.streak);
        for (pet_id, theirs) in &other.bond {
            let ours = self.bond.entry(pet_id.clone()).or_default();
            ours.base = join_base(ours.base, theirs.base);
            for (&date, &counters) in &theirs.days {
                let day = ours.days.entry(date).or_default();
                *day = join_day(*day, counters);
            }
            ours.onboarding_yes |= theirs.onboarding_yes;
        }
    }
}

/// Two versions of one device's counters for one day.
fn join_day(a: DayCounters, b: DayCounters) -> DayCounters {
    DayCounters { g: a.g || b.g, n_care: a.n_care.max(b.n_care), n_focus: a.n_focus.max(b.n_focus) }
}

impl StreakLedger {
    fn from_legacy(stats: &GameStats) -> Self {
        let mut ledger = Self { longest: stats.longest_streak, ..Self::default() };
        let end = stats
            .last_care_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        if let Some(end) = end.filter(|_| stats.current_streak > 0) {
            ledger.run = Some(StreakRun { end, len: stats.current_streak });
            let days = u64::from(stats.current_streak).min(STREAK_WINDOW_D);
            ledger.care_dates = (0..days).filter_map(|back| end.checked_sub_days(Days::new(back))).collect();
        }
        ledger
    }

    /// Care on `date`. A date before the current run (clock set back) only
    /// adds the date.
    pub fn record(&mut self, date: NaiveDate) {
        let run = match self.run {
            Some(run) if date <= run.end => run,
            Some(run) if run.end.succ_opt() == Some(date) => StreakRun { end: date, len: run.len.saturating_add(1) },
            _ => StreakRun { end: date, len: 1 },
        };
        self.run = Some(run);
        self.longest = self.longest.max(run.len);
        self.care_dates.insert(date);
        self.prune();
    }

    fn absorb(&mut self, other: &StreakLedger) {
        self.care_dates.extend(other.care_dates.iter().copied());
        self.prune();
        self.run = self.run.max(other.run);
        self.longest = self.longest.max(other.longest);
    }

    /// Keeps the STREAK_WINDOW_D days up to the newest date. Depends only on
    /// the newest date, so pruning before or after a join gives the same set.
    fn prune(&mut self) {
        let Some(&newest) = self.care_dates.last() else { return };
        if let Some(cutoff) = newest.checked_sub_days(Days::new(STREAK_WINDOW_D - 1)) {
            self.care_dates.retain(|&d| d >= cutoff);
        }
    }
}
