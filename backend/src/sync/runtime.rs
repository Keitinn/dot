//! Sync wired into the app (#38, design「複数端末とセーブ同期」段階 1).
//!
//! Holds this Mac's ledger, the last good copy of every other Mac's file and
//! what SYNC shows. The ledger is kept apart from `GameStats`: the merge is
//! written back into the game, and building the ledger from the game again
//! would count the other Macs twice. So every care, coin and payout goes into
//! the ledger through a `record_*` call as it happens.
//!
//! While sync is off nothing here reads or writes a file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime};
use chrono::NaiveDate;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::commands::Moment;
use crate::companion::state::is_active;
use crate::companion::{CareKind, CompanionState, Onboarding};
use crate::pet::{achievements, EvolutionStage, GameStats, PetState};
use crate::storage::{self, LoadOutcome};
use super::device;
use super::folder::{self, SyncCache, DEVICES_DIR};
use super::merge::{merge, Merged};
use super::passive::slot_of;
use super::settings::{self, StatsMark, SyncSettings};
use super::snapshot::{join_base, BondBase, DeviceSnapshot, StreakLedger};

/// This Mac's ledger, kept locally. The folder holds a copy.
pub const LEDGER_FILE: &str = "sync-ledger.json";
/// The last good copy of every device file read, kept locally.
pub const CACHE_FILE: &str = "sync-cache.json";
/// A folder slower than this (an evicted iCloud file downloading, a sleeping
/// network disk) is given up on for this time; the last good copies stay.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Passive slots before this (2023-01-01 UTC) are not taken in from a file.
pub const PASSIVE_EPOCH: i64 = 1_672_531_200;

// Shown as they are in the SYNC settings, so short and upper case.
pub const ERR_NO_FOLDER_SET: &str = "SET A FOLDER FIRST";
pub const ERR_NO_FOLDER: &str = "FOLDER MISSING";
pub const ERR_CANNOT_READ: &str = "CANNOT READ FOLDER";
pub const ERR_CANNOT_WRITE: &str = "CANNOT WRITE FOLDER";
pub const ERR_TIMEOUT: &str = "FOLDER TIMED OUT";
pub const ERR_NO_SAVE_DIR: &str = "NO SAVE FOLDER";
pub const ERR_CANNOT_SAVE: &str = "CANNOT SAVE SETTING";
pub const ERR_NO_DEVICE_ID: &str = "NO DEVICE ID";
pub const ERR_OFF: &str = "SYNC IS OFF";
pub const ERR_NOT_A_CHOICE: &str = "NOT A CHOICE";

/// Where the hardware UUID for `device.json` comes from.
#[derive(Debug, Clone)]
pub enum Hardware {
    ThisMac,
    /// For tests: two simulated Macs on one machine.
    Fixed(String),
}

impl Hardware {
    fn uuid(&self) -> String {
        match self {
            Hardware::ThisMac => device::this_hardware_uuid(),
            Hardware::Fixed(uuid) => uuid.clone(),
        }
    }
}

/// Name, size and modification time of each other device file.
type Signature = Vec<(String, u64, Option<SystemTime>)>;

#[derive(Debug, Clone, Default)]
struct Status {
    last_sync_at: Option<i64>,
    unreadable: usize,
    read_error: Option<String>,
    write_error: Option<String>,
    choice: Option<Vec<PetChoice>>,
}

/// One pet offered at `WHICH DOT?`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PetChoice {
    pub id: String,
    pub stage: EvolutionStage,
    /// Seconds, as `PetState.age`.
    pub age: u64,
    /// The pet this Mac has now.
    pub mine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncSettingsView {
    pub enabled: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncStatusView {
    pub enabled: bool,
    /// Unix seconds of the last read of the folder that worked.
    pub last_sync_at: Option<i64>,
    /// Device files skipped on the last read; their last good copy is used.
    pub unreadable: usize,
    pub error: Option<String>,
    /// Two or more living pets: `WHICH DOT?`. This Mac's pet first.
    pub choice: Option<Vec<PetChoice>>,
}

pub struct SyncRuntime {
    /// Application Support, where the local sync files live.
    local_dir: Option<PathBuf>,
    hardware: Hardware,
    settings: SyncSettings,
    /// This Mac's share. Some while sync runs.
    own: Option<DeviceSnapshot>,
    cache: SyncCache,
    /// The other device files as last read; unchanged means no read.
    seen: Option<Signature>,
    /// The own file as last written to the folder.
    written: Option<String>,
    /// The ledger as last saved locally.
    saved: Option<String>,
    status: Status,
    /// A read of the folder is running.
    reading: Arc<AtomicBool>,
}

impl SyncRuntime {
    fn new(local_dir: Option<PathBuf>, hardware: Hardware, settings: SyncSettings) -> Self {
        Self {
            local_dir,
            hardware,
            settings,
            own: None,
            cache: SyncCache::default(),
            seen: None,
            written: None,
            saved: None,
            status: Status::default(),
            reading: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Off, with nowhere to keep its files.
    pub fn without_dir() -> Self {
        Self::new(None, Hardware::ThisMac, SyncSettings::default())
    }

    /// Reads `sync-settings.json` in `dir`. Only when sync is on does it go on
    /// to `device.json`, the ledger and the cache.
    pub fn load(dir: &Path, hardware: Hardware) -> Self {
        let mut runtime = Self::new(Some(dir.to_path_buf()), hardware, settings::load(dir));
        if runtime.settings.enabled {
            if let Err(e) = runtime.start() {
                eprintln!("Sync could not start: {}", e);
                runtime.status.read_error = Some(e);
            }
        }
        runtime
    }

    pub fn is_on(&self) -> bool {
        self.settings.enabled && self.own.is_some()
    }

    /// This Mac's ledger while sync runs.
    pub fn ledger(&self) -> Option<&DeviceSnapshot> {
        self.own.as_ref()
    }

    pub fn settings(&self) -> &SyncSettings {
        &self.settings
    }

    pub fn settings_view(&self) -> SyncSettingsView {
        SyncSettingsView {
            enabled: self.settings.enabled,
            path: self.settings.path.as_ref().map(|p| p.display().to_string()),
        }
    }

    pub fn status_view(&self) -> SyncStatusView {
        let status = &self.status;
        SyncStatusView {
            enabled: self.settings.enabled,
            last_sync_at: status.last_sync_at,
            unreadable: status.unreadable,
            error: status.write_error.clone().or_else(|| status.read_error.clone()),
            choice: status.choice.clone().filter(|_| self.is_on()),
        }
    }

    /// Loads the device id, the ledger and the cache.
    fn start(&mut self) -> Result<(), String> {
        let dir = self.local_dir.clone().ok_or_else(|| ERR_NO_SAVE_DIR.to_string())?;
        let device_id = device::load_or_create_in(&dir, &self.hardware.uuid())
            .map_err(|e| {
                eprintln!("Could not load the device id: {}", e);
                ERR_NO_DEVICE_ID.to_string()
            })?
            .device_id;
        let mut cache: SyncCache = load_local(&dir.join(CACHE_FILE)).unwrap_or_default();
        let own = match load_local::<DeviceSnapshot>(&dir.join(LEDGER_FILE)) {
            Some(ledger) if ledger.device_id == device_id => ledger,
            Some(other) => {
                // device.json was made again, or came from another Mac: that
                // ledger is another device's share now. It still counts; this
                // Mac starts a new share and the old save is not migrated again
                if device::is_valid_id(&other.device_id) {
                    cache.absorb(&other);
                }
                DeviceSnapshot::new(&device_id)
            }
            // The copy in the folder, if any, is taken in on the first read
            None => DeviceSnapshot::new(&device_id),
        };
        self.cache = cache;
        self.own = Some(own);
        self.seen = None;
        self.written = None;
        self.saved = None;
        Ok(())
    }

    /// Sync is on in the settings but could not start. What is done meanwhile
    /// is counted as if sync were off, once it starts.
    pub fn hold_stats_if_stopped(&mut self, stats: &GameStats) {
        if self.settings.enabled && self.own.is_none() && self.settings.paused_at.is_none() {
            self.settings.paused_at = Some(StatsMark::of(stats));
        }
    }

    /// Checks and saves the folder. An invalid one changes nothing.
    pub fn set_folder(&mut self, raw: &str) -> Result<(), String> {
        let dir = self.local_dir.clone().ok_or_else(|| ERR_NO_SAVE_DIR.to_string())?;
        let path = settings::validate_folder(raw)?;
        if self.settings.path.as_ref() == Some(&path) {
            return Ok(());
        }
        let next = SyncSettings { path: Some(path), ..self.settings.clone() };
        save_settings(&dir, &next)?;
        self.settings = next;
        // The cache is kept: the same Macs write the same ids into the new
        // folder, and a join of what was read before adds nothing twice
        self.seen = None;
        self.written = None;
        self.status = Status::default();
        Ok(())
    }

    /// Turns sync on. The first time on this Mac, the save so far becomes its
    /// ledger (`from_legacy`); after a pause, what was done while off is added.
    pub fn enable(&mut self, pet: &mut PetState, companion: &mut CompanionState, stats: &GameStats) -> Result<(), String> {
        if self.is_on() {
            return Ok(());
        }
        let dir = self.local_dir.clone().ok_or_else(|| ERR_NO_SAVE_DIR.to_string())?;
        if self.settings.path.is_none() {
            return Err(ERR_NO_FOLDER_SET.to_string());
        }
        self.start()?;
        let mut next = self.settings.clone();
        let own = self.own.as_mut().expect("start sets the ledger");
        let mut revived_while_off = false;
        if !next.migrated {
            let device_id = own.device_id.clone();
            own.absorb(&DeviceSnapshot::from_legacy(&device_id, pet, stats, Some(companion)));
            next.migrated = true;
        } else if let Some(mark) = next.paused_at {
            revived_while_off = catch_up(own, mark, companion, stats);
        }
        next.enabled = true;
        next.paused_at = None;
        if let Err(e) = save_settings(&dir, &next) {
            self.own = None;
            return Err(e);
        }
        self.settings = next;
        if revived_while_off {
            // The egg kept the id of the pet that died, which another Mac may
            // still hold as dead; a new id keeps the two apart
            let id = uuid::Uuid::new_v4().to_string();
            if companion.pet_id.as_deref() == Some(pet.id.as_str()) {
                companion.pet_id = Some(id.clone());
            }
            pet.id = id;
        }
        self.persist_local();
        Ok(())
    }

    /// Turns sync off. The ledger is kept for when it is turned on again.
    pub fn disable(&mut self, stats: &GameStats) -> Result<(), String> {
        if !self.settings.enabled {
            return Ok(());
        }
        let dir = self.local_dir.clone().ok_or_else(|| ERR_NO_SAVE_DIR.to_string())?;
        self.persist_local();
        let paused_at = match self.own {
            Some(_) => StatsMark::of(stats),
            // Never started: the mark from when it should have
            None => self.settings.paused_at.unwrap_or_else(|| StatsMark::of(stats)),
        };
        let next = SyncSettings { enabled: false, paused_at: Some(paused_at), ..self.settings.clone() };
        save_settings(&dir, &next)?;
        self.settings = next;
        self.own = None;
        self.seen = None;
        self.written = None;
        self.saved = None;
        self.status = Status::default();
        Ok(())
    }

    // --- Recording what this Mac does ---

    /// A FEED / PLAY / SLEEP after `GameStats::record_*`. `earned_before` is
    /// `total_coins_earned` before it, so the daily care bonus is told apart.
    pub fn record_care(&mut self, kind: CareKind, stats: &GameStats, earned_before: u64) {
        let Some(own) = self.own.as_mut() else { return };
        // record_* always sets the care date (UTC, as the streak counts days)
        let Some(date) = stats.last_care_date.as_deref().and_then(parse_date) else { return };
        own.record_care(kind, date);
        let bonus = stats.total_coins_earned.saturating_sub(earned_before);
        if bonus > 0 {
            own.record_daily_bonus(date, bonus);
        }
    }

    /// `before` and `after` are `last_passive_time` around `accrue_passive`.
    pub fn record_passive(&mut self, before: Option<i64>, after: Option<i64>, earned: u64) {
        if let Some(own) = self.own.as_mut() {
            own.record_passive(before, after, earned);
        }
    }

    pub fn record_revive(&mut self) {
        if let Some(own) = self.own.as_mut() {
            own.record_revive();
        }
    }

    pub fn record_earned(&mut self, amount: u64) {
        if let Some(own) = self.own.as_mut() {
            own.record_earned(amount);
        }
    }

    pub fn record_spent(&mut self, amount: u64) {
        if let Some(own) = self.own.as_mut() {
            own.record_spent(amount);
        }
    }

    /// Left out of the merge from now on; its snapshots stay in the files.
    pub fn retire_pet(&mut self, pet_id: &str) {
        if let Some(own) = self.own.as_mut() {
            own.retire_pet(pet_id);
        }
    }

    pub fn observe_pet(&mut self, pet: &PetState) {
        if let Some(own) = self.own.as_mut() {
            own.record_pet(pet);
        }
    }

    pub fn observe_stats(&mut self, stats: &GameStats) {
        if let Some(own) = self.own.as_mut() {
            for id in &stats.unlocked {
                own.unlock(id);
            }
        }
    }

    /// The day counters so far, for the date they belong to. Called after
    /// every change, so they are in the ledger before the next settlement
    /// resets them.
    pub fn observe_companion(&mut self, companion: &CompanionState) {
        let Some(own) = self.own.as_mut() else { return };
        let (Some(pet_id), Some(settled)) = (companion.pet_id.as_deref(), companion.last_settle_date) else { return };
        let has_base = own.bond.get(pet_id).is_some_and(|ledger| ledger.base.is_some());
        if !has_base {
            if companion.bond == 0 && companion.overdue_applied == 0 {
                own.record_bond_start(pet_id, settled);
            } else {
                record_settled_bond(own, companion);
            }
        }
        if companion.today != Default::default() {
            own.record_bond_day(pet_id, settled, companion.today);
        }
        if companion.onboarding == Onboarding::Yes {
            own.bond.entry(pet_id.to_string()).or_default().onboarding_yes = true;
        }
    }

    // --- Merging ---

    /// All devices added up: the cache and this Mac's ledger. Passive slots
    /// from files are cut to [PASSIVE_EPOCH, now), so a made-up range cannot
    /// pay for time that has not passed.
    pub fn merged(&self, now: i64) -> Option<Merged> {
        let own = self.own.as_ref()?;
        let others: Vec<DeviceSnapshot> = self.cache.snapshots().map(|s| sanitized(s, now)).collect();
        Some(merge(others.iter().chain(std::iter::once(own))))
    }

    /// Whether the coins of all devices cover `price`; a negative balance
    /// stops buying until it is back above the price.
    pub fn can_spend(&self, price: u64, now: i64) -> bool {
        self.merged(now).is_none_or(|merged| merged.can_spend(price))
    }

    /// Writes the merge back into the game: the newest snapshot of this
    /// Mac's pet, the totals, Bond. The pet is updated once from its snapshot
    /// to now, so its needs and death follow the usual rules.
    pub fn apply(&mut self, pet: &mut PetState, companion: &mut CompanionState, stats: &mut GameStats, at: Moment) {
        if !self.is_on() {
            return;
        }
        self.observe_pet(pet);
        self.observe_companion(companion);
        self.observe_stats(stats);
        let Some(merged) = self.merged(at.now) else { return };

        if let Some(next) = pet_to_follow(&merged, pet) {
            if to_json(next) != to_json(pet) {
                *pet = next.clone();
                // A snapshot from a clock ahead of this one counts as no time passing
                pet.update_at(at.now, at.idle_secs);
            }
        }
        // A pet taken from another Mac starts the companion over. That start
        // goes into the ledger before Bond comes back from the replay, so the
        // replayed Bond is not taken for a settlement of this Mac's own
        companion.advance_day(pet, at.today);
        self.observe_companion(companion);
        apply_bond(&merged, pet, companion, at.today);
        apply_stats(&merged, stats);
        let _ = achievements::check_achievements(stats, pet);

        self.observe_pet(pet);
        self.observe_companion(companion);
        self.observe_stats(stats);
        self.status.choice = self
            .merged(at.now)
            .filter(Merged::needs_choice)
            .map(|merged| choices(&merged, &pet.id));
    }

    /// `WHICH DOT?`: keeps `pet_id` and retires every other living pet. The
    /// retired ones stay in the device files, left out of the merge.
    pub fn choose(
        &mut self,
        pet_id: &str,
        pet: &mut PetState,
        companion: &mut CompanionState,
        stats: &mut GameStats,
        at: Moment,
    ) -> Result<(), String> {
        if !self.is_on() {
            return Err(ERR_OFF.to_string());
        }
        self.observe_pet(pet);
        let merged = self.merged(at.now).ok_or_else(|| ERR_OFF.to_string())?;
        let living: Vec<String> = merged.living_pets().iter().map(|p| p.id.clone()).collect();
        if !living.iter().any(|id| id == pet_id) {
            return Err(ERR_NOT_A_CHOICE.to_string());
        }
        if let Some(own) = self.own.as_mut() {
            for id in living.iter().filter(|id| *id != pet_id) {
                own.retire_pet(id);
            }
        }
        self.apply(pet, companion, stats, at);
        Ok(())
    }

    // --- Files ---

    /// Saves the ledger locally, then writes this Mac's file into the folder
    /// unless it is the same as last time. A folder that cannot be written is
    /// shown in SYNC; the game goes on from the local save.
    pub fn write(&mut self) {
        if !self.is_on() {
            return;
        }
        self.persist_local();
        let (Some(own), Some(root)) = (self.own.as_ref(), self.settings.path.as_ref()) else { return };
        let json = match serde_json::to_string_pretty(own) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Could not serialize the ledger: {}", e);
                return;
            }
        };
        if self.written.as_deref() == Some(json.as_str()) {
            return;
        }
        // An unmounted disk: never create its path on the startup disk
        if !root.is_dir() {
            self.status.write_error = Some(ERR_NO_FOLDER.to_string());
            return;
        }
        match folder::write_own(root, own) {
            Ok(_) => {
                self.written = Some(json);
                self.status.write_error = None;
            }
            Err(e) => {
                eprintln!("Could not write to the sync folder: {}", e);
                self.status.write_error = Some(ERR_CANNOT_WRITE.to_string());
            }
        }
    }

    /// Saves the ledger in Application Support if it changed.
    pub fn persist_local(&mut self) {
        let (Some(own), Some(dir)) = (self.own.as_ref(), self.local_dir.as_ref()) else { return };
        let Ok(json) = serde_json::to_string(own) else { return };
        if self.saved.as_deref() == Some(json.as_str()) {
            return;
        }
        match storage::save_json(&dir.join(LEDGER_FILE), own) {
            Ok(()) => self.saved = Some(json),
            Err(e) => eprintln!("Could not save the sync ledger: {}", e),
        }
    }

    fn persist_cache(&self) {
        if let Some(dir) = &self.local_dir {
            if let Err(e) = storage::save_json(&dir.join(CACHE_FILE), &self.cache) {
                eprintln!("Could not save the sync cache: {}", e);
            }
        }
    }

    /// What a read of the folder needs, or None when sync is off or a read
    /// is still running. `force` reads even if no file seems to have changed.
    pub fn begin_fetch(&self, force: bool) -> Option<FetchJob> {
        let own = self.own.as_ref().filter(|_| self.settings.enabled)?;
        let root = self.settings.path.clone()?;
        if self.reading.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return None;
        }
        Some(FetchJob {
            root,
            own_id: own.device_id.clone(),
            cache: self.cache.clone(),
            seen: if force { None } else { self.seen.clone() },
            reading: Arc::clone(&self.reading),
        })
    }

    /// Takes in what a read found. A read of a folder set since is dropped.
    pub fn finish_fetch(&mut self, outcome: FetchOutcome, now: i64) {
        if !self.is_on() || self.settings.path.as_deref() != Some(outcome.root.as_path()) {
            return;
        }
        match outcome.result {
            Fetched::Unchanged(signature) => {
                self.seen = Some(signature);
                self.mark_synced(now);
            }
            Fetched::Read { cache, skipped, signature } => {
                self.cache.join(&cache);
                self.take_in_own_copies(now);
                self.seen = Some(signature);
                self.status.unreadable = skipped;
                self.mark_synced(now);
                self.persist_cache();
            }
            Fetched::Failed(error) => self.status.read_error = Some(error),
        }
    }

    fn mark_synced(&mut self, now: i64) {
        self.status.last_sync_at = Some(now);
        self.status.read_error = None;
    }

    /// Conflict copies of this Mac's own file (`kept_conflicts`), and its
    /// file in the folder when the local ledger was lost: the ledger takes
    /// them in, so the next write covers them and the copies can move aside.
    fn take_in_own_copies(&mut self, now: i64) {
        let Some(own) = self.own.as_mut() else { return };
        if let Some(copies) = self.cache.device(&own.device_id) {
            own.absorb(&sanitized(copies, now));
        }
    }
}

/// A read of the folder, done without the game's locks held.
pub struct FetchJob {
    root: PathBuf,
    own_id: String,
    cache: SyncCache,
    seen: Option<Signature>,
    reading: Arc<AtomicBool>,
}

pub struct FetchOutcome {
    root: PathBuf,
    result: Fetched,
}

enum Fetched {
    Unchanged(Signature),
    Read { cache: SyncCache, skipped: usize, signature: Signature },
    Failed(String),
}

impl Drop for FetchJob {
    fn drop(&mut self) {
        self.reading.store(false, Ordering::Release);
    }
}

impl FetchJob {
    /// Reads on a thread of its own and waits at most `timeout`. A read that
    /// takes longer finishes in the background and is thrown away.
    pub fn run(self, timeout: Duration) -> FetchOutcome {
        let root = self.root.clone();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("dot-sync-read".into()).spawn(move || {
            let mut job = self;
            let _ = tx.send(job.read());
        });
        let result = match spawned {
            Ok(_) => rx.recv_timeout(timeout).unwrap_or_else(|_| Fetched::Failed(ERR_TIMEOUT.to_string())),
            Err(e) => {
                eprintln!("Could not start the sync read: {}", e);
                Fetched::Failed(ERR_CANNOT_READ.to_string())
            }
        };
        FetchOutcome { root, result }
    }

    fn read(&mut self) -> Fetched {
        if !self.root.is_dir() {
            return Fetched::Failed(ERR_NO_FOLDER.to_string());
        }
        let signature = match signature(&self.root, &self.own_id) {
            Ok(signature) => signature,
            Err(e) => {
                eprintln!("Could not list {}: {}", self.root.display(), e);
                return Fetched::Failed(ERR_CANNOT_READ.to_string());
            }
        };
        if self.seen.as_ref() == Some(&signature) {
            return Fetched::Unchanged(signature);
        }
        let report = self.cache.read_folder(&self.root);
        Fetched::Read { cache: std::mem::take(&mut self.cache), skipped: report.skipped.len(), signature }
    }
}

/// The other device files, without reading them. The own file is left out:
/// writing it must not make the next tick read everything again.
fn signature(root: &Path, own_id: &str) -> io::Result<Signature> {
    let own_name = format!("{}.json", own_id);
    let entries = match fs::read_dir(root.join(DEVICES_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut signature = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == own_name {
            continue;
        }
        let meta = entry.metadata()?;
        signature.push((name, meta.len(), meta.modified().ok()));
    }
    signature.sort();
    Ok(signature)
}

fn load_local<T: DeserializeOwned>(path: &Path) -> Option<T> {
    match storage::load_json::<T>(path) {
        LoadOutcome::Loaded(value) | LoadOutcome::Recovered(value) => Some(value),
        LoadOutcome::Missing | LoadOutcome::Reset => None,
    }
}

fn save_settings(dir: &Path, next: &SyncSettings) -> Result<(), String> {
    settings::save(dir, next).map_err(|e| {
        eprintln!("Could not save the sync setting: {}", e);
        ERR_CANNOT_SAVE.to_string()
    })
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

fn to_json(pet: &PetState) -> serde_json::Value {
    serde_json::to_value(pet).unwrap_or_default()
}

fn sanitized(snapshot: &DeviceSnapshot, now: i64) -> DeviceSnapshot {
    let mut snapshot = snapshot.clone();
    snapshot.coins.passive.clamp_to(slot_of(PASSIVE_EPOCH), slot_of(now));
    snapshot
}

/// Bond as settled on this Mac, as the ledger's base. The latest settlement
/// wins over older ones and over every start.
fn record_settled_bond(own: &mut DeviceSnapshot, companion: &CompanionState) {
    let (Some(pet_id), Some(settled_on)) = (companion.pet_id.as_deref(), companion.last_settle_date) else { return };
    let base = BondBase {
        settled_on,
        bond: companion.bond,
        overdue_applied: companion.overdue_applied,
        last_open_date: companion.last_open_date.unwrap_or(settled_on),
        start: false,
    };
    let ledger = own.bond.entry(pet_id.to_string()).or_default();
    ledger.base = join_base(ledger.base, Some(base));
}

/// Adds to the ledger what the stats counted while sync was off. Returns
/// whether a pet was revived meanwhile.
fn catch_up(own: &mut DeviceSnapshot, mark: StatsMark, companion: &CompanionState, stats: &GameStats) -> bool {
    let since = |now: u64, then: u64| now.saturating_sub(then);
    let counters = &mut own.counters;
    counters.feeds = counters.feeds.saturating_add(since(stats.total_feeds, mark.feeds));
    counters.plays = counters.plays.saturating_add(since(stats.total_plays, mark.plays));
    counters.sleeps = counters.sleeps.saturating_add(since(stats.total_sleeps, mark.sleeps));
    let revives = since(stats.total_pets_raised, mark.pets_raised);
    counters.revives = counters.revives.saturating_add(revives);
    let earned = since(stats.total_coins_earned, mark.total_coins_earned);
    own.record_earned(earned);
    own.record_spent(mark.coins.saturating_add(earned).saturating_sub(stats.coins));
    for id in &stats.unlocked {
        own.unlock(id);
    }
    own.streak.absorb(&StreakLedger::from_legacy(stats));
    if revives == 0 {
        record_settled_bond(own, companion);
    }
    revives > 0
}

/// The snapshot this Mac's pet becomes: its own pet's newest snapshot while
/// it lives. When it died or was not chosen and exactly one pet lives
/// elsewhere, that one. Two or more living pets wait for `WHICH DOT?`, each
/// Mac keeping its own.
fn pet_to_follow<'a>(merged: &'a Merged, pet: &PetState) -> Option<&'a PetState> {
    // Retired pets are not in the merge
    let mine = merged.pets.get(&pet.id);
    let living = merged.living_pets();
    match mine {
        Some(mine) if mine.is_alive => Some(mine),
        _ if living.len() == 1 => Some(living[0]),
        mine => mine,
    }
}

/// Bond from the merged day counters. `companion.today` stays this Mac's
/// share, so it is never counted twice.
fn apply_bond(merged: &Merged, pet: &PetState, companion: &mut CompanionState, today: NaiveDate) {
    if !is_active(pet) || companion.pet_id.as_deref() != Some(pet.id.as_str()) {
        return;
    }
    if let Some(replay) = merged.bond_on(&pet.id, today) {
        // Only when both stand on the same day; a clock set back waits
        if companion.last_settle_date == Some(replay.last_settle_date) {
            companion.bond = replay.bond;
            companion.overdue_applied = replay.overdue_applied;
            companion.last_open_date = companion.last_open_date.max(Some(replay.last_open_date));
        }
    }
    if merged.bond.get(&pet.id).is_some_and(|ledger| ledger.onboarding_yes) {
        companion.onboarding = Onboarding::Yes;
    }
}

/// The totals of all devices. Leaves the mini game daily cap and the passive
/// clock alone: they belong to this Mac.
fn apply_stats(merged: &Merged, stats: &mut GameStats) {
    stats.total_feeds = merged.total_feeds;
    stats.total_plays = merged.total_plays;
    stats.total_sleeps = merged.total_sleeps;
    stats.total_pets_raised = merged.total_pets_raised;
    stats.coins = merged.coins_display();
    stats.total_coins_earned = merged.total_coins_earned;
    for id in &merged.unlocked {
        if !stats.unlocked.contains(id) {
            stats.unlocked.push(id.clone());
        }
    }
    stats.current_streak = merged.current_streak;
    stats.longest_streak = merged.longest_streak;
    if let Some(last) = merged.last_care_date {
        stats.last_care_date = Some(last.format("%Y-%m-%d").to_string());
    }
}

fn choices(merged: &Merged, mine: &str) -> Vec<PetChoice> {
    let mut choices: Vec<PetChoice> = merged
        .living_pets()
        .into_iter()
        .map(|pet| PetChoice { id: pet.id.clone(), stage: pet.stage, age: pet.age, mine: pet.id == mine })
        .collect();
    choices.sort_by_key(|choice| !choice.mine);
    choices
}
