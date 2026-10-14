//! Merging device files (design「マージ規則」). Commutative and idempotent:
//! the same files in any order, read any number of times, give the same result.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use chrono::{Days, NaiveDate};
use serde::Serialize;
use crate::companion::bond::{self, DayCounters};
use crate::pet::PetState;
use super::passive;
use super::snapshot::{join_base, BondBase, BondLedger, DeviceSnapshot};

/// What all devices add up to.
#[derive(Debug, Clone, Serialize)]
pub struct Merged {
    /// The chosen snapshot of each pet, by id. Retired pets are left out.
    pub pets: BTreeMap<String, PetState>,
    pub retired_pets: BTreeSet<String>,
    pub total_feeds: u64,
    pub total_plays: u64,
    pub total_sleeps: u64,
    pub total_pets_raised: u64,
    /// Can be negative when two offline devices spent the same coins.
    pub coin_balance: i64,
    pub total_coins_earned: u64,
    pub passive_slots: u64,
    pub unlocked: BTreeSet<String>,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_care_date: Option<NaiveDate>,
    /// Bond day counters of all devices, by pet id. Retired pets are left out.
    pub bond: BTreeMap<String, BondLedger>,
}

/// Bond for one pet after replaying the merged day counters through F1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BondReplay {
    pub bond: u16,
    pub overdue_applied: u32,
    pub last_settle_date: NaiveDate,
    pub last_open_date: NaiveDate,
    /// Counters of the day not settled yet.
    pub today: DayCounters,
}

pub fn merge<'a>(snapshots: impl IntoIterator<Item = &'a DeviceSnapshot>) -> Merged {
    // Versions of one device's file first collapse into one, so a file read
    // twice (or its conflict copy) is never added twice below
    let devices = join_by_device(snapshots);
    let retired_pets: BTreeSet<String> =
        devices.values().flat_map(|d| d.retired_pets.iter().cloned()).collect();
    let sum = |field: fn(&DeviceSnapshot) -> u64| devices.values().map(field).fold(0u64, u64::saturating_add);

    let passive_slots = passive::union_count(devices.values().map(|d| &d.coins.passive));
    let mut daily_bonus: BTreeMap<NaiveDate, u64> = BTreeMap::new();
    for (&date, &amount) in devices.values().flat_map(|d| &d.coins.daily_bonus) {
        let paid = daily_bonus.entry(date).or_default();
        *paid = (*paid).max(amount);
    }
    let total_coins_earned = sum(|d| d.coins.earned)
        .saturating_add(passive_slots)
        .saturating_add(daily_bonus.values().fold(0, |a, &b| a.saturating_add(b)));
    let balance = i128::from(total_coins_earned) - i128::from(sum(|d| d.coins.spent));
    let streak = merge_streak(&devices);

    Merged {
        pets: choose_pets(&devices, &retired_pets),
        total_feeds: sum(|d| d.counters.feeds),
        total_plays: sum(|d| d.counters.plays),
        total_sleeps: sum(|d| d.counters.sleeps),
        total_pets_raised: sum(|d| d.counters.revives).saturating_add(1),
        coin_balance: balance.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        total_coins_earned,
        passive_slots,
        unlocked: devices.values().flat_map(|d| d.achievements.iter().cloned()).collect(),
        current_streak: streak.current,
        longest_streak: streak.longest,
        last_care_date: streak.last,
        bond: merge_bond(&devices, &retired_pets),
        retired_pets,
    }
}

fn join_by_device<'a>(snapshots: impl IntoIterator<Item = &'a DeviceSnapshot>) -> BTreeMap<String, DeviceSnapshot> {
    let mut devices: BTreeMap<String, DeviceSnapshot> = BTreeMap::new();
    for snapshot in snapshots {
        devices
            .entry(snapshot.device_id.clone())
            .or_insert_with(|| DeviceSnapshot::new(&snapshot.device_id))
            .absorb(snapshot);
    }
    devices
}

/// Orders two snapshots of the same pet; the greater one is kept.
/// The time of the last care decides, not `last_update`: a device left running
/// moves `last_update` every 5 seconds without anyone caring for the pet.
pub(crate) fn compare_pets(a_device: &str, a: &PetState, b_device: &str, b: &PetState) -> Ordering {
    event_time(a)
        .cmp(&event_time(b))
        // A death and a care in the same second: the pet could not be cared for, so the death stands
        .then_with(|| (!a.is_alive).cmp(&!b.is_alive))
        .then_with(|| a.present_secs_since_action.cmp(&b.present_secs_since_action))
        // The smaller device id wins
        .then_with(|| b_device.cmp(a_device))
        // Only versions from the same device get this far
        .then_with(|| a.last_update.cmp(&b.last_update))
        .then_with(|| a.age.cmp(&b.age))
        .then_with(|| canonical_json(a).cmp(&canonical_json(b)))
}

/// A living pet counts from its last care; a dead one from its death, so care
/// after the death on another device (which had not heard of it) wins.
fn event_time(pet: &PetState) -> i64 {
    if pet.is_alive {
        pet.last_action_at
    } else {
        // An update stops at the death, so `last_update` is when it died
        pet.last_update.max(pet.last_action_at)
    }
}

fn canonical_json(pet: &PetState) -> String {
    serde_json::to_string(pet).unwrap_or_default()
}

/// The greatest snapshot of each pet, with the largest age any device saw:
/// growth never goes back.
fn choose_pets(devices: &BTreeMap<String, DeviceSnapshot>, retired: &BTreeSet<String>) -> BTreeMap<String, PetState> {
    let mut chosen: BTreeMap<&str, (&str, &PetState)> = BTreeMap::new();
    let mut age: BTreeMap<&str, u64> = BTreeMap::new();
    for (device_id, device) in devices {
        for pet in device.pets.values().filter(|p| !retired.contains(&p.id)) {
            let oldest = age.entry(&pet.id).or_default();
            *oldest = (*oldest).max(pet.age);
            let replace = chosen
                .get(pet.id.as_str())
                .is_none_or(|&(kept_device, kept)| compare_pets(device_id, pet, kept_device, kept).is_gt());
            if replace {
                chosen.insert(&pet.id, (device_id, pet));
            }
        }
    }
    chosen
        .into_iter()
        .map(|(id, (_, pet))| {
            let mut pet = pet.clone();
            pet.age = pet.age.max(age[id]);
            (id.to_string(), pet)
        })
        .collect()
}

struct Streak {
    current: u32,
    longest: u32,
    last: Option<NaiveDate>,
}

/// Recounts the streak from the care dates of all devices, plus each device's
/// run so a streak older than the date window is kept.
fn merge_streak(devices: &BTreeMap<String, DeviceSnapshot>) -> Streak {
    let mut spans: Vec<(NaiveDate, NaiveDate)> = Vec::new();
    let mut longest = 0;
    for streak in devices.values().map(|d| &d.streak) {
        longest = longest.max(streak.longest);
        spans.extend(streak.care_dates.iter().map(|&date| (date, date)));
        if let Some(run) = streak.run.filter(|run| run.len > 0) {
            let start = run.end.checked_sub_days(Days::new(u64::from(run.len - 1))).unwrap_or(NaiveDate::MIN);
            spans.push((start, run.end));
        }
    }
    spans.sort_unstable();
    let mut joined: Vec<(NaiveDate, NaiveDate)> = Vec::new();
    for (start, end) in spans {
        match joined.last_mut() {
            Some(last) if last.1.succ_opt().is_none_or(|next| start <= next) => last.1 = last.1.max(end),
            _ => joined.push((start, end)),
        }
    }
    let days = |&(start, end): &(NaiveDate, NaiveDate)| u32::try_from((end - start).num_days() + 1).unwrap_or(u32::MAX);
    Streak {
        // Spans are sorted and apart, so the last one holds the newest care
        current: joined.last().map_or(0, days),
        longest: joined.iter().map(days).fold(longest, u32::max),
        last: joined.last().map(|&(_, end)| end),
    }
}

/// Day counters of all devices, per pet: opened on any device counts once,
/// care and focus add up. The daily cap is applied when replaying.
fn merge_bond(devices: &BTreeMap<String, DeviceSnapshot>, retired: &BTreeSet<String>) -> BTreeMap<String, BondLedger> {
    let mut merged: BTreeMap<String, BondLedger> = BTreeMap::new();
    for (pet_id, ledger) in devices.values().flat_map(|d| &d.bond).filter(|(id, _)| !retired.contains(*id)) {
        let into = merged.entry(pet_id.clone()).or_default();
        into.base = join_base(into.base, ledger.base);
        for (&date, counters) in &ledger.days {
            let day = into.days.entry(date).or_default();
            day.g |= counters.g;
            day.n_care = day.n_care.saturating_add(counters.n_care);
            day.n_focus = day.n_focus.saturating_add(counters.n_focus);
        }
        into.onboarding_yes |= ledger.onboarding_yes;
    }
    merged
}

/// Replays F1 from the ledger's base up to `today`, settling on each day that
/// has counters and once more on `today`, the way a single device settles on
/// the first update of a day after being closed. F1's difference rule makes
/// this the same Bond as settling every day (apart from where a tier floor
/// is crossed mid-gap), and it loops only over recorded days.
pub fn replay_bond(ledger: &BondLedger, today: NaiveDate) -> Option<BondReplay> {
    let base = ledger.base.or_else(|| {
        let first = *ledger.days.keys().next()?;
        Some(BondBase { settled_on: first, bond: 0, overdue_applied: 0, last_open_date: first, start: true })
    })?;
    let mut bond_value = base.bond;
    let mut overdue_applied = base.overdue_applied;
    let mut last_open = base.last_open_date;
    let mut settled_on = base.settled_on;
    while settled_on < today {
        let counters = ledger.days.get(&settled_on).copied().unwrap_or_default();
        if counters.g {
            last_open = last_open.max(settled_on);
            overdue_applied = 0;
        }
        let next = settled_on
            .succ_opt()
            .and_then(|after| ledger.days.range(after..today).next().map(|(&day, _)| day))
            .unwrap_or(today);
        let settled = bond::settle(bond_value, &counters, (next - last_open).num_days(), overdue_applied);
        bond_value = settled.bond;
        overdue_applied = settled.overdue_applied;
        settled_on = next;
    }
    // A clock set back to before the base changes nothing
    let pending = if today == settled_on { ledger.days.get(&today).copied().unwrap_or_default() } else { DayCounters::default() };
    if pending.g {
        last_open = last_open.max(today);
        overdue_applied = 0;
    }
    Some(BondReplay {
        bond: bond_value,
        overdue_applied,
        last_settle_date: settled_on,
        last_open_date: last_open,
        today: pending,
    })
}

impl Merged {
    /// A negative balance shows as 0.
    pub fn coins_display(&self) -> u64 {
        u64::try_from(self.coin_balance).unwrap_or(0)
    }

    /// Buying stops while the balance is below the price, including while it is negative.
    pub fn can_spend(&self, amount: u64) -> bool {
        i128::from(self.coin_balance) >= i128::from(amount)
    }

    pub fn living_pets(&self) -> Vec<&PetState> {
        self.pets.values().filter(|pet| pet.is_alive).collect()
    }

    /// More than one living pet: the player picks one (`WHICH DOT?`, #38).
    pub fn needs_choice(&self) -> bool {
        self.living_pets().len() > 1
    }

    pub fn bond_on(&self, pet_id: &str, today: NaiveDate) -> Option<BondReplay> {
        self.bond.get(pet_id).and_then(|ledger| replay_bond(ledger, today))
    }
}
