//! Stage 1 (#38): two Macs simulated with two Application Support folders
//! pointed at one sync folder. Nothing here touches the real save or a real
//! sync service.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use chrono::{Local, NaiveDate, Utc};
use crate::commands::{
    self, buy_snack_at, care_at, choose_pet_at, report_game_result_at, revive_at, set_sync_settings_at,
    sync_now_at, tick_and_sync, GameReport, Moment,
};
use crate::companion::{CareKind, CompanionState};
use crate::pet::{achievements, EvolutionStage, GameStats, PetState};
use crate::{storage, AppState};
use super::device::DEVICE_FILE;
use super::folder::{CONFLICTS_DIR, DEVICES_DIR};
use super::passive::slot_of;
use super::runtime::{CACHE_FILE, ERR_NO_FOLDER_SET, ERR_TIMEOUT, LEDGER_FILE, PASSIVE_EPOCH, WRITE_TIMEOUT};
use super::settings::{self, SETTINGS_FILE};
use super::{DeviceSnapshot, Hardware, SyncStatusView};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("dot-sync-wiring-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn text(&self) -> String {
        self.0.display().to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One simulated Mac: its own Application Support and hardware UUID.
struct Mac {
    home: TempDir,
    hardware: &'static str,
    app: AppState,
}

impl Mac {
    fn new(hardware: &'static str, pet: &PetState, stats: &GameStats) -> Self {
        let home = TempDir::new();
        storage::save_state_in(home.path(), pet).unwrap();
        storage::save_stats_in(home.path(), stats).unwrap();
        let app = load(home.path(), hardware);
        Self { home, hardware, app }
    }

    /// Quit and start again from what is on disk.
    fn restart(&mut self) {
        self.app = load(self.home.path(), self.hardware);
    }

    fn dir(&self) -> Option<&Path> {
        Some(self.home.path())
    }

    fn turn_on(&self, folder: &TempDir, at: Moment) {
        set_sync_settings_at(&self.app, Some(&folder.text()), Some(true), at, self.dir()).unwrap();
    }

    fn sync(&self, at: Moment) {
        tick_and_sync(&self.app, at, self.dir()).unwrap();
    }

    fn feed(&self, at: Moment) {
        care_at(&self.app, CareKind::Feed, GameStats::record_feed, at.now, at.today, self.dir()).unwrap();
    }

    fn device_id(&self) -> String {
        self.app.sync.lock().unwrap().ledger().unwrap().device_id.clone()
    }

    fn device_file(&self, folder: &TempDir) -> PathBuf {
        folder.path().join(DEVICES_DIR).join(format!("{}.json", self.device_id()))
    }

    fn pet(&self) -> PetState {
        self.app.pet.lock().unwrap().clone()
    }

    fn stats(&self) -> GameStats {
        self.app.stats.lock().unwrap().clone()
    }

    fn status(&self) -> SyncStatusView {
        self.app.sync.lock().unwrap().status_view()
    }
}

fn load(home: &Path, hardware: &str) -> AppState {
    AppState::load_with_hardware(home, today(), Hardware::Fixed(hardware.to_string()))
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Now, by the real clock: FEED reads it, so these tests follow it.
fn now_moment() -> Moment {
    at(Utc::now().timestamp())
}

fn at(now: i64) -> Moment {
    Moment { now, idle_secs: 0, today: today() }
}

fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

/// A hungry egg with `id`, last updated at `t`.
fn pet(id: &str, t: i64) -> PetState {
    let mut pet = PetState::new();
    pet.id = id.to_string();
    pet.birth_time = t - 600;
    pet.last_update = t;
    pet.hunger = 40;
    pet
}

fn stats_with_coins(coins: u64) -> GameStats {
    let mut stats = GameStats::new();
    stats.add_coins(coins);
    stats
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

/// The pet as the needs show it, without the clock fields.
fn needs(pet: &PetState) -> (String, u8, u8, u8, u8, bool) {
    (pet.id.clone(), pet.hunger, pet.happiness, pet.energy, pet.health, pet.is_alive)
}

// --- Off ---

#[test]
fn off_runs_exactly_as_before_and_writes_no_sync_file() {
    let t = Utc::now().timestamp();
    let mut stats = stats_with_coins(20);
    stats.last_passive_time = Some(t - 3_000);
    let seed_pet = pet("pet-x", t - 60);
    let mac = Mac::new("HW-A", &seed_pet, &stats);
    let folder = TempDir::new();
    // No sync at all: the app as it was before #38
    let reference = AppState::with(seed_pet.clone(), stats.clone(), CompanionState::default(), None);
    let reference_home = TempDir::new();

    for (app, dir) in [(&mac.app, mac.home.path()), (&reference, reference_home.path())] {
        commands::tick(app, at(t), Some(dir)).unwrap();
        tick_and_sync(app, at(t + 60), Some(dir)).unwrap();
        care_at(app, CareKind::Feed, GameStats::record_feed, t + 60, today(), Some(dir)).unwrap();
        buy_snack_at(app, "cookie", t + 60, Some(dir)).unwrap();
        tick_and_sync(app, at(t + 120), Some(dir)).unwrap();
        app.pet.lock().unwrap().is_alive = false;
        revive_at(app, Some(dir)).unwrap();
    }

    assert_eq!(json(&mac.stats()), json(&*reference.stats.lock().unwrap()));
    assert_eq!(needs(&mac.pet()), needs(&reference.pet.lock().unwrap()));
    assert_eq!(*mac.app.companion.lock().unwrap(), *reference.companion.lock().unwrap());
    // The egg keeps its id when sync is off, as it always did
    assert_eq!(mac.pet().id, "pet-x");
    assert!(names(folder.path()).is_empty());
    let local = names(mac.home.path());
    for file in [DEVICE_FILE, SETTINGS_FILE, LEDGER_FILE, CACHE_FILE] {
        assert!(!local.contains(&file.to_string()), "{} was written: {:?}", file, local);
    }
    assert_eq!(local, names(reference_home.path()));
    assert!(!mac.status().enabled);

    // SYNC NOW and WHICH DOT? do nothing while off
    let stats_before = json(&mac.stats());
    sync_now_at(&mac.app, at(t + 180), mac.dir()).unwrap();
    assert!(choose_pet_at(&mac.app, "pet-x", at(t + 180), mac.dir()).is_err());
    assert_eq!(json(&mac.stats()), stats_before);
    assert!(names(folder.path()).is_empty());
    let local = names(mac.home.path());
    for file in [DEVICE_FILE, SETTINGS_FILE, LEDGER_FILE, CACHE_FILE] {
        assert!(!local.contains(&file.to_string()), "{} was written: {:?}", file, local);
    }
}

// --- Two Macs, one folder ---

/// Two Macs that already share pet X (a copied save), sync on. A has 10 coins.
fn two_macs_with_one_pet(folder: &TempDir, t: i64) -> (Mac, Mac) {
    let a = Mac::new("HW-A", &pet("pet-x", t), &stats_with_coins(10));
    let b = Mac::new("HW-B", &pet("pet-x", t), &GameStats::new());
    a.turn_on(folder, at(t));
    b.turn_on(folder, at(t));
    a.sync(at(t));
    (a, b)
}

#[test]
fn a_feed_on_one_mac_shows_on_the_other() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    assert_ne!(a.device_id(), b.device_id());

    a.feed(now_moment());
    let fed = a.pet();
    b.sync(at(fed.last_update));

    let on_b = b.pet();
    assert_eq!(on_b.last_action_at, fed.last_action_at);
    assert_eq!(needs(&on_b), needs(&fed));
    assert_eq!(b.stats().total_feeds, 1);
    // A's 10 coins and the first care bonus of the day, counted once
    assert_eq!(b.stats().coins, 11);
    assert_eq!(a.stats().coins, 11);
    assert!(b.status().last_sync_at.is_some());
    assert_eq!(b.status().error, None);
}

#[test]
fn passive_income_for_the_same_ten_minutes_counts_once() {
    let folder = TempDir::new();
    // On a slot boundary, so neither ledger drops a slot
    let t = (slot_of(Utc::now().timestamp()) - 20) * 600;
    let mut stats = GameStats::new();
    stats.last_passive_time = Some(t);
    let a = Mac::new("HW-A", &pet("pet-x", t), &stats);
    let b = Mac::new("HW-B", &pet("pet-x", t), &stats);
    a.turn_on(&folder, at(t));
    b.turn_on(&folder, at(t));

    for minute in 1..=120 {
        let now = at(t + minute * 60);
        a.sync(now);
        b.sync(now);
    }
    a.sync(at(t + 120 * 60));

    assert_eq!(a.stats().coins, 12);
    assert_eq!(b.stats().coins, 12);
    assert_eq!(b.stats().total_coins_earned, 12);
    assert!(a.pet().is_alive && b.pet().is_alive);
}

#[test]
fn writing_over_a_file_never_loses_coins_or_counters_and_syncing_again_adds_nothing() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);

    a.feed(now_moment());
    // B wins a game meanwhile, with nothing of A's feed in its file
    let started = Instant::now();
    let token = b.app.game_sessions.lock().unwrap().start("janken", started).unwrap();
    let report = GameReport { token: &token, game: "janken", outcome: "win", moves: None };
    let reward = report_game_result_at(&b.app, report, started + Duration::from_secs(2), "2026-10-14", b.dir()).unwrap();
    assert_eq!(reward.earned, 2);
    let later = now_moment();
    a.sync(later);
    b.sync(later);
    a.sync(later);

    for mac in [&a, &b] {
        let stats = mac.stats();
        assert_eq!(stats.total_feeds, 1, "on {}", mac.hardware);
        // 10 (A's save) + 1 (care bonus) + 2 (B's game)
        assert_eq!(stats.coins, 13, "on {}", mac.hardware);
    }

    let (stats_a, stats_b, pet_b) = (a.stats(), b.stats(), b.pet());
    for _ in 0..2 {
        a.sync(later);
        b.sync(later);
        sync_now_at(&b.app, later, b.dir()).unwrap();
    }
    assert_eq!(json(&a.stats()), json(&stats_a));
    assert_eq!(json(&b.stats()), json(&stats_b));
    assert_eq!(json(&b.pet()), json(&pet_b));
}

#[test]
fn a_broken_unreadable_or_missing_file_keeps_the_last_good_one() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, mut b) = two_macs_with_one_pet(&folder, t);
    a.feed(now_moment());
    let later = now_moment();
    b.sync(later);
    let (pet_before, stats_before) = (b.pet(), b.stats());
    assert_eq!(stats_before.total_feeds, 1);
    let a_file = a.device_file(&folder);

    let check = |b: &Mac, what: &str| {
        assert_eq!(needs(&b.pet()), needs(&pet_before), "{}", what);
        assert_eq!(b.pet().id, "pet-x", "{}", what);
        assert_eq!(json(&b.stats()), json(&stats_before), "{}", what);
    };

    fs::write(&a_file, "{\"device_id\": \"aaaa").unwrap();
    sync_now_at(&b.app, later, b.dir()).unwrap();
    check(&b, "broken");
    assert_eq!(b.status().unreadable, 1);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(&a_file, "{}").unwrap();
        fs::set_permissions(&a_file, fs::Permissions::from_mode(0o000)).unwrap();
        sync_now_at(&b.app, later, b.dir()).unwrap();
        fs::set_permissions(&a_file, fs::Permissions::from_mode(0o644)).unwrap();
        check(&b, "unreadable");
    }

    fs::remove_file(&a_file).unwrap();
    sync_now_at(&b.app, later, b.dir()).unwrap();
    check(&b, "missing");

    // And after a restart, from the copy kept locally
    b.restart();
    sync_now_at(&b.app, later, b.dir()).unwrap();
    check(&b, "missing after a restart");
}

#[test]
fn an_older_conflict_copy_does_not_double_and_moves_to_conflicts() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    let old_copy = fs::read_to_string(a.device_file(&folder)).unwrap();
    a.feed(now_moment());
    let copy = folder.path().join(DEVICES_DIR).join(format!("{} 2.json", a.device_id()));
    fs::write(&copy, old_copy).unwrap();

    let later = now_moment();
    b.sync(later);
    b.sync(later);

    assert_eq!(b.stats().total_feeds, 1);
    assert_eq!(b.stats().coins, 11);
    assert!(!copy.exists());
    assert!(folder.path().join(CONFLICTS_DIR).join(copy.file_name().unwrap()).exists());
}

#[test]
fn a_conflict_copy_of_its_own_file_is_taken_in_by_its_owner() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    // A sync service kept a version of A's file that A no longer has: 4 more feeds
    let mut lost: DeviceSnapshot = serde_json::from_str(&fs::read_to_string(a.device_file(&folder)).unwrap()).unwrap();
    lost.counters.feeds += 4;
    let copy = folder.path().join(DEVICES_DIR).join(format!("{} (conflicted copy).json", a.device_id()));
    fs::write(&copy, serde_json::to_string(&lost).unwrap()).unwrap();

    let later = at(t + 60);
    a.sync(later);
    assert_eq!(a.app.sync.lock().unwrap().ledger().unwrap().counters.feeds, lost.counters.feeds);
    assert!(copy.exists(), "kept until A's own file holds it");
    // A's own file now holds it, so the copy moves aside on the next read
    sync_now_at(&a.app, later, a.dir()).unwrap();
    assert!(!copy.exists());
    b.sync(later);
    assert_eq!(b.stats().total_feeds, lost.counters.feeds);
    assert_eq!(a.stats().total_feeds, lost.counters.feeds);
}

#[test]
fn two_living_pets_ask_which_dot_and_the_one_not_chosen_is_archived() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let mut x = pet("pet-x", t);
    x.stage = EvolutionStage::Child;
    x.age = 3 * 3_600;
    let a = Mac::new("HW-A", &x, &stats_with_coins(40));
    let b = Mac::new("HW-B", &pet("pet-y", t), &stats_with_coins(25));
    a.turn_on(&folder, at(t));
    b.turn_on(&folder, at(t));
    a.sync(at(t));

    let choice_a = a.status().choice.expect("A asks");
    let choice_b = b.status().choice.expect("B asks");
    let ids = |choice: &[super::PetChoice]| choice.iter().map(|c| (c.id.clone(), c.mine)).collect::<Vec<_>>();
    assert_eq!(ids(&choice_a), vec![("pet-x".to_string(), true), ("pet-y".to_string(), false)]);
    assert_eq!(ids(&choice_b), vec![("pet-y".to_string(), true), ("pet-x".to_string(), false)]);
    assert_eq!((choice_a[0].stage, choice_a[0].age), (EvolutionStage::Child, 3 * 3_600));
    // Until someone chooses, each Mac keeps its own pet, and the coins add up
    a.sync(at(t + 60));
    b.sync(at(t + 60));
    assert_eq!(a.pet().id, "pet-x");
    assert_eq!(b.pet().id, "pet-y");
    assert_eq!(a.stats().coins, 65);
    assert_eq!(b.stats().coins, 65);

    let status = choose_pet_at(&b.app, "pet-x", at(t + 60), b.dir()).unwrap();

    assert_eq!(status.choice, None);
    assert_eq!(b.pet().id, "pet-x");
    assert_eq!(b.pet().stage, EvolutionStage::Child);
    {
        let sync = b.app.sync.lock().unwrap();
        let ledger = sync.ledger().unwrap();
        assert!(ledger.retired_pets.contains("pet-y"));
        // Archived, not deleted
        assert!(ledger.pets.contains_key("pet-y"));
    }
    a.sync(at(t + 120));
    assert_eq!(a.status().choice, None);
    assert_eq!(a.pet().id, "pet-x");
    assert_eq!(a.stats().coins, 65);
    assert_eq!(b.stats().coins, 65);
    // The other pet is out of the merge now
    assert!(choose_pet_at(&b.app, "pet-y", at(t + 120), b.dir()).is_err());
}

#[test]
fn a_pet_revived_on_one_mac_is_taken_up_by_the_other() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);

    {
        let mut pet = a.app.pet.lock().unwrap();
        pet.is_alive = false;
        pet.health = 0;
    }
    revive_at(&a.app, a.dir()).unwrap();
    let egg = a.pet();
    assert_ne!(egg.id, "pet-x");
    b.sync(at(egg.last_update));

    assert_eq!(b.pet().id, egg.id);
    assert!(b.pet().is_alive);
    assert_eq!(b.status().choice, None);
    assert_eq!(b.stats().total_pets_raised, 2);
}

#[test]
fn reset_of_a_living_pet_does_not_ask_which_dot() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    b.sync(at(t));

    revive_at(&a.app, a.dir()).unwrap();
    let egg = a.pet();
    a.sync(at(egg.last_update));
    b.sync(at(egg.last_update));

    assert_eq!(a.status().choice, None);
    assert_eq!(b.status().choice, None);
    assert_eq!(b.pet().id, egg.id);
}

#[test]
fn a_day_of_care_is_in_the_ledger_before_the_next_day_settles_it() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let mut adult = pet("pet-x", t);
    adult.stage = EvolutionStage::Adult;
    adult.age = 9 * 86_400;
    let a = Mac::new("HW-A", &adult, &GameStats::new());
    let b = Mac::new("HW-B", &adult, &GameStats::new());
    let day = today();
    let next_day = day.succ_opt().unwrap();
    a.turn_on(&folder, at(t));
    b.turn_on(&folder, at(t));

    // Hungry, so the FEED counts for Bond on A
    care_at(&a.app, CareKind::Feed, GameStats::record_feed, t, day, a.dir()).unwrap();
    assert_eq!(a.app.companion.lock().unwrap().today.n_care, 1);
    // The next day's tick settles A's day and resets the counters
    commands::tick(&a.app, Moment { now: t, idle_secs: 0, today: next_day }, a.dir()).unwrap();
    assert_eq!(a.app.companion.lock().unwrap().today.n_care, 0);

    let recorded = a.app.sync.lock().unwrap().ledger().unwrap().bond["pet-x"].days[&day];
    assert_eq!(recorded.n_care, 1);
    a.sync(Moment { now: t, idle_secs: 0, today: next_day });
    b.sync(Moment { now: t, idle_secs: 0, today: next_day });
    let bond_a = a.app.companion.lock().unwrap().bond;
    assert!(bond_a > 0);
    assert_eq!(b.app.companion.lock().unwrap().bond, bond_a);
}

#[test]
fn turning_sync_off_and_on_again_keeps_what_was_done_meanwhile() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    a.feed(now_moment());
    let off = now_moment();
    set_sync_settings_at(&a.app, None, Some(false), off, a.dir()).unwrap();

    // Off: A plays twice and nothing reaches the folder
    let written = fs::read_to_string(a.device_file_while_off(&folder)).unwrap();
    for _ in 0..2 {
        a.app.pet.lock().unwrap().last_play_time = None;
        care_at(&a.app, CareKind::Play, GameStats::record_play, off.now, off.today, a.dir()).unwrap();
    }
    assert_eq!(fs::read_to_string(a.device_file_while_off(&folder)).unwrap(), written);

    set_sync_settings_at(&a.app, None, Some(true), now_moment(), a.dir()).unwrap();
    b.sync(now_moment());

    for mac in [&a, &b] {
        let stats = mac.stats();
        assert_eq!((stats.total_feeds, stats.total_plays), (1, 2), "on {}", mac.hardware);
        assert_eq!(stats.coins, 11, "on {}", mac.hardware);
    }
}

impl Mac {
    /// The device file by the id in `device.json`, readable while sync is off.
    fn device_file_while_off(&self, folder: &TempDir) -> PathBuf {
        let record: super::DeviceRecord =
            serde_json::from_str(&fs::read_to_string(self.home.path().join(DEVICE_FILE)).unwrap()).unwrap();
        folder.path().join(DEVICES_DIR).join(format!("{}.json", record.device_id))
    }
}

// --- Migration ---

#[test]
fn turning_sync_on_for_an_old_save_changes_nothing_and_never_migrates_twice() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let mut adult = pet("pet-old", t);
    adult.stage = EvolutionStage::Adult;
    adult.age = 9 * 86_400;
    let mut stats: GameStats = serde_json::from_str(
        r#"{"total_feeds":12,"total_plays":5,"total_sleeps":2,"total_pets_raised":3,
            "current_streak":4,"longest_streak":9,"last_care_date":"2026-10-13",
            "unlocked":["first_feed"],"coins":37,"total_coins_earned":120}"#,
    )
    .unwrap();
    stats.last_passive_time = Some(t);
    achievements::check_achievements(&mut stats, &adult);
    let mut mac = Mac::new("HW-A", &adult, &stats);
    {
        let mut companion = mac.app.companion.lock().unwrap();
        companion.bond = 321;
        companion.today.n_care = 2;
    }
    let before = (json(&mac.pet()), json(&mac.stats()), mac.app.companion.lock().unwrap().clone());

    mac.turn_on(&folder, at(t));
    mac.sync(at(t));

    assert_eq!(json(&mac.pet()), before.0);
    assert_eq!(json(&mac.stats()), before.1);
    assert_eq!(*mac.app.companion.lock().unwrap(), before.2);
    assert!(mac.app.sync.lock().unwrap().settings().migrated);
    let first_id = mac.device_id();

    // device.json is made again (deleted, or the save came from another Mac)
    fs::remove_file(mac.home.path().join(DEVICE_FILE)).unwrap();
    mac.restart();
    sync_now_at(&mac.app, at(t), mac.dir()).unwrap();

    assert_ne!(mac.device_id(), first_id);
    assert_eq!(mac.app.sync.lock().unwrap().ledger().unwrap().counters.feeds, 0);
    assert_eq!(json(&mac.stats()), before.1);
    assert_eq!(mac.app.companion.lock().unwrap().bond, 321);
}

#[test]
fn a_lost_settings_file_does_not_migrate_the_merged_save_again() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, mut b) = two_macs_with_one_pet(&folder, t);
    b.sync(at(t));
    // B's stats now hold A's 10 coins, merged
    assert_eq!(b.stats().coins, 10);

    fs::write(b.home.path().join(SETTINGS_FILE), "{ broken").unwrap();
    let _ = fs::remove_file(b.home.path().join(format!("{}.bak", SETTINGS_FILE)));
    b.restart();
    assert!(!b.status().enabled);
    b.turn_on(&folder, at(t));
    a.sync(at(t));

    assert_eq!(b.app.sync.lock().unwrap().ledger().unwrap().coins.earned, 0);
    assert_eq!(b.stats().coins, 10);
    assert_eq!(a.stats().coins, 10);
}

#[cfg(unix)]
#[test]
fn a_skipped_file_is_read_again_on_the_next_tick() {
    use std::os::unix::fs::PermissionsExt;
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    b.sync(at(t));
    a.feed(now_moment());
    let a_file = a.device_file(&folder);

    // Unreadable for a moment; chmod leaves the size and the modification time
    fs::set_permissions(&a_file, fs::Permissions::from_mode(0o000)).unwrap();
    b.sync(now_moment());
    fs::set_permissions(&a_file, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(b.status().unreadable, 1);
    assert_eq!(b.stats().total_feeds, 0);

    b.sync(now_moment());
    assert_eq!(b.status().unreadable, 0);
    assert_eq!(b.stats().total_feeds, 1);
}

#[test]
fn a_folder_that_hangs_on_write_holds_the_game_up_only_briefly() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, _b) = two_macs_with_one_pet(&folder, t);
    a.app.sync.lock().unwrap().slow_write_for_test = Some(WRITE_TIMEOUT * 4);

    let started = Instant::now();
    a.feed(now_moment());
    let first = started.elapsed();
    assert!(first < WRITE_TIMEOUT * 2, "first write held the game for {:?}", first);
    assert_eq!(a.status().error.as_deref(), Some(ERR_TIMEOUT));

    // The stuck write is still running: the next ones do not wait for it
    let started = Instant::now();
    a.app.pet.lock().unwrap().last_play_time = None;
    care_at(&a.app, CareKind::Play, GameStats::record_play, now_moment().now, today(), a.dir()).unwrap();
    a.sync(now_moment());
    assert!(started.elapsed() < WRITE_TIMEOUT, "later writes waited {:?}", started.elapsed());
    assert_eq!(a.stats().total_plays, 1);
}

// --- Bad input ---

#[test]
fn a_made_up_passive_range_is_cut_to_now() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let b = Mac::new("HW-B", &pet("pet-x", t), &GameStats::new());
    b.turn_on(&folder, at(t));
    let mut forged = DeviceSnapshot::new("ffffffff-0000-4000-8000-00000000000f");
    forged.coins.passive.ranges = vec![(i64::MIN, i64::MAX)];
    let devices = folder.path().join(DEVICES_DIR);
    fs::write(devices.join(format!("{}.json", forged.device_id)), serde_json::to_string(&forged).unwrap()).unwrap();
    // And the same in a conflict copy of B's own file, which B takes in
    let mut own_copy = forged.clone();
    own_copy.device_id = b.device_id();
    fs::write(devices.join(format!("{} 2.json", b.device_id())), serde_json::to_string(&own_copy).unwrap()).unwrap();

    sync_now_at(&b.app, at(t), b.dir()).unwrap();

    let most = (slot_of(t) - slot_of(PASSIVE_EPOCH)) as u64;
    assert_eq!(b.stats().coins, most);
    assert!(b.app.sync.lock().unwrap().ledger().unwrap().coins.passive.slot_count() <= most);
}

#[test]
fn an_invalid_folder_is_refused_and_changes_nothing() {
    let good = TempDir::new();
    let t = Utc::now().timestamp();
    let mac = Mac::new("HW-A", &pet("pet-x", t), &GameStats::new());
    assert_eq!(
        set_sync_settings_at(&mac.app, None, Some(true), at(t), mac.dir()),
        Err(ERR_NO_FOLDER_SET.to_string())
    );
    set_sync_settings_at(&mac.app, Some(&good.text()), None, at(t), mac.dir()).unwrap();
    let saved = fs::read_to_string(mac.home.path().join(SETTINGS_FILE)).unwrap();
    let view = mac.app.sync.lock().unwrap().settings_view();
    assert_eq!(view.path.as_deref(), Some(good.text().as_str()));

    let file = good.path().join("note.txt");
    fs::write(&file, "x").unwrap();
    let mut bad = vec![
        (good.path().join("missing").display().to_string(), settings::ERR_NOT_FOUND),
        (file.display().to_string(), settings::ERR_NOT_A_FOLDER),
        ("relative/path".to_string(), settings::ERR_NOT_FULL),
    ];
    #[cfg(unix)]
    let locked = {
        use std::os::unix::fs::PermissionsExt;
        let locked = good.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        bad.push((locked.display().to_string(), settings::ERR_CANNOT_WRITE_HERE));
        locked
    };

    for (path, error) in &bad {
        let result = set_sync_settings_at(&mac.app, Some(path), Some(true), at(t), mac.dir());
        assert_eq!(result, Err(error.to_string()), "{}", path);
        assert_eq!(mac.app.sync.lock().unwrap().settings_view(), view);
        assert_eq!(fs::read_to_string(mac.home.path().join(SETTINGS_FILE)).unwrap(), saved);
        assert!(!mac.status().enabled);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(!mac.home.path().join(DEVICE_FILE).exists());
}

#[test]
fn spending_more_than_all_macs_hold_is_refused() {
    let folder = TempDir::new();
    let t = Utc::now().timestamp();
    let (a, b) = two_macs_with_one_pet(&folder, t);
    b.sync(at(t));
    assert_eq!(b.stats().coins, 10);
    // Both spend the same 10 coins while apart
    a.app.sync.lock().unwrap().record_spent(10);
    a.sync(at(t));
    b.app.sync.lock().unwrap().record_spent(10);
    sync_now_at(&b.app, at(t), b.dir()).unwrap();
    assert_eq!(b.stats().coins, 0);

    // Even with this Mac's own count saying otherwise, nothing is bought below zero
    b.app.stats.lock().unwrap().coins = 10;
    b.app.pet.lock().unwrap().hunger = 10;
    assert!(buy_snack_at(&b.app, "candy", t, b.dir()).is_err());
}
