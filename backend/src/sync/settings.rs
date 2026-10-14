//! The SYNC FOLDER setting (#38), in the local `sync-settings.json` next to
//! the save. Never synced: each Mac chooses its own folder.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::pet::GameStats;
use crate::storage::{self, LoadOutcome};
use super::folder::{CONFLICTS_DIR, DEVICES_DIR};

pub const SETTINGS_FILE: &str = "sync-settings.json";

// Shown as they are in the SYNC settings, so short and upper case.
pub const ERR_EMPTY: &str = "ENTER A FOLDER";
pub const ERR_NOT_A_PATH: &str = "NOT A PATH";
pub const ERR_NOT_FULL: &str = "USE A FULL PATH";
pub const ERR_NOT_FOUND: &str = "FOLDER NOT FOUND";
pub const ERR_NOT_A_FOLDER: &str = "NOT A FOLDER";
pub const ERR_CANNOT_WRITE_HERE: &str = "CANNOT WRITE HERE";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSettings {
    pub enabled: bool,
    /// The folder as it was checked: absolute, symlinks resolved.
    pub path: Option<PathBuf>,
    /// The save from before sync has gone into this Mac's ledger. Done once
    /// only, even when `device.json` is made again.
    pub migrated: bool,
    /// The stats when sync was last turned off. Turning it on again adds
    /// what was done in between to the ledger.
    pub paused_at: Option<StatsMark>,
}

/// The parts of `GameStats` the ledger counts, at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsMark {
    pub feeds: u64,
    pub plays: u64,
    pub sleeps: u64,
    pub pets_raised: u64,
    pub coins: u64,
    pub total_coins_earned: u64,
}

impl StatsMark {
    pub fn of(stats: &GameStats) -> Self {
        Self {
            feeds: stats.total_feeds,
            plays: stats.total_plays,
            sleeps: stats.total_sleeps,
            pets_raised: stats.total_pets_raised,
            coins: stats.coins,
            total_coins_earned: stats.total_coins_earned,
        }
    }
}

/// A broken settings file reads as off, so sync never starts by accident.
pub fn load(dir: &Path) -> SyncSettings {
    match storage::load_json::<SyncSettings>(&dir.join(SETTINGS_FILE)) {
        LoadOutcome::Loaded(settings) | LoadOutcome::Recovered(settings) => settings,
        LoadOutcome::Missing | LoadOutcome::Reset => SyncSettings::default(),
    }
}

pub fn save(dir: &Path, settings: &SyncSettings) -> Result<(), String> {
    storage::save_json(&dir.join(SETTINGS_FILE), settings)
}

/// Checks the folder typed in SYNC and makes `devices/` and `conflicts/` in
/// it. `~` is the home folder. Returns the folder as it will be saved.
pub fn validate_folder(raw: &str) -> Result<PathBuf, String> {
    validate_folder_with_home(raw, dirs::home_dir().as_deref())
}

pub fn validate_folder_with_home(raw: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let path = expand(raw, home)?;
    let folder = fs::canonicalize(&path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ERR_NOT_FOUND.to_string(),
        _ => ERR_NOT_A_FOLDER.to_string(),
    })?;
    if !folder.is_dir() {
        return Err(ERR_NOT_A_FOLDER.to_string());
    }
    let writable = probe_write(&folder)
        .and_then(|()| fs::create_dir_all(folder.join(DEVICES_DIR)))
        .and_then(|()| fs::create_dir_all(folder.join(CONFLICTS_DIR)));
    if let Err(e) = writable {
        eprintln!("Sync folder {} is not writable: {}", folder.display(), e);
        return Err(ERR_CANNOT_WRITE_HERE.to_string());
    }
    Ok(folder)
}

fn expand(raw: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(ERR_EMPTY.to_string());
    }
    if text.contains('\0') {
        return Err(ERR_NOT_A_PATH.to_string());
    }
    let path = match (text, text.strip_prefix("~/"), home) {
        ("~", _, Some(home)) => home.to_path_buf(),
        (_, Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(text),
    };
    if !path.is_absolute() {
        return Err(ERR_NOT_FULL.to_string());
    }
    Ok(path)
}

/// Creates and removes a hidden file, the only sure test that writing works.
fn probe_write(folder: &Path) -> io::Result<()> {
    let probe = folder.join(format!(".dot-write-test-{}", uuid::Uuid::new_v4()));
    OpenOptions::new().write(true).create_new(true).open(&probe)?;
    fs::remove_file(&probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("dot-sync-settings-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> =
            fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_good_folder_is_normalized_and_gets_devices_and_conflicts() {
        let dir = TempDir::new();
        let messy = format!("  {}/./sub/..  ", dir.0.display());
        fs::create_dir(dir.0.join("sub")).unwrap();

        let folder = validate_folder_with_home(&messy, None).unwrap();

        assert_eq!(folder, dir.0);
        assert_eq!(names(&dir.0), vec!["conflicts", "devices", "sub"]);
    }

    #[test]
    fn tilde_is_the_home_folder() {
        let home = TempDir::new();
        fs::create_dir(home.0.join("Sync")).unwrap();
        assert_eq!(validate_folder_with_home("~/Sync", Some(&home.0)).unwrap(), home.0.join("Sync"));
        assert_eq!(validate_folder_with_home("~", Some(&home.0)).unwrap(), home.0);
    }

    #[test]
    fn bad_input_is_refused_before_touching_the_disk() {
        assert_eq!(validate_folder_with_home("   ", None), Err(ERR_EMPTY.to_string()));
        assert_eq!(validate_folder_with_home("a\0b", None), Err(ERR_NOT_A_PATH.to_string()));
        assert_eq!(validate_folder_with_home("Dropbox/dot", None), Err(ERR_NOT_FULL.to_string()));
        assert_eq!(validate_folder_with_home("~/dot", None), Err(ERR_NOT_FULL.to_string()));
    }

    #[test]
    fn a_missing_folder_is_refused() {
        let dir = TempDir::new();
        let missing = dir.0.join("nope");
        assert_eq!(validate_folder_with_home(missing.to_str().unwrap(), None), Err(ERR_NOT_FOUND.to_string()));
        assert!(!missing.exists());
    }

    #[test]
    fn a_file_is_refused() {
        let dir = TempDir::new();
        let file = dir.0.join("pet.json");
        fs::write(&file, "{}").unwrap();
        assert_eq!(validate_folder_with_home(file.to_str().unwrap(), None), Err(ERR_NOT_A_FOLDER.to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_is_refused_and_left_as_it_was() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        let locked = dir.0.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

        let result = validate_folder_with_home(locked.to_str().unwrap(), None);

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result, Err(ERR_CANNOT_WRITE_HERE.to_string()));
        assert!(names(&locked).is_empty());
    }

    #[test]
    fn settings_round_trip_and_a_broken_file_reads_as_off() {
        let dir = TempDir::new();
        assert_eq!(load(&dir.0), SyncSettings::default());
        let settings = SyncSettings {
            enabled: true,
            path: Some(dir.0.clone()),
            migrated: true,
            paused_at: Some(StatsMark { feeds: 3, ..StatsMark::default() }),
        };
        save(&dir.0, &settings).unwrap();
        assert_eq!(load(&dir.0), settings);

        fs::write(dir.0.join(SETTINGS_FILE), "{ broken").unwrap();
        fs::remove_file(dir.0.join(format!("{}.bak", SETTINGS_FILE))).ok();
        assert!(!load(&dir.0).enabled);
    }
}
