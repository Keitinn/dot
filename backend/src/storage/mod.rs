use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write};
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::pet::PetState;
use crate::pet::GameStats;
use crate::companion::CompanionState;

pub const STATE_FILE: &str = "pet-state.json";
pub const STATS_FILE: &str = "game-stats.json";
pub const COMPANION_FILE: &str = "companion.json";

/// Result of reading one save file. Reading never fails outright: a broken
/// save is moved aside instead of being reported as an error, so the caller
/// can never mistake it for "no save yet" and overwrite it.
#[derive(Debug)]
pub enum LoadOutcome<T> {
    /// No save file and no usable backup (first launch).
    Missing,
    Loaded(T),
    /// The save was missing or broken; restored from the `.bak` copy.
    Recovered(T),
    /// The save or its backup was broken and nothing usable is left. Broken
    /// files were moved aside as `<stem>.corrupt-<unix secs>.json`; start fresh.
    Reset,
}

pub fn config_dir() -> Result<PathBuf, String> {
    let mut path = dirs::config_dir()
        .ok_or_else(|| "Could not find config directory".to_string())?;
    path.push("dot");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn save_state(pet: &PetState) -> Result<(), String> {
    save_state_in(&config_dir()?, pet)
}

pub fn load_state() -> Result<LoadOutcome<PetState>, String> {
    Ok(load_state_from(&config_dir()?))
}

pub fn save_stats(stats: &GameStats) -> Result<(), String> {
    save_stats_in(&config_dir()?, stats)
}

pub fn load_stats() -> Result<LoadOutcome<GameStats>, String> {
    Ok(load_stats_from(&config_dir()?))
}

pub fn save_state_in(dir: &Path, pet: &PetState) -> Result<(), String> {
    save_json(&dir.join(STATE_FILE), pet)
}

pub fn load_state_from(dir: &Path) -> LoadOutcome<PetState> {
    load_json(&dir.join(STATE_FILE))
}

pub fn save_stats_in(dir: &Path, stats: &GameStats) -> Result<(), String> {
    save_json(&dir.join(STATS_FILE), stats)
}

pub fn load_stats_from(dir: &Path) -> LoadOutcome<GameStats> {
    load_json(&dir.join(STATS_FILE))
}

pub fn save_companion(companion: &CompanionState) -> Result<(), String> {
    save_companion_in(&config_dir()?, companion)
}

/// A file of its own, so a broken companion save never touches the pet or the stats.
pub fn save_companion_in(dir: &Path, companion: &CompanionState) -> Result<(), String> {
    let _ = (dir, companion);
    unimplemented!()
}

pub fn load_companion_from(dir: &Path) -> LoadOutcome<CompanionState> {
    let _ = dir;
    unimplemented!()
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    match fs::read(path) {
        // Unchanged: skip the fsync and keep the older backup.
        Ok(current) if current == json.as_bytes() => return Ok(()),
        Ok(_) => {
            // The backup is only a fallback for a damaged save, so it skips fsync.
            replace_file(&hidden_sibling(path, "bak.tmp"), &backup_path(path), |tmp| {
                fs::copy(path, tmp).map(|_| ())
            })
            .map_err(|e| format!("could not back up {}: {}", path.display(), e))?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("could not read {} before saving: {}", path.display(), e)),
    }
    replace_file(&hidden_sibling(path, "tmp"), path, |tmp| {
        let mut file = File::create(tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()
    })
    .map_err(|e| format!("could not write {}: {}", path.display(), e))
}

/// Builds `tmp` with `write`, then renames it over `target`, so `target` is
/// either the old file or the complete new one, never a partial write.
fn replace_file(
    tmp: &Path,
    target: &Path,
    write: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let result = write(tmp).and_then(|()| fs::rename(tmp, target));
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

fn load_json<T: DeserializeOwned>(path: &Path) -> LoadOutcome<T> {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let main_broken = match read_json(path) {
        Ok(Some(value)) => return LoadOutcome::Loaded(value),
        Ok(None) => false,
        Err(e) => {
            eprintln!("Could not load {}: {}", path.display(), e);
            move_aside(path, &stem);
            true
        }
    };

    let bak = backup_path(path);
    match read_json(&bak) {
        Ok(Some(value)) => LoadOutcome::Recovered(value),
        Ok(None) if main_broken => LoadOutcome::Reset,
        Ok(None) => LoadOutcome::Missing,
        Err(e) => {
            // Keep it too: the next two saves would otherwise rotate over it.
            eprintln!("Backup {} is unusable: {}", bak.display(), e);
            move_aside(&bak, &format!("{}.bak", stem));
            LoadOutcome::Reset
        }
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_str(&json).map(Some).map_err(|e| e.to_string())
}

/// Renames a broken file to `<stem>.corrupt-<unix secs>.json` so the next save
/// cannot overwrite it. Falls back to copying if the rename fails.
fn move_aside(path: &Path, stem: &str) {
    let dest = corrupt_path(path, stem, Utc::now().timestamp());
    let moved = fs::rename(path, &dest).or_else(|e| {
        eprintln!("Could not move {} aside ({}); copying instead", path.display(), e);
        fs::copy(path, &dest).map(|_| ())
    });
    match moved {
        Ok(()) => eprintln!("Kept unreadable save as {}", dest.display()),
        Err(e) => eprintln!("Could not preserve {}: {}", path.display(), e),
    }
}

fn corrupt_path(path: &Path, stem: &str, timestamp: i64) -> PathBuf {
    let candidate = |suffix: String| {
        path.with_file_name(format!("{}.corrupt-{}{}.json", stem, timestamp, suffix))
    };
    // Never reuse a name: an earlier corrupt file may hold the only copy.
    (0..)
        .map(|n| candidate(if n == 0 { String::new() } else { format!("-{}", n) }))
        .find(|p| !p.exists())
        .expect("unbounded search always finds a free name")
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// `.pet-state.json.<suffix>` next to `path`. The leading dot keeps scratch
/// files out of Finder.
fn hidden_sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{}.{}", name, suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("dot-storage-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn file_names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.0)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        fn corrupt_files(&self, prefix: &str) -> Vec<PathBuf> {
            self.file_names()
                .into_iter()
                .filter(|n| n.starts_with(&format!("{}.corrupt-", prefix)) && n.ends_with(".json"))
                .map(|n| self.0.join(n))
                .collect()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn named_pet(name: &str) -> PetState {
        let mut pet = PetState::new();
        pet.name = name.to_string();
        pet
    }

    fn stats_with_coins(coins: u64) -> GameStats {
        let mut stats = GameStats::new();
        stats.coins = coins;
        stats
    }

    const BROKEN_JSON: &str = "{\"name\": \"half-writ";

    #[test]
    fn missing_save_loads_as_missing() {
        let dir = TempDir::new();
        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Missing));
        assert!(matches!(load_stats_from(dir.path()), LoadOutcome::Missing));
    }

    #[test]
    fn state_round_trips() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("mochi")).unwrap();

        match load_state_from(dir.path()) {
            LoadOutcome::Loaded(pet) => assert_eq!(pet.name, "mochi"),
            other => panic!("expected Loaded, got {:?}", other),
        }
    }

    #[test]
    fn stats_round_trip() {
        let dir = TempDir::new();
        save_stats_in(dir.path(), &stats_with_coins(42)).unwrap();

        match load_stats_from(dir.path()) {
            LoadOutcome::Loaded(stats) => assert_eq!(stats.coins, 42),
            other => panic!("expected Loaded, got {:?}", other),
        }
    }

    #[test]
    fn save_leaves_no_temp_files_behind() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("a")).unwrap();
        save_state_in(dir.path(), &named_pet("b")).unwrap();

        assert_eq!(dir.file_names(), vec!["pet-state.json", "pet-state.json.bak"]);
    }

    #[test]
    fn save_keeps_previous_generation_as_bak() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("first")).unwrap();
        save_state_in(dir.path(), &named_pet("second")).unwrap();

        let bak = fs::read_to_string(dir.path().join("pet-state.json.bak")).unwrap();
        let bak_pet: PetState = serde_json::from_str(&bak).unwrap();
        assert_eq!(bak_pet.name, "first");
    }

    #[test]
    fn first_save_creates_no_bak() {
        let dir = TempDir::new();
        save_stats_in(dir.path(), &stats_with_coins(1)).unwrap();

        assert_eq!(dir.file_names(), vec!["game-stats.json"]);
    }

    #[test]
    fn broken_state_is_moved_aside_with_contents_intact() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATE_FILE), BROKEN_JSON).unwrap();

        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));

        assert!(!dir.path().join(STATE_FILE).exists());
        let corrupt = dir.corrupt_files("pet-state");
        assert_eq!(corrupt.len(), 1, "files: {:?}", dir.file_names());
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), BROKEN_JSON);
    }

    #[test]
    fn broken_stats_is_moved_aside_with_contents_intact() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATS_FILE), BROKEN_JSON).unwrap();

        assert!(matches!(load_stats_from(dir.path()), LoadOutcome::Reset));

        let corrupt = dir.corrupt_files("game-stats");
        assert_eq!(corrupt.len(), 1, "files: {:?}", dir.file_names());
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), BROKEN_JSON);
    }

    #[test]
    fn saving_after_reset_does_not_touch_the_moved_aside_file() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATE_FILE), BROKEN_JSON).unwrap();
        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));

        save_state_in(dir.path(), &PetState::new()).unwrap();
        save_state_in(dir.path(), &PetState::new()).unwrap();

        let corrupt = dir.corrupt_files("pet-state");
        assert_eq!(corrupt.len(), 1);
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), BROKEN_JSON);
    }

    #[test]
    fn repeated_corruption_never_overwrites_an_earlier_corrupt_file() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATE_FILE), "first broken").unwrap();
        load_state_from(dir.path());
        fs::write(dir.path().join(STATE_FILE), "second broken").unwrap();
        load_state_from(dir.path());

        let mut contents: Vec<String> = dir
            .corrupt_files("pet-state")
            .iter()
            .map(|p| fs::read_to_string(p).unwrap())
            .collect();
        contents.sort();
        assert_eq!(contents, vec!["first broken", "second broken"]);
    }

    #[test]
    fn broken_state_with_good_bak_is_recovered_from_bak() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("old")).unwrap();
        save_state_in(dir.path(), &named_pet("new")).unwrap();
        fs::write(dir.path().join(STATE_FILE), BROKEN_JSON).unwrap();

        match load_state_from(dir.path()) {
            LoadOutcome::Recovered(pet) => assert_eq!(pet.name, "old"),
            other => panic!("expected Recovered, got {:?}", other),
        }
        let corrupt = dir.corrupt_files("pet-state");
        assert_eq!(corrupt.len(), 1);
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), BROKEN_JSON);
    }

    #[test]
    fn broken_stats_with_good_bak_is_recovered_from_bak() {
        let dir = TempDir::new();
        save_stats_in(dir.path(), &stats_with_coins(100)).unwrap();
        save_stats_in(dir.path(), &stats_with_coins(120)).unwrap();
        fs::write(dir.path().join(STATS_FILE), BROKEN_JSON).unwrap();

        match load_stats_from(dir.path()) {
            LoadOutcome::Recovered(stats) => assert_eq!(stats.coins, 100),
            other => panic!("expected Recovered, got {:?}", other),
        }
    }

    #[test]
    fn broken_state_with_broken_bak_resets() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATE_FILE), BROKEN_JSON).unwrap();
        fs::write(dir.path().join("pet-state.json.bak"), "also broken").unwrap();

        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));
        assert_eq!(dir.corrupt_files("pet-state").len(), 1);
    }

    #[test]
    fn broken_bak_is_moved_aside_too() {
        let dir = TempDir::new();
        fs::write(dir.path().join(STATE_FILE), BROKEN_JSON).unwrap();
        fs::write(dir.path().join("pet-state.json.bak"), "also broken").unwrap();

        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));

        assert!(!dir.path().join("pet-state.json.bak").exists());
        let corrupt = dir.corrupt_files("pet-state.bak");
        assert_eq!(corrupt.len(), 1, "files: {:?}", dir.file_names());
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), "also broken");
    }

    #[test]
    fn missing_state_with_broken_bak_resets_and_keeps_the_bak() {
        let dir = TempDir::new();
        fs::write(dir.path().join("pet-state.json.bak"), "also broken").unwrap();

        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));
        assert_eq!(dir.corrupt_files("pet-state.bak").len(), 1);
    }

    #[test]
    fn identical_save_is_skipped() {
        let dir = TempDir::new();
        save_stats_in(dir.path(), &stats_with_coins(3)).unwrap();
        save_stats_in(dir.path(), &stats_with_coins(3)).unwrap();

        // An unchanged save neither rewrites the file nor rotates the backup.
        assert_eq!(dir.file_names(), vec!["game-stats.json"]);
    }

    #[test]
    fn corrupt_path_never_reuses_a_name() {
        let dir = TempDir::new();
        let path = dir.path().join(STATE_FILE);
        let first = corrupt_path(&path, "pet-state", 1_700_000_000);
        fs::write(&first, "taken").unwrap();

        let second = corrupt_path(&path, "pet-state", 1_700_000_000);

        assert_eq!(first.file_name().unwrap(), "pet-state.corrupt-1700000000.json");
        assert_eq!(second.file_name().unwrap(), "pet-state.corrupt-1700000000-1.json");
    }

    #[test]
    fn missing_state_with_good_bak_is_recovered_from_bak() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("kept")).unwrap();
        save_state_in(dir.path(), &named_pet("lost")).unwrap();
        fs::remove_file(dir.path().join(STATE_FILE)).unwrap();

        match load_state_from(dir.path()) {
            LoadOutcome::Recovered(pet) => assert_eq!(pet.name, "kept"),
            other => panic!("expected Recovered, got {:?}", other),
        }
    }

    #[test]
    fn unreadable_state_is_moved_aside() {
        let dir = TempDir::new();
        // A directory where the file should be makes the read itself fail.
        fs::create_dir(dir.path().join(STATE_FILE)).unwrap();

        assert!(matches!(load_state_from(dir.path()), LoadOutcome::Reset));
        assert!(!dir.path().join(STATE_FILE).exists());
        assert_eq!(dir.corrupt_files("pet-state").len(), 1);
    }

    #[test]
    fn failed_save_leaves_existing_save_intact() {
        let dir = TempDir::new();
        save_state_in(dir.path(), &named_pet("safe")).unwrap();
        let before = fs::read_to_string(dir.path().join(STATE_FILE)).unwrap();
        // Block the temp file so the write fails before anything is replaced.
        fs::create_dir(dir.path().join(".pet-state.json.tmp")).unwrap();

        assert!(save_state_in(dir.path(), &named_pet("doomed")).is_err());

        assert_eq!(fs::read_to_string(dir.path().join(STATE_FILE)).unwrap(), before);
        assert!(!dir.path().join(".pet-state.json.bak.tmp").exists());
        match load_state_from(dir.path()) {
            LoadOutcome::Loaded(pet) => assert_eq!(pet.name, "safe"),
            other => panic!("expected Loaded, got {:?}", other),
        }
    }

    fn companion_with_bond(bond: u16) -> CompanionState {
        CompanionState { bond, ..CompanionState::default() }
    }

    #[test]
    fn companion_round_trips() {
        let dir = TempDir::new();
        save_companion_in(dir.path(), &companion_with_bond(280)).unwrap();

        match load_companion_from(dir.path()) {
            LoadOutcome::Loaded(companion) => assert_eq!(companion.bond, 280),
            other => panic!("expected Loaded, got {:?}", other),
        }
        assert_eq!(dir.file_names(), vec!["companion.json"]);
    }

    #[test]
    fn missing_companion_loads_as_missing() {
        let dir = TempDir::new();
        assert!(matches!(load_companion_from(dir.path()), LoadOutcome::Missing));
    }

    #[test]
    fn broken_companion_is_moved_aside_with_contents_intact() {
        let dir = TempDir::new();
        fs::write(dir.path().join(COMPANION_FILE), BROKEN_JSON).unwrap();

        assert!(matches!(load_companion_from(dir.path()), LoadOutcome::Reset));

        assert!(!dir.path().join(COMPANION_FILE).exists());
        let corrupt = dir.corrupt_files("companion");
        assert_eq!(corrupt.len(), 1, "files: {:?}", dir.file_names());
        assert_eq!(fs::read_to_string(&corrupt[0]).unwrap(), BROKEN_JSON);
    }

    #[test]
    fn broken_companion_with_good_bak_is_recovered_from_bak() {
        let dir = TempDir::new();
        save_companion_in(dir.path(), &companion_with_bond(100)).unwrap();
        save_companion_in(dir.path(), &companion_with_bond(120)).unwrap();
        fs::write(dir.path().join(COMPANION_FILE), BROKEN_JSON).unwrap();

        match load_companion_from(dir.path()) {
            LoadOutcome::Recovered(companion) => assert_eq!(companion.bond, 100),
            other => panic!("expected Recovered, got {:?}", other),
        }
    }

    #[cfg(unix)]
    #[test]
    fn save_into_read_only_dir_fails_and_leaves_existing_save_intact() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new();
        save_stats_in(dir.path(), &stats_with_coins(7)).unwrap();
        let before = fs::read_to_string(dir.path().join(STATS_FILE)).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let result = save_stats_in(dir.path(), &stats_with_coins(0));

        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(dir.path().join(STATS_FILE)).unwrap(), before);
    }
}
