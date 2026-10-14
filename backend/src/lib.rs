pub mod commands;
pub mod game_session;
pub mod pet;
pub mod signals;
pub mod storage;

use std::path::Path;
use std::sync::Mutex;
use game_session::GameSessions;
use pet::PetState;
use pet::GameStats;
use storage::LoadOutcome;

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
}

impl AppState {
    pub fn new() -> Self {
        match storage::config_dir() {
            Ok(dir) => Self::load_from(&dir),
            Err(e) => {
                eprintln!("Save directory unavailable, starting without a save: {}", e);
                Self::with(PetState::new(), GameStats::new(), None)
            }
        }
    }

    pub fn load_from(dir: &Path) -> Self {
        let (pet, pet_warning) = resolve(storage::load_state_from(dir), "pet state", PetState::new);
        let (stats, stats_warning) = resolve(storage::load_stats_from(dir), "game stats", GameStats::new);

        // A reset loses more than a recovery, so it wins when both happen.
        let warning = [pet_warning, stats_warning]
            .into_iter()
            .flatten()
            .max_by_key(|w| *w == WARNING_BACKED_UP)
            .map(str::to_string);

        Self::with(pet, stats, warning)
    }

    fn with(pet: PetState, stats: GameStats, load_warning: Option<String>) -> Self {
        Self {
            pet: Mutex::new(pet),
            stats: Mutex::new(stats),
            load_warning: Mutex::new(load_warning),
            game_sessions: Mutex::new(GameSessions::new()),
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

    #[test]
    fn clean_load_has_no_warning() {
        let dir = TempDir::new();
        let mut pet = PetState::new();
        pet.name = "mochi".to_string();
        storage::save_state_in(&dir.0, &pet).unwrap();

        let app = AppState::load_from(&dir.0);

        assert_eq!(app.pet.lock().unwrap().name, "mochi");
        assert_eq!(app.take_load_warning(), None);
    }

    #[test]
    fn broken_save_starts_fresh_and_warns_once() {
        let dir = TempDir::new();
        fs::write(dir.0.join(storage::STATS_FILE), "{ broken").unwrap();

        let app = AppState::load_from(&dir.0);

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

        let app = AppState::load_from(&dir.0);

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

        let app = AppState::load_from(&dir.0);

        assert_eq!(app.take_load_warning().as_deref(), Some(WARNING_BACKED_UP));
    }
}
