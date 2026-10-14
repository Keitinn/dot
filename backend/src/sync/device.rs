//! This Mac's device id, kept in the local `device.json` (never synced).

use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::storage::{self, LoadOutcome};

pub const DEVICE_FILE: &str = "device.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceRecord {
    pub device_id: String,
    /// Hash of the hardware UUID the id was made on; empty if it could not be read.
    pub hardware_hash: String,
}

/// This Mac's record in Application Support, created on first use.
pub fn load_or_create() -> Result<DeviceRecord, String> {
    load_or_create_in(&storage::config_dir()?, &this_hardware_uuid())
}

/// This Mac's hardware UUID, or empty (logged) when it cannot be read.
pub fn this_hardware_uuid() -> String {
    platform::hardware_uuid().unwrap_or_else(|| {
        eprintln!("Hardware UUID is unavailable; a copied device.json will not be detected");
        String::new()
    })
}

/// Loads `dir/device.json`, or makes a new id. A record made on other
/// hardware (Migration Assistant or a restore copied it here) gets a new id,
/// so two Macs never write the same device file. `hardware_uuid` is empty
/// when it could not be read; then the saved id is kept as it is.
pub fn load_or_create_in(dir: &Path, hardware_uuid: &str) -> Result<DeviceRecord, String> {
    let current = hardware_hash(hardware_uuid);
    let saved = match storage::load_json::<DeviceRecord>(&dir.join(DEVICE_FILE)) {
        LoadOutcome::Loaded(record) | LoadOutcome::Recovered(record) => Some(record),
        LoadOutcome::Missing | LoadOutcome::Reset => None,
    }
    .filter(|record| is_valid_id(&record.device_id));

    let record = match saved {
        Some(record) if current.is_empty() || record.hardware_hash == current => return Ok(record),
        // Made before the hardware UUID could be read: this is the same Mac as far as we know
        Some(record) if record.hardware_hash.is_empty() => DeviceRecord { hardware_hash: current, ..record },
        _ => DeviceRecord { device_id: uuid::Uuid::new_v4().to_string(), hardware_hash: current },
    };
    storage::save_json(&dir.join(DEVICE_FILE), &record)?;
    Ok(record)
}

/// A device id is used as a file name, so only letters, digits and `-`.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// FNV-1a (64 bit) of the hardware UUID. Only tells two Macs apart and stays
/// the same across Rust versions, unlike `DefaultHasher`; the raw UUID is not
/// written anywhere.
fn hardware_hash(hardware_uuid: &str) -> String {
    let hardware_uuid = hardware_uuid.trim();
    if hardware_uuid.is_empty() {
        return String::new();
    }
    let hash = hardware_uuid
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
    format!("{:016x}", hash)
}

/// The `IOPlatformUUID` value in `ioreg -rd1 -c IOPlatformExpertDevice` output.
fn parse_platform_uuid(ioreg: &str) -> Option<String> {
    ioreg
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "\"IOPlatformUUID\"").then(|| value.trim().trim_matches('"').to_string())
        })
        .filter(|uuid| !uuid.is_empty())
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::Command;

    /// Needs no permission. `ioreg` ships with macOS.
    pub fn hardware_uuid() -> Option<String> {
        let output = Command::new("/usr/sbin/ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        super::parse_platform_uuid(&String::from_utf8_lossy(&output.stdout))
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn hardware_uuid() -> Option<String> {
        None
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
            let path = std::env::temp_dir().join(format!("dot-device-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const HW_ONE: &str = "11111111-2222-3333-4444-555555555555";
    const HW_TWO: &str = "99999999-8888-7777-6666-555555555555";

    #[test]
    fn first_use_creates_an_id_and_keeps_it() {
        let dir = TempDir::new();
        let first = load_or_create_in(&dir.0, HW_ONE).unwrap();
        assert!(is_valid_id(&first.device_id));
        assert_eq!(first.hardware_hash, hardware_hash(HW_ONE));
        assert_eq!(load_or_create_in(&dir.0, HW_ONE).unwrap(), first);
    }

    #[test]
    fn a_copy_on_other_hardware_gets_a_new_id() {
        let dir = TempDir::new();
        let original = load_or_create_in(&dir.0, HW_ONE).unwrap();

        // device.json arrived on another Mac
        let copied = load_or_create_in(&dir.0, HW_TWO).unwrap();

        assert_ne!(copied.device_id, original.device_id);
        assert_eq!(copied.hardware_hash, hardware_hash(HW_TWO));
        assert_eq!(load_or_create_in(&dir.0, HW_TWO).unwrap(), copied);
    }

    #[test]
    fn unreadable_hardware_keeps_the_saved_id() {
        let dir = TempDir::new();
        let saved = load_or_create_in(&dir.0, HW_ONE).unwrap();
        assert_eq!(load_or_create_in(&dir.0, "").unwrap(), saved);
        // And the saved hash is not wiped
        assert_eq!(load_or_create_in(&dir.0, HW_ONE).unwrap(), saved);
    }

    #[test]
    fn an_id_made_without_hardware_adopts_it_later() {
        let dir = TempDir::new();
        let without = load_or_create_in(&dir.0, "").unwrap();
        assert_eq!(without.hardware_hash, "");

        let with = load_or_create_in(&dir.0, HW_ONE).unwrap();

        assert_eq!(with.device_id, without.device_id);
        assert_eq!(with.hardware_hash, hardware_hash(HW_ONE));
    }

    #[test]
    fn a_broken_device_file_is_moved_aside_and_replaced() {
        let dir = TempDir::new();
        fs::write(dir.0.join(DEVICE_FILE), "{ broken").unwrap();

        let record = load_or_create_in(&dir.0, HW_ONE).unwrap();

        assert!(is_valid_id(&record.device_id));
        let moved = fs::read_dir(&dir.0)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("device.corrupt-"))
            .count();
        assert_eq!(moved, 1);
    }

    #[test]
    fn an_unsafe_saved_id_is_replaced() {
        let dir = TempDir::new();
        fs::write(dir.0.join(DEVICE_FILE), r#"{"device_id":"../x","hardware_hash":""}"#).unwrap();
        let record = load_or_create_in(&dir.0, HW_ONE).unwrap();
        assert_ne!(record.device_id, "../x");
        assert!(is_valid_id(&record.device_id));
    }

    #[test]
    fn hash_is_stable_and_hides_the_uuid() {
        assert_eq!(hardware_hash("a"), "af63dc4c8601ec8c");
        assert_eq!(hardware_hash(""), "");
        assert_eq!(hardware_hash(HW_ONE), hardware_hash(&format!(" {} ", HW_ONE)));
        assert!(!hardware_hash(HW_ONE).contains("1111"));
    }

    #[test]
    fn platform_uuid_is_read_from_ioreg_output() {
        let output = r#"+-o J316sAP  <class IOPlatformExpertDevice, id 0x100000202, registered>
    {
      "IOPlatformSerialNumber" = "XXXXXXXXXX"
      "IOPlatformUUID" = "11111111-2222-3333-4444-555555555555"
      "IOPolledInterface" = "AppleARMWatchdogTimerHibernateHandler is not serializable"
    }"#;
        assert_eq!(parse_platform_uuid(output).as_deref(), Some(HW_ONE));
        assert_eq!(parse_platform_uuid("nothing here"), None);
        assert_eq!(parse_platform_uuid(r#""IOPlatformUUID" = """#), None);
    }

    #[test]
    fn device_ids_must_be_safe_file_names() {
        assert!(is_valid_id("aaaaaaaa-0000-4000-8000-000000000001"));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("../etc"));
        assert!(!is_valid_id("a b"));
        assert!(!is_valid_id(&"a".repeat(65)));
    }
}
