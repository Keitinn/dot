//! Stage 0: two Macs simulated with two folders (design Acceptance Criteria,
//! 複数端末). Times are fixed; nothing reads the clock.

use std::fs;
use std::path::{Path, PathBuf};
use chrono::NaiveDate;
use crate::companion::bond::DayCounters;
use crate::companion::{CareKind, CompanionState};
use crate::pet::{EvolutionStage, GameStats, PetState};
use super::device::load_or_create_in;
use super::folder::{write_own, CONFLICTS_DIR, DEVICES_DIR};
use super::merge::{merge, Merged};
use super::snapshot::{BondBase, DeviceSnapshot};
use super::SyncCache;

const T0: i64 = 1_700_000_000;
const HOUR: i64 = 3600;
const K0: i64 = 2_833_333;
const ID_A: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const ID_B: &str = "bbbbbbbb-0000-4000-8000-000000000002";
const ID_C: &str = "cccccccc-0000-4000-8000-000000000003";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("dot-sync-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

fn json(merged: &Merged) -> serde_json::Value {
    serde_json::to_value(merged).unwrap()
}

fn json_of_pet(merged: &Merged, id: &str) -> serde_json::Value {
    serde_json::to_value(&merged.pets[id]).unwrap()
}

/// A pet with `id`, last cared for at `last_action_at`, present `present`
/// seconds since, updated last at `last_update`.
fn pet(id: &str, last_action_at: i64, present: u64, last_update: i64) -> PetState {
    let mut pet = PetState::new();
    pet.id = id.to_string();
    pet.birth_time = T0 - 24 * HOUR;
    pet.last_action_at = last_action_at;
    pet.present_secs_since_action = present;
    pet.last_update = last_update;
    pet
}

fn dead(mut pet: PetState, died_at: i64) -> PetState {
    pet.is_alive = false;
    pet.health = 0;
    pet.last_update = died_at;
    pet
}

fn day(g: bool, n_care: u32, n_focus: u32) -> DayCounters {
    DayCounters { g, n_care, n_focus }
}

fn feeds(snapshot: &mut DeviceSnapshot, times: usize, on: NaiveDate) {
    for _ in 0..times {
        snapshot.record_care(CareKind::Feed, on);
    }
}

/// Device A: fed pet X at T0+10h, paid two hours of passive income, cared for
/// three days running.
fn device_a() -> DeviceSnapshot {
    let mut a = DeviceSnapshot::new(ID_A);
    a.record_pet(&pet("pet-x", T0 + 10 * HOUR, 100, T0 + 10 * HOUR + 100));
    feeds(&mut a, 1, date(12));
    feeds(&mut a, 1, date(13));
    feeds(&mut a, 1, date(14));
    a.record_earned(40);
    a.record_spent(5);
    a.record_passive(Some(K0 * 600), Some((K0 + 12) * 600), 12);
    a.record_daily_bonus(date(14), 3);
    a.unlock("first_feed");
    a.record_bond_start("pet-x", date(10));
    a.record_bond_day("pet-x", date(14), day(true, 5, 2));
    a
}

/// An older version of A's file: fewer feeds, an earlier snapshot of X.
fn device_a_old() -> DeviceSnapshot {
    let mut a = DeviceSnapshot::new(ID_A);
    a.record_pet(&pet("pet-x", T0 + 8 * HOUR, 50, T0 + 8 * HOUR + 50));
    feeds(&mut a, 2, date(12));
    a.record_earned(30);
    a.record_passive(Some(K0 * 600), Some((K0 + 6) * 600), 6);
    a.record_bond_start("pet-x", date(10));
    a.record_bond_day("pet-x", date(14), day(true, 2, 0));
    a
}

/// Device B: fed X at T0+9h, then kept running (last_update moves on), and
/// paid passive income for an hour overlapping A's.
fn device_b() -> DeviceSnapshot {
    let mut b = DeviceSnapshot::new(ID_B);
    b.record_pet(&pet("pet-x", T0 + 9 * HOUR, 9_999, T0 + 20 * HOUR));
    feeds(&mut b, 2, date(15));
    b.record_earned(25);
    b.record_passive(Some((K0 + 6) * 600), Some((K0 + 18) * 600), 12);
    b.record_daily_bonus(date(14), 3);
    b.record_daily_bonus(date(15), 4);
    b.unlock("streak_7");
    b.unlock("first_feed");
    b.record_bond_day("pet-x", date(14), day(true, 5, 2));
    b.record_bond_day("pet-x", date(15), day(true, 1, 0));
    b
}

/// Device C: X died here at T0+11h and C revived a new pet Y; spent a lot.
fn device_c() -> DeviceSnapshot {
    let mut c = DeviceSnapshot::new(ID_C);
    c.record_pet(&dead(pet("pet-x", T0 + 7 * HOUR, 500, T0 + 7 * HOUR), T0 + 11 * HOUR));
    c.record_revive();
    c.record_pet(&pet("pet-y", T0 + 12 * HOUR, 0, T0 + 12 * HOUR));
    c.record_spent(100);
    c.record_care(CareKind::Play, date(16));
    c.record_bond_start("pet-y", date(16));
    c
}

fn assert_commutative_and_idempotent(a: &DeviceSnapshot, b: &DeviceSnapshot) {
    let ab = json(&merge([a, b]));
    assert_eq!(ab, json(&merge([b, a])), "A,B vs B,A");
    assert_eq!(ab, json(&merge([a, b, b])), "A,B vs A,B,B");
    assert_eq!(ab, json(&merge([a, a, b])), "A,B vs A,A,B");
}

// --- Commutative and idempotent ---

#[test]
fn merge_is_commutative_and_idempotent_for_every_pair() {
    let all = [device_a(), device_a_old(), device_b(), device_c()];
    for a in &all {
        for b in &all {
            assert_commutative_and_idempotent(a, b);
        }
    }
}

#[test]
fn merge_of_three_does_not_depend_on_order() {
    let (a, b, c) = (device_a(), device_b(), device_c());
    let expected = json(&merge([&a, &b, &c]));
    for order in [[&a, &c, &b], [&b, &a, &c], [&b, &c, &a], [&c, &a, &b], [&c, &b, &a]] {
        assert_eq!(json(&merge(order)), expected);
    }
    assert_eq!(json(&merge([&a, &b, &c, &c, &a, &b])), expected);
}

#[test]
fn merging_a_file_with_itself_changes_nothing() {
    let a = device_a();
    let alone = json(&merge([&a]));
    assert_eq!(json(&merge([&a, &a, &a])), alone);
    let mut absorbed = a.clone();
    absorbed.absorb(&a);
    assert_eq!(json(&merge([&absorbed])), alone);
}

// --- Acceptance Criteria (複数端末) ---

#[test]
fn ac_same_two_hours_of_passive_income_on_two_devices_pay_twelve() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_passive(Some(K0 * 600), Some((K0 + 12) * 600), 12);
    b.record_passive(Some(K0 * 600), Some((K0 + 12) * 600), 12);

    let merged = merge([&a, &b]);

    assert_eq!(merged.passive_slots, 12);
    assert_eq!(merged.coins_display(), 12);
    assert_eq!(merged.total_coins_earned, 12);
}

#[test]
fn ac_feeds_on_two_devices_add_up_and_rereading_does_not_add_more() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    feeds(&mut a, 3, date(14));
    feeds(&mut b, 2, date(14));

    assert_eq!(merge([&a, &b]).total_feeds, 5);
    assert_eq!(merge([&a, &b, &a, &b]).total_feeds, 5);
}

#[test]
fn ac_later_care_wins_over_a_device_that_only_kept_updating() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_pet(&pet("pet-x", T0 + 10 * HOUR, 0, T0 + 10 * HOUR));
    // B fed at 9:00 and then polled every 5 seconds until much later
    b.record_pet(&pet("pet-x", T0 + 9 * HOUR, 6 * HOUR as u64, T0 + 15 * HOUR));

    let merged = merge([&a, &b]);

    let chosen = &merged.pets["pet-x"];
    assert_eq!(chosen.last_action_at, T0 + 10 * HOUR);
    assert_eq!(chosen.last_update, T0 + 10 * HOUR);
}

#[test]
fn ac_broken_device_file_is_skipped_and_the_last_good_one_is_used() {
    let shared = TempDir::new();
    let mut b = DeviceSnapshot::new(ID_B);
    feeds(&mut b, 2, date(14));
    write_own(shared.path(), &device_a()).unwrap();
    write_own(shared.path(), &b).unwrap();
    let mut cache = SyncCache::default();
    cache.read_folder(shared.path());
    let before = cache.merged();

    // A's file is caught half written; meanwhile B fed once more
    let a_file = shared.path().join(DEVICES_DIR).join(format!("{}.json", ID_A));
    fs::write(&a_file, "{\"device_id\": \"aaaa").unwrap();
    feeds(&mut b, 1, date(14));
    write_own(shared.path(), &b).unwrap();
    let report = cache.read_folder(shared.path());

    assert_eq!(report.skipped, vec![a_file.clone()]);
    let merged = cache.merged();
    assert_eq!(merged.pets.keys().collect::<Vec<_>>(), vec!["pet-x"]);
    assert_eq!(json_of_pet(&merged, "pet-x"), json_of_pet(&before, "pet-x"));
    assert_eq!(merged.total_feeds, 3 + 3);
    assert_eq!(merged.coins_display(), before.coins_display());
    // Someone else's file is left where it is, not moved aside
    assert_eq!(fs::read_to_string(&a_file).unwrap(), "{\"device_id\": \"aaaa");
}

#[test]
fn ac_older_conflict_copy_does_not_double_and_moves_to_conflicts() {
    let shared = TempDir::new();
    let devices = shared.path().join(DEVICES_DIR);
    write_own(shared.path(), &device_a()).unwrap();
    let copy_name = format!("{} 2.json", ID_A);
    fs::write(devices.join(&copy_name), serde_json::to_string(&device_a_old()).unwrap()).unwrap();

    let mut cache = SyncCache::default();
    let report = cache.read_folder(shared.path());

    assert_eq!(json(&cache.merged()), json(&merge([&device_a()])));
    assert_eq!(cache.merged().total_feeds, 3);
    let moved = shared.path().join(CONFLICTS_DIR).join(&copy_name);
    assert_eq!(report.moved_to_conflicts, vec![moved.clone()]);
    assert!(moved.exists());
    assert!(!devices.join(&copy_name).exists());
    assert!(devices.join(format!("{}.json", ID_A)).exists());

    // Reading again finds nothing new
    let again = cache.read_folder(shared.path());
    assert!(again.moved_to_conflicts.is_empty());
    assert_eq!(cache.merged().total_feeds, 3);
}

#[test]
fn ac_same_day_bond_counters_on_two_devices_gain_at_most_g_cap() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    for device in [&mut a, &mut b] {
        device.record_bond_start("pet-x", date(1));
        device.record_bond_day("pet-x", date(1), day(true, 5, 2));
    }

    let merged = merge([&a, &b]);

    assert_eq!(merged.bond["pet-x"].days[&date(1)], day(true, 10, 4));
    assert_eq!(merged.bond_on("pet-x", date(2)).unwrap().bond, 50);
}

#[test]
fn ac_first_sync_with_two_living_pets_asks_and_adds_coins() {
    let x = pet("pet-x", T0 + HOUR, 10, T0 + 2 * HOUR);
    let y = pet("pet-y", T0 + 3 * HOUR, 20, T0 + 4 * HOUR);
    let mut stats_a = GameStats::new();
    stats_a.add_coins(40);
    let mut stats_b = GameStats::new();
    stats_b.add_coins(25);
    let a = DeviceSnapshot::from_legacy(ID_A, &x, &stats_a, None);
    let mut b = DeviceSnapshot::from_legacy(ID_B, &y, &stats_b, None);

    let merged = merge([&a, &b]);

    assert!(merged.needs_choice());
    assert_eq!(merged.living_pets().len(), 2);
    // Until someone chooses, neither pet changes
    assert_eq!(json_of_pet(&merged, "pet-x"), serde_json::to_value(&x).unwrap());
    assert_eq!(json_of_pet(&merged, "pet-y"), serde_json::to_value(&y).unwrap());
    assert_eq!(merged.coins_display(), 65);

    // B chooses X: Y is retired and B carries on with X
    b.retire_pet("pet-y");
    b.record_pet(&x);
    let chosen = merge([&a, &b]);

    assert!(!chosen.needs_choice());
    assert_eq!(chosen.pets.keys().collect::<Vec<_>>(), vec!["pet-x"]);
    assert!(chosen.retired_pets.contains("pet-y"));
    assert_eq!(chosen.coins_display(), 65);
}

// --- Choosing the pet snapshot ---

#[test]
fn same_care_time_picks_the_longer_present_time() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_pet(&pet("pet-x", T0, 600, T0 + 600));
    b.record_pet(&pet("pet-x", T0, 1_800, T0 + 7_200));

    assert_eq!(merge([&a, &b]).pets["pet-x"].present_secs_since_action, 1_800);
}

#[test]
fn full_tie_picks_the_smaller_device_id() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    let mut from_a = pet("pet-x", T0, 600, T0 + 600);
    from_a.hunger = 11;
    let mut from_b = pet("pet-x", T0, 600, T0 + 900);
    from_b.hunger = 22;
    a.record_pet(&from_a);
    b.record_pet(&from_b);

    assert_eq!(merge([&b, &a]).pets["pet-x"].hunger, 11);
}

#[test]
fn death_after_the_last_care_elsewhere_stands() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_pet(&dead(pet("pet-x", T0, 0, T0), T0 + 15 * HOUR));
    b.record_pet(&pet("pet-x", T0 + 9 * HOUR, 60, T0 + 20 * HOUR));

    let merged = merge([&a, &b]);

    assert!(!merged.pets["pet-x"].is_alive);
    assert!(!merged.needs_choice());
}

#[test]
fn care_after_the_death_on_another_device_keeps_the_pet_alive() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_pet(&dead(pet("pet-x", T0, 0, T0), T0 + 15 * HOUR));
    a.record_revive();
    a.record_pet(&pet("pet-y", T0 + 15 * HOUR + 60, 0, T0 + 15 * HOUR + 60));
    // B had not heard of the death and fed the old pet later
    b.record_pet(&pet("pet-x", T0 + 16 * HOUR, 0, T0 + 16 * HOUR));

    let merged = merge([&a, &b]);

    assert!(merged.pets["pet-x"].is_alive);
    assert_eq!(merged.pets["pet-x"].last_action_at, T0 + 16 * HOUR);
    // Two living pets again: the player chooses (WHICH DOT?)
    assert!(merged.needs_choice());
    assert_eq!(merged.total_pets_raised, 2);
}

#[test]
fn death_and_care_in_the_same_second_keep_the_death() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_pet(&dead(pet("pet-x", T0, 0, T0), T0 + HOUR));
    b.record_pet(&pet("pet-x", T0 + HOUR, 0, T0 + HOUR));

    assert!(!merge([&a, &b]).pets["pet-x"].is_alive);
}

#[test]
fn age_is_the_largest_any_device_saw() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    let mut cared = pet("pet-x", T0 + 2 * HOUR, 0, T0 + 2 * HOUR);
    cared.age = 1_000;
    let mut older = pet("pet-x", T0 + HOUR, 0, T0 + 5 * HOUR);
    older.age = 9_000;
    a.record_pet(&cared);
    b.record_pet(&older);

    let chosen = &merge([&a, &b]).pets["pet-x"];

    assert_eq!(chosen.last_action_at, T0 + 2 * HOUR);
    assert_eq!(chosen.age, 9_000);
}

// --- Coins, achievements, streak ---

#[test]
fn coins_add_earnings_and_passive_slots_minus_spending() {
    let merged = merge([&device_a(), &device_b()]);
    // earned 40 + 25, passive slots K0..K0+18, bonus 3 (Oct 14 once) + 4, spent 5
    assert_eq!(merged.passive_slots, 18);
    assert_eq!(merged.total_coins_earned, 65 + 18 + 7);
    assert_eq!(merged.coin_balance, 65 + 18 + 7 - 5);
    assert_eq!(merged.coins_display(), 85);
}

#[test]
fn overspending_shows_zero_and_blocks_buying() {
    let mut stats = GameStats::new();
    stats.add_coins(10);
    let mut a = DeviceSnapshot::from_legacy(ID_A, &pet("pet-x", T0, 0, T0), &stats, None);
    let mut b = DeviceSnapshot::new(ID_B);
    // Both saw 10 coins while offline and spent them
    a.record_spent(10);
    b.record_spent(10);

    let merged = merge([&a, &b]);

    assert_eq!(merged.coin_balance, -10);
    assert_eq!(merged.coins_display(), 0);
    assert!(!merged.can_spend(1));
    // Nothing is bought until the balance is back above zero
    assert!(!merged.can_spend(0));

    b.record_earned(15);
    let recovered = merge([&a, &b]);
    assert_eq!(recovered.coins_display(), 5);
    assert!(recovered.can_spend(5));
    assert!(!recovered.can_spend(6));
}

#[test]
fn achievements_are_the_union() {
    let merged = merge([&device_a(), &device_b()]);
    assert_eq!(merged.unlocked.iter().collect::<Vec<_>>(), vec!["first_feed", "streak_7"]);
}

#[test]
fn streak_counts_days_across_devices() {
    let merged = merge([&device_a(), &device_b()]);
    // A: Oct 12, 13, 14. B: Oct 15
    assert_eq!(merged.current_streak, 4);
    assert_eq!(merged.longest_streak, 4);
    assert_eq!(merged.last_care_date, Some(date(15)));
}

#[test]
fn streak_gap_restarts_but_longest_stays() {
    let mut a = DeviceSnapshot::new(ID_A);
    for d in [1, 2, 3, 4, 5] {
        a.record_care(CareKind::Feed, date(d));
    }
    let mut b = DeviceSnapshot::new(ID_B);
    b.record_care(CareKind::Feed, date(8));
    b.record_care(CareKind::Feed, date(9));

    let merged = merge([&a, &b]);

    assert_eq!(merged.current_streak, 2);
    assert_eq!(merged.longest_streak, 5);
}

#[test]
fn care_dates_keep_only_the_window_but_the_run_keeps_the_streak() {
    let mut a = DeviceSnapshot::new(ID_A);
    for back in 0..100u64 {
        a.record_care(CareKind::Feed, date(1) + chrono::Days::new(back));
    }
    assert_eq!(a.streak.care_dates.len(), 60);
    assert_eq!(merge([&a]).current_streak, 100);
}

// --- Bond replay ---

fn adult() -> PetState {
    let mut pet = PetState::new();
    pet.stage = EvolutionStage::Adult;
    pet
}

#[test]
fn bond_replay_matches_the_companion_settling_alone() {
    let pet = adult();
    let mut companion = CompanionState::default();
    companion.advance_day(&pet, date(1));
    companion.bond = 280;
    let mut snapshot = DeviceSnapshot::from_legacy(ID_A, &pet, &GameStats::new(), Some(&companion));

    // F1 example: closed until d5 (Oct 6)
    companion.advance_day(&pet, date(6));
    let replay = merge([&snapshot]).bond_on(&pet.id, date(6)).unwrap();
    assert_eq!((replay.bond, replay.overdue_applied), (264, 2));
    assert_eq!(replay.bond, companion.bond);

    // Greeting, one FEED (hunger < 80) and one focus on d5: 292 on d6
    companion.record_open(&pet, T0, date(6));
    companion.record_care(&pet, 50, date(6));
    companion.today.n_focus = 1;
    snapshot.record_bond_day(&pet.id, date(6), companion.today);
    companion.advance_day(&pet, date(7));
    let replay = merge([&snapshot]).bond_on(&pet.id, date(7)).unwrap();
    assert_eq!(replay.bond, 292);
    assert_eq!(replay.bond, companion.bond);
    assert_eq!(replay.last_open_date, date(6));
}

#[test]
fn bond_replay_of_a_migrated_save_keeps_its_bond_today() {
    let pet = adult();
    let mut companion = CompanionState::default();
    companion.advance_day(&pet, date(13));
    companion.bond = 321;
    companion.record_care(&pet, 10, date(13));
    let snapshot = DeviceSnapshot::from_legacy(ID_A, &pet, &GameStats::new(), Some(&companion));

    let replay = merge([&snapshot]).bond_on(&pet.id, date(13)).unwrap();

    assert_eq!(replay.bond, 321);
    assert_eq!(replay.today, companion.today);
    assert_eq!(replay.last_settle_date, date(13));
}

#[test]
fn bond_replay_running_every_day_matches_the_f1_example() {
    let mut a = DeviceSnapshot::new(ID_A);
    a.bond.entry("pet-x".to_string()).or_default().base = Some(BondBase {
        settled_on: date(1),
        bond: 280,
        overdue_applied: 0,
        last_open_date: date(1),
    });
    let merged = merge([&a]);
    for (d, expected) in [(2, 280), (3, 280), (4, 280), (5, 272), (6, 264)] {
        assert_eq!(merged.bond_on("pet-x", date(d)).unwrap().bond, expected, "Oct {}", d);
    }
}

#[test]
fn bond_replay_with_the_clock_set_back_changes_nothing() {
    let mut a = DeviceSnapshot::new(ID_A);
    a.record_bond_start("pet-x", date(10));
    a.record_bond_day("pet-x", date(10), day(true, 1, 0));
    let replay = merge([&a]).bond_on("pet-x", date(8)).unwrap();
    assert_eq!(replay.bond, 0);
    assert_eq!(replay.last_settle_date, date(10));
}

#[test]
fn opening_on_either_device_answers_onboarding_and_greets_once() {
    let mut a = DeviceSnapshot::new(ID_A);
    let mut b = DeviceSnapshot::new(ID_B);
    a.record_bond_start("pet-x", date(1));
    a.record_bond_day("pet-x", date(1), day(true, 0, 0));
    b.record_bond_day("pet-x", date(1), day(true, 0, 0));
    b.bond.get_mut("pet-x").unwrap().onboarding_yes = true;

    let merged = merge([&a, &b]);

    assert!(merged.bond["pet-x"].onboarding_yes);
    assert_eq!(merged.bond_on("pet-x", date(2)).unwrap().bond, 10);
}

// --- Old saves ---

const OLD_PET_JSON: &str = r#"{"id":"pet-old","name":"mochi","stage":"Child","age":7200,
    "hunger":61,"happiness":72,"energy":83,"health":94,"is_alive":true,
    "birth_time":1699990000,"last_update":1700000000,"activity":"Idle",
    "activity_until":null,"last_feed_time":1699999000,"last_play_time":null}"#;

const OLD_STATS_JSON: &str = r#"{"total_feeds":12,"total_plays":5,"total_sleeps":2,"total_pets_raised":3,
    "current_streak":4,"longest_streak":9,"last_care_date":"2026-10-13",
    "unlocked":["first_feed","first_hatch"],"coins":37,"total_coins_earned":120,
    "last_passive_time":1700000400}"#;

#[test]
fn old_save_migrates_without_changing_any_value() {
    let pet: PetState = serde_json::from_str(OLD_PET_JSON).unwrap();
    let stats: GameStats = serde_json::from_str(OLD_STATS_JSON).unwrap();
    assert_eq!((pet.last_action_at, pet.present_secs_since_action), (0, 0));

    let snapshot = DeviceSnapshot::from_legacy(ID_A, &pet, &stats, None);
    let merged = merge([&snapshot]);

    assert_eq!(json_of_pet(&merged, "pet-old"), serde_json::to_value(&pet).unwrap());
    assert_eq!(merged.total_feeds, 12);
    assert_eq!(merged.total_plays, 5);
    assert_eq!(merged.total_sleeps, 2);
    assert_eq!(merged.total_pets_raised, 3);
    assert_eq!(merged.coins_display(), 37);
    assert_eq!(merged.total_coins_earned, 120);
    assert_eq!(merged.unlocked.iter().collect::<Vec<_>>(), vec!["first_feed", "first_hatch"]);
    assert_eq!(merged.current_streak, 4);
    assert_eq!(merged.longest_streak, 9);
    assert_eq!(merged.last_care_date, Some(date(13)));
    // Passive income continues from the next whole slot: ceil(1700000400 / 600)
    assert_eq!(snapshot.coins.passive.first_slot, Some(2_833_334));
    assert_eq!(merged.passive_slots, 0);
}

#[test]
fn oldest_stats_without_coins_migrate_to_zero_coins() {
    let stats: GameStats = serde_json::from_str(
        r#"{"total_feeds":3,"total_plays":1,"total_sleeps":0,"total_pets_raised":1,
            "current_streak":0,"longest_streak":2,"last_care_date":"2026-10-01","unlocked":[]}"#,
    )
    .unwrap();
    let merged = merge([&DeviceSnapshot::from_legacy(ID_A, &PetState::new(), &stats, None)]);
    assert_eq!(merged.coins_display(), 0);
    assert_eq!(merged.total_pets_raised, 1);
    assert_eq!(merged.longest_streak, 2);
    assert_eq!(merged.current_streak, 0);
}

#[test]
fn a_streak_longer_than_the_window_survives_migration() {
    let mut stats = GameStats::new();
    stats.current_streak = 100;
    stats.longest_streak = 100;
    stats.last_care_date = Some("2026-10-13".to_string());
    let snapshot = DeviceSnapshot::from_legacy(ID_A, &PetState::new(), &stats, None);
    assert_eq!(snapshot.streak.care_dates.len(), 60);
    assert_eq!(merge([&snapshot]).current_streak, 100);
}

#[test]
fn files_from_a_newer_version_load_and_ignore_unknown_fields() {
    let mut value = serde_json::to_value(device_a()).unwrap();
    value["schema_version"] = serde_json::json!(99);
    value["memories"] = serde_json::json!([{"id": "first_snow"}]);
    value["counters"]["focus_runs"] = serde_json::json!(4);
    let read: DeviceSnapshot = serde_json::from_value(value).unwrap();
    assert_eq!(json(&merge([&read])), json(&merge([&device_a()])));

    let sparse: DeviceSnapshot = serde_json::from_str(&format!(r#"{{"device_id":"{}"}}"#, ID_B)).unwrap();
    assert_eq!(merge([&sparse]).total_pets_raised, 1);
}

// --- Two Macs, two folders ---

fn copy_device_file(from: &TempDir, to: &TempDir, id: &str) {
    let name = format!("{}.json", id);
    fs::copy(from.path().join(DEVICES_DIR).join(&name), to.path().join(DEVICES_DIR).join(&name)).unwrap();
}

#[test]
fn two_macs_with_their_own_folders_end_in_the_same_state() {
    let (home, work) = (TempDir::new(), TempDir::new());
    let id_home = load_or_create_in(home.path(), "HOME-HW").unwrap().device_id;
    let id_work = load_or_create_in(work.path(), "WORK-HW").unwrap().device_id;
    assert_ne!(id_home, id_work);

    let mut on_home = DeviceSnapshot::new(&id_home);
    feeds(&mut on_home, 3, date(14));
    on_home.record_pet(&pet("pet-x", T0 + 10 * HOUR, 0, T0 + 10 * HOUR));
    let mut on_work = DeviceSnapshot::new(&id_work);
    feeds(&mut on_work, 2, date(14));
    on_work.record_pet(&pet("pet-x", T0 + 9 * HOUR, 3_600, T0 + 12 * HOUR));
    write_own(home.path(), &on_home).unwrap();
    write_own(work.path(), &on_work).unwrap();

    // The sync service copies each file to the other folder
    copy_device_file(&home, &work, &id_home);
    copy_device_file(&work, &home, &id_work);
    let (mut seen_home, mut seen_work) = (SyncCache::default(), SyncCache::default());
    seen_home.read_folder(home.path());
    seen_work.read_folder(work.path());

    assert_eq!(json(&seen_home.merged()), json(&seen_work.merged()));
    assert_eq!(seen_home.merged().total_feeds, 5);
    assert_eq!(seen_home.merged().pets["pet-x"].last_action_at, T0 + 10 * HOUR);
}

#[test]
fn own_file_is_written_atomically_without_backups() {
    let shared = TempDir::new();
    let a = device_a();
    let path = write_own(shared.path(), &a).unwrap();
    write_own(shared.path(), &a).unwrap();
    assert_eq!(path, shared.path().join(DEVICES_DIR).join(format!("{}.json", ID_A)));
    let names: Vec<String> = fs::read_dir(shared.path().join(DEVICES_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec![format!("{}.json", ID_A)]);
}

#[test]
fn an_unsafe_device_id_is_not_written() {
    let shared = TempDir::new();
    assert!(write_own(shared.path(), &DeviceSnapshot::new("../escape")).is_err());
    assert!(write_own(shared.path(), &DeviceSnapshot::new("")).is_err());
}

#[test]
fn hidden_and_other_files_in_devices_are_ignored() {
    let shared = TempDir::new();
    write_own(shared.path(), &device_a()).unwrap();
    let devices = shared.path().join(DEVICES_DIR);
    fs::write(devices.join(".aaaa.json.tmp"), "partial").unwrap();
    fs::write(devices.join("notes.txt"), "hello").unwrap();

    let mut cache = SyncCache::default();
    let report = cache.read_folder(shared.path());

    assert_eq!(report.read, 1);
    assert!(report.skipped.is_empty());
}

#[test]
fn a_missing_folder_reads_nothing() {
    let shared = TempDir::new();
    let mut cache = SyncCache::default();
    let report = cache.read_folder(&shared.path().join("nope"));
    assert_eq!(report.read, 0);
    assert!(cache.merged().pets.is_empty());
}
