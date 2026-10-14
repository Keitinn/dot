//! The shared folder: `devices/<device_id>.json`, one file per Mac. Only
//! reads and writes the folder it is given; `runtime` decides when.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::storage;
use super::device::is_valid_id;
use super::merge::{merge, Merged};
use super::snapshot::DeviceSnapshot;

pub const DEVICES_DIR: &str = "devices";
pub const CONFLICTS_DIR: &str = "conflicts";

/// Writes this device's file atomically, as `<root>/devices/<device_id>.json`.
pub fn write_own(root: &Path, snapshot: &DeviceSnapshot) -> Result<PathBuf, String> {
    if !is_valid_id(&snapshot.device_id) {
        return Err(format!("not a device id: {:?}", snapshot.device_id));
    }
    let dir = root.join(DEVICES_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {}", dir.display(), e))?;
    let path = dir.join(format!("{}.json", snapshot.device_id));
    storage::write_json(&path, snapshot)?;
    Ok(path)
}

#[derive(Debug, Default)]
pub struct ReadReport {
    pub read: usize,
    /// Files that could not be read or parsed; the last good version is used.
    pub skipped: Vec<PathBuf>,
    /// Conflict copies, merged and then moved to `conflicts/`.
    pub moved_to_conflicts: Vec<PathBuf>,
    /// Conflict copies holding something the device's own file lacks. Merged,
    /// but left in place for the other Macs until the owner takes them in.
    pub kept_conflicts: Vec<PathBuf>,
}

/// Every device file read so far, joined per device. A file that cannot be
/// read this time leaves its last good version in place, so an unreadable
/// file never looks like a new pet. Kept locally between launches (#38).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncCache {
    devices: BTreeMap<String, DeviceSnapshot>,
}

impl SyncCache {
    pub fn absorb(&mut self, snapshot: &DeviceSnapshot) {
        self.devices
            .entry(snapshot.device_id.clone())
            .or_insert_with(|| DeviceSnapshot::new(&snapshot.device_id))
            .absorb(snapshot);
    }

    /// Joins another cache; in any order, as often as wanted.
    pub fn join(&mut self, other: &SyncCache) {
        for snapshot in other.devices.values() {
            self.absorb(snapshot);
        }
    }

    pub fn device(&self, device_id: &str) -> Option<&DeviceSnapshot> {
        self.devices.get(device_id)
    }

    pub fn snapshots(&self) -> impl Iterator<Item = &DeviceSnapshot> {
        self.devices.values()
    }

    /// Reads every `devices/*.json` under `root` into the cache. A copy whose
    /// name is not `<its device_id>.json` (`<id> 2.json` from iCloud, a
    /// Dropbox conflicted copy) is merged like any other version of that
    /// device. It moves to `conflicts/` only once the device's own file holds
    /// everything in it; until then moving it would hide its data from the
    /// other Macs.
    pub fn read_folder(&mut self, root: &Path) -> ReadReport {
        let mut report = ReadReport::default();
        let dir = root.join(DEVICES_DIR);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    eprintln!("Could not list {}: {}", dir.display(), e);
                }
                return report;
            }
        };
        let mut paths: Vec<PathBuf> =
            entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_device_file(p)).collect();
        paths.sort();

        for path in paths {
            let snapshot = match read_snapshot(&path) {
                Ok(snapshot) => snapshot,
                Err(e) => {
                    // Maybe still arriving or evicted: keep the last good version
                    eprintln!("Skipping {}: {}", path.display(), e);
                    report.skipped.push(path);
                    continue;
                }
            };
            self.absorb(&snapshot);
            report.read += 1;
            if path.file_stem().and_then(|s| s.to_str()) == Some(snapshot.device_id.as_str()) {
                continue;
            }
            if !own_file_covers(&dir, &snapshot) {
                report.kept_conflicts.push(path);
                continue;
            }
            match move_to_conflicts(root, &path) {
                Ok(dest) => report.moved_to_conflicts.push(dest),
                Err(e) => eprintln!("Could not move {} to {}: {}", path.display(), CONFLICTS_DIR, e),
            }
        }
        report
    }

    pub fn merged(&self) -> Merged {
        merge(self.devices.values())
    }
}

/// `*.json`, not hidden (the atomic writer's scratch files start with `.`).
fn is_device_file(path: &Path) -> bool {
    let hidden = path.file_name().and_then(|n| n.to_str()).is_none_or(|n| n.starts_with('.'));
    !hidden && path.extension().is_some_and(|ext| ext == "json") && path.is_file()
}

fn read_snapshot(path: &Path) -> Result<DeviceSnapshot, String> {
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let snapshot: DeviceSnapshot = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if snapshot.device_id.is_empty() {
        return Err("no device_id".to_string());
    }
    Ok(snapshot)
}

/// Whether `<devices>/<copy's device_id>.json` already holds all of `copy`:
/// joining the copy into it changes nothing.
fn own_file_covers(devices: &Path, copy: &DeviceSnapshot) -> bool {
    let Ok(own) = read_snapshot(&devices.join(format!("{}.json", copy.device_id))) else {
        return false;
    };
    let mut current = DeviceSnapshot::new(&copy.device_id);
    current.absorb(&own);
    let mut with_copy = current.clone();
    with_copy.absorb(copy);
    serde_json::to_value(&current).ok() == serde_json::to_value(&with_copy).ok()
}

fn move_to_conflicts(root: &Path, path: &Path) -> io::Result<PathBuf> {
    let dir = root.join(CONFLICTS_DIR);
    fs::create_dir_all(&dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name.strip_suffix(".json").unwrap_or(&name).to_string();
    // Never overwrite an earlier conflict copy
    let dest = (0..)
        .map(|n| dir.join(if n == 0 { name.clone() } else { format!("{}-{}.json", stem, n) }))
        .find(|p| !p.exists())
        .expect("unbounded search always finds a free name");
    fs::rename(path, &dest)?;
    Ok(dest)
}
