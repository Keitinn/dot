pub mod commands;
pub mod companion;
pub mod game_session;
pub mod pet;
pub mod signals;
pub mod storage;
pub mod sync;

use std::path::Path;
use std::sync::Mutex;
use chrono::{Local, NaiveDate};
use companion::CompanionState;
use game_session::GameSessions;
use pet::PetState;
use pet::GameStats;
use storage::LoadOutcome;
use sync::{Hardware, SyncRuntime};

/// Shown once when a save was restored from its `.bak` copy.
pub const WARNING_RECOVERED: &str = "SAVE RECOVERED";
/// Shown once when a broken save was moved aside and a fresh one started.
pub const WARNING_BACKED_UP: &str = "SAVE BACKED UP";

pub struct AppState {
    pub pet: Mutex<PetState>,
    pub stats: Mutex<GameStats>,
    pub load_warning: Mutex<Option<String>>,
    /// Started mini games waiting for their result. Not saved.
    pub game_sessions: Mutex<GameSessions>,
    /// Lock after `pet`, and before `stats` when both are held.
    pub companion: Mutex<CompanionState>,
    /// Lock last: after `pet`, `companion` and `stats`, whichever are held.
    pub sync: Mutex<SyncRuntime>,
}

impl AppState {
    pub fn new() -> Self {
        match storage::config_dir() {
            Ok(dir) => Self::load_from(&dir, Local::now().date_naive()),
            Err(e) => {
                eprintln!("Save directory unavailable, starting without a save: {}", e);
                Self::with(PetState::new(), GameStats::new(), CompanionState::default(), None)
            }
        }
    }

    /// `today` is the local date, used to start or settle the companion.
    pub fn load_from(dir: &Path, today: NaiveDate) -> Self {
        Self::load_with_hardware(dir, today, Hardware::ThisMac)
    }

    /// `load_from` with the hardware UUID for the sync device id given, so
    /// tests can run two Macs on one machine.
    pub fn load_with_hardware(dir: &Path, today: NaiveDate, hardware: Hardware) -> Self {
        let (pet, pet_warning) = resolve(storage::load_state_from(dir), "pet state", PetState::new);
        let (stats, stats_warning) = resolve(storage::load_stats_from(dir), "game stats", GameStats::new);
        // Bond is never shown, so losing it is only logged, not warned about
        let (mut companion, _) =
            resolve(storage::load_companion_from(dir), "companion", CompanionState::default);
        // An adult from before the companion existed starts here, at Bond 0
        companion.advance_day(&pet, today);

        // A reset loses more than a recovery, so it wins when both happen.
        let warning = [pet_warning, stats_warning]
            .into_iter()
            .flatten()
            .max_by_key(|w| *w == WARNING_BACKED_UP)
            .map(str::to_string);

        // Off unless SYNC was turned on; then the ledger and cache load too
        let mut sync = SyncRuntime::load(dir, hardware);
        sync.hold_stats_if_stopped(&stats);
        let mut state = Self::with(pet, stats, companion, warning);
        state.sync = Mutex::new(sync);
        state
    }

    fn with(
        pet: PetState,
        stats: GameStats,
        companion: CompanionState,
        load_warning: Option<String>,
    ) -> Self {
        Self {
            pet: Mutex::new(pet),
            stats: Mutex::new(stats),
            load_warning: Mutex::new(load_warning),
            game_sessions: Mutex::new(GameSessions::new()),
            companion: Mutex::new(companion),
            sync: Mutex::new(SyncRuntime::without_dir()),
        }
    }

    /// Returns the load warning once; later calls get `None`.
    pub fn take_load_warning(&self) -> Option<String> {
        self.load_warning.lock().ok().and_then(|mut w| w.take())
    }
}

fn resolve<T>(
    outcome: LoadOutcome<T>,
    what: &str,
    fresh: fn() -> T,
) -> (T, Option<&'static str>) {
    match outcome {
        LoadOutcome::Loaded(value) => (value, None),
        LoadOutcome::Missing => (fresh(), None),
        LoadOutcome::Recovered(value) => {
            eprintln!("Recovered {} from backup", what);
            (value, Some(WARNING_RECOVERED))
        }
        LoadOutcome::Reset => {
            eprintln!("Saved {} was unreadable; moved it aside and started fresh", what);
            (fresh(), Some(WARNING_BACKED_UP))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("dot-appstate-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
    }

    fn save_adult(dir: &Path) -> PetState {
        let mut pet = PetState::new();
        pet.stage = pet::EvolutionStage::Adult;
        storage::save_state_in(dir, &pet).unwrap();
        pet
    }

    #[test]
    fn existing_adult_without_a_companion_save_starts_today_at_zero() {
        let dir = TempDir::new();
        save_adult(&dir.0);

        let app = AppState::load_from(&dir.0, today());

        let companion = app.companion.lock().unwrap();
        assert_eq!(companion.bond, 0);
        assert_eq!(companion.last_settle_date, Some(today()));
        assert_eq!(companion.last_open_date, Some(today()));
        assert_eq!(companion.overdue_applied, 0);
        assert_eq!(companion.onboarding, companion::Onboarding::Unseen);
    }

    #[test]
    fn saved_companion_is_loaded_and_settled() {
        let dir = TempDir::new();
        let pet = save_adult(&dir.0);
        let mut saved = CompanionState::default();
        saved.advance_day(&pet, NaiveDate::from_ymd_opt(2026, 10, 13).unwrap());
        saved.bond = 280;
        saved.today.n_care = 2;
        storage::save_companion_in(&dir.0, &saved).unwrap();

        let app = AppState::load_from(&dir.0, today());

        let companion = app.companion.lock().unwrap();
        assert_eq!(companion.bond, 286);
        assert_eq!(companion.last_settle_date, Some(today()));
    }

    #[test]
    fn broken_companion_save_leaves_pet_and_stats_alone() {
        let dir = TempDir::new();
        let mut pet = PetState::new();
        pet.name = "mochi".to_string();
        storage::save_state_in(&dir.0, &pet).unwrap();
        let mut stats = GameStats::new();
        stats.coins = 42;
        storage::save_stats_in(&dir.0, &stats).unwrap();
        let pet_file = fs::read(dir.0.join(storage::STATE_FILE)).unwrap();
        let stats_file = fs::read(dir.0.join(storage::STATS_FILE)).unwrap();
        fs::write(dir.0.join(storage::COMPANION_FILE), "{ broken").unwrap();

        let app = AppState::load_from(&dir.0, today());

        assert_eq!(app.pet.lock().unwrap().name, "mochi");
        assert_eq!(app.stats.lock().unwrap().coins, 42);
        assert_eq!(*app.companion.lock().unwrap(), CompanionState::default());
        assert_eq!(app.take_load_warning(), None);
        assert_eq!(fs::read(dir.0.join(storage::STATE_FILE)).unwrap(), pet_file);
        assert_eq!(fs::read(dir.0.join(storage::STATS_FILE)).unwrap(), stats_file);
        let moved_aside = fs::read_dir(&dir.0)
            .unwrap()
            .filter(|e| {
                let name = e.as_ref().unwrap().file_name().to_string_lossy().into_owned();
                name.starts_with("companion.corrupt-")
            })
            .count();
        assert_eq!(moved_aside, 1);
    }

    #[test]
    fn clean_load_has_no_warning() {
        let dir = TempDir::new();
        let mut pet = PetState::new();
        pet.name = "mochi".to_string();
        storage::save_state_in(&dir.0, &pet).unwrap();

        let app = AppState::load_from(&dir.0, today());

        assert_eq!(app.pet.lock().unwrap().name, "mochi");
        assert_eq!(app.take_load_warning(), None);
    }

    #[test]
    fn broken_save_starts_fresh_and_warns_once() {
        let dir = TempDir::new();
        fs::write(dir.0.join(storage::STATS_FILE), "{ broken").unwrap();

        let app = AppState::load_from(&dir.0, today());

        assert_eq!(app.stats.lock().unwrap().coins, GameStats::new().coins);
        assert_eq!(app.take_load_warning().as_deref(), Some(WARNING_BACKED_UP));
        assert_eq!(app.take_load_warning(), None);
    }

    #[test]
    fn recovered_save_warns_recovered() {
        let dir = TempDir::new();
        let mut stats = GameStats::new();
        stats.coins = 55;
        storage::save_stats_in(&dir.0, &stats).unwrap();
        stats.coins = 60;
        storage::save_stats_in(&dir.0, &stats).unwrap();
        fs::write(dir.0.join(storage::STATS_FILE), "{ broken").unwrap();

        let app = AppState::load_from(&dir.0, today());

        assert_eq!(app.stats.lock().unwrap().coins, 55);
        assert_eq!(app.take_load_warning().as_deref(), Some(WARNING_RECOVERED));
    }

    #[test]
    fn reset_outranks_recovery_in_the_warning() {
        let dir = TempDir::new();
        let mut pet = PetState::new();
        storage::save_state_in(&dir.0, &pet).unwrap();
        pet.name = "renamed".to_string();
        storage::save_state_in(&dir.0, &pet).unwrap();
        fs::write(dir.0.join(storage::STATE_FILE), "{ broken").unwrap();
        fs::write(dir.0.join(storage::STATS_FILE), "{ broken").unwrap();

        let app = AppState::load_from(&dir.0, today());

        assert_eq!(app.take_load_warning().as_deref(), Some(WARNING_BACKED_UP));
    }
}
