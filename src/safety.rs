//! ZFS Version Safety Module
//!
//! Detects ZFS version at startup and manages safety lock state.
//! Unapproved ZFS versions trigger read-only mode until explicitly overridden.
//!
//! Version requirements are loaded from settings.json:
//! - min_zfs_version: Minimum supported ZFS version (e.g., "2.0")
//! - max_zfs_version: Maximum supported ZFS version (e.g., "2.3")

use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{SafetyState, ZfsVersionInfo};

/// Settings loaded from settings.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub safety: SafetySettings,
}

/// Safety-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySettings {
    pub min_zfs_version: String,
    pub max_zfs_version: String,
}

impl Default for SafetySettings {
    fn default() -> Self {
        SafetySettings {
            min_zfs_version: "2.0".to_string(),
            max_zfs_version: "2.3".to_string(),
        }
    }
}

/// Load settings from settings.json or use defaults
/// Looks for settings.json in the same directory as the executable
pub fn load_settings() -> Settings {
    let settings_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("settings.json")))
        .unwrap_or_else(|| std::path::PathBuf::from("settings.json"));

    match fs::read_to_string(&settings_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse {}: {}. Using defaults.",
                    settings_path.display(),
                    e
                );
                Settings::default()
            }
        },
        Err(_) => {
            eprintln!(
                "Note: {} not found, using default values.",
                settings_path.display()
            );
            Settings::default()
        }
    }
}

/// Safety manager for ZFS version validation
#[derive(Clone)]
pub struct SafetyManager {
    state: Arc<RwLock<SafetyState>>,
    settings: SafetySettings,
}

impl SafetyManager {
    /// Initialize safety manager by detecting ZFS version
    pub fn new() -> Result<Self, String> {
        let settings = load_settings().safety;
        let version_info = detect_zfs_version()?;
        let compatible = is_version_in_range(&version_info, &settings);

        let locked = !compatible;
        let lock_reason = if locked {
            Some(format!(
                "ZFS version {} is outside approved range ({} - {})",
                version_info.semantic_version, settings.min_zfs_version, settings.max_zfs_version
            ))
        } else {
            None
        };

        let state = SafetyState {
            locked,
            zfs_version: version_info,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            approved_versions: vec![format!(
                "{} - {}",
                settings.min_zfs_version, settings.max_zfs_version
            )],
            compatible,
            lock_reason,
            override_at: None,
        };

        Ok(SafetyManager {
            state: Arc::new(RwLock::new(state)),
            settings,
        })
    }

    /// Check if safety lock is active
    pub fn is_locked(&self) -> bool {
        self.state.read().unwrap().locked
    }

    /// Get current safety state
    pub fn get_state(&self) -> SafetyState {
        self.state.read().unwrap().clone()
    }

    /// Get the settings
    pub fn get_settings(&self) -> &SafetySettings {
        &self.settings
    }

    /// Override safety lock (unlock)
    pub fn override_lock(&self) -> Result<(), String> {
        let mut state = self.state.write().unwrap();
        if !state.locked {
            return Err("Safety lock is not active".to_string());
        }
        state.locked = false;
        state.override_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        Ok(())
    }

    /// Get lock reason for error messages
    pub fn get_lock_message(&self) -> String {
        let state = self.state.read().unwrap();
        format!(
            "Safety lock active: ZFS version {} is not approved (requires {} - {}). Use POST /v1/safety to override.",
            state.zfs_version.semantic_version,
            self.settings.min_zfs_version,
            self.settings.max_zfs_version
        )
    }
}

/// Detect ZFS version using multiple methods
fn detect_zfs_version() -> Result<ZfsVersionInfo, String> {
    // Method 1: Try `zfs version` command
    if let Ok(version) = detect_via_zfs_command() {
        return Ok(version);
    }

    // Method 2: Try `modinfo zfs` for kernel module version
    if let Ok(version) = detect_via_modinfo() {
        return Ok(version);
    }

    // Method 3: Try reading /sys/module/zfs/version
    if let Ok(version) = detect_via_sysfs() {
        return Ok(version);
    }

    Err("Failed to detect ZFS version using any method".to_string())
}

fn detect_via_zfs_command() -> Result<ZfsVersionInfo, String> {
    let output = Command::new("zfs")
        .arg("version")
        .output()
        .map_err(|e| format!("Failed to run zfs version: {}", e))?;

    if !output.status.success() {
        return Err("zfs version command failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse output like "zfs-2.1.5-1ubuntu6~22.04.1"
    for line in stdout.lines() {
        if line.starts_with("zfs-") {
            let full = line.trim_start_matches("zfs-");
            return parse_version_string(full, "zfs version");
        }
    }

    Err("Could not parse zfs version output".to_string())
}

fn detect_via_modinfo() -> Result<ZfsVersionInfo, String> {
    let output = Command::new("modinfo")
        .args(["-F", "version", "zfs"])
        .output()
        .map_err(|e| format!("Failed to run modinfo: {}", e))?;

    if !output.status.success() {
        return Err("modinfo command failed".to_string());
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if version.is_empty() {
        return Err("Empty modinfo output".to_string());
    }

    parse_version_string(&version, "modinfo")
}

fn detect_via_sysfs() -> Result<ZfsVersionInfo, String> {
    let version = std::fs::read_to_string("/sys/module/zfs/version")
        .map_err(|e| format!("Failed to read sysfs: {}", e))?
        .trim()
        .to_string();

    parse_version_string(&version, "sysfs")
}

fn parse_version_string(full: &str, method: &str) -> Result<ZfsVersionInfo, String> {
    // Extract semantic version (e.g., "2.1.5" from "2.1.5-1ubuntu6~22.04.1")
    let semantic = full
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or(full);

    let parts: Vec<&str> = semantic.split('.').collect();

    let major = parts
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or("Failed to parse major version")?;

    let minor = parts
        .get(1)
        .and_then(|s| s.parse().ok())
        .ok_or("Failed to parse minor version")?;

    let patch = parts.get(2).and_then(|s| s.parse().ok());

    Ok(ZfsVersionInfo {
        full_version: full.to_string(),
        semantic_version: semantic.to_string(),
        major,
        minor,
        patch,
        detection_method: method.to_string(),
    })
}

/// Parse a version string like "2.0" or "2.1.5" into (major, minor)
fn parse_min_max_version(version_str: &str) -> (u32, u32) {
    let parts: Vec<&str> = version_str.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor)
}

/// Check if detected version is within the min/max range
fn is_version_in_range(version: &ZfsVersionInfo, settings: &SafetySettings) -> bool {
    let (min_major, min_minor) = parse_min_max_version(&settings.min_zfs_version);
    let (max_major, max_minor) = parse_min_max_version(&settings.max_zfs_version);

    let detected = (version.major, version.minor);
    let min = (min_major, min_minor);
    let max = (max_major, max_minor);

    // Version is valid if: min <= detected <= max
    detected >= min && detected <= max
}

// ============================================================================
// UNIT TESTS
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_string_full() {
        let info = parse_version_string("2.1.5-1ubuntu6~22.04.1", "test").unwrap();
        assert_eq!(info.full_version, "2.1.5-1ubuntu6~22.04.1");
        assert_eq!(info.semantic_version, "2.1.5");
        assert_eq!(info.major, 2);
        assert_eq!(info.minor, 1);
        assert_eq!(info.patch, Some(5));
    }

    #[test]
    fn test_parse_version_string_simple() {
        let info = parse_version_string("2.2.0", "test").unwrap();
        assert_eq!(info.semantic_version, "2.2.0");
        assert_eq!(info.major, 2);
        assert_eq!(info.minor, 2);
        assert_eq!(info.patch, Some(0));
    }

    #[test]
    fn test_parse_min_max_version() {
        assert_eq!(parse_min_max_version("2.0"), (2, 0));
        assert_eq!(parse_min_max_version("2.3"), (2, 3));
        assert_eq!(parse_min_max_version("2.1.5"), (2, 1));
    }

    fn make_version(major: u32, minor: u32, patch: u32) -> ZfsVersionInfo {
        ZfsVersionInfo {
            full_version: format!("{}.{}.{}", major, minor, patch),
            semantic_version: format!("{}.{}.{}", major, minor, patch),
            major,
            minor,
            patch: Some(patch),
            detection_method: "test".to_string(),
        }
    }

    fn default_settings() -> SafetySettings {
        SafetySettings {
            min_zfs_version: "2.0".to_string(),
            max_zfs_version: "2.3".to_string(),
        }
    }

    #[test]
    fn test_version_in_range_exact_min() {
        let settings = default_settings();
        let version = make_version(2, 0, 0);
        assert!(is_version_in_range(&version, &settings));
    }

    #[test]
    fn test_version_in_range_exact_max() {
        let settings = default_settings();
        let version = make_version(2, 3, 0);
        assert!(is_version_in_range(&version, &settings));
    }

    #[test]
    fn test_version_in_range_middle() {
        let settings = default_settings();
        let version = make_version(2, 1, 5);
        assert!(is_version_in_range(&version, &settings));
    }

    #[test]
    fn test_version_below_min() {
        let settings = default_settings();
        let version = make_version(1, 8, 0);
        assert!(!is_version_in_range(&version, &settings));
    }

    #[test]
    fn test_version_above_max() {
        let settings = default_settings();
        let version = make_version(2, 4, 0);
        assert!(!is_version_in_range(&version, &settings));
    }

    #[test]
    fn test_version_way_above_max() {
        let settings = default_settings();
        let version = make_version(3, 0, 0);
        assert!(!is_version_in_range(&version, &settings));
    }
}
