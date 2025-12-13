// zfs_management/types.rs
// Public types for ZFS management operations

use std::collections::HashMap;

/// Pool status information
pub struct PoolStatus {
    pub name: String,
    pub health: String,
    pub size: u64,
    pub allocated: u64,
    pub free: u64,
    pub capacity: u8,
    pub vdevs: u32,
    pub errors: Option<String>,
}

/// Pool available for import
pub struct ImportablePool {
    pub name: String,
    pub health: String,
}

/// Type alias for ZFS error messages
pub type ZfsError = String;

/// Result of a successful rollback operation
pub struct RollbackResult {
    pub destroyed_snapshots: Option<Vec<String>>,
    pub destroyed_clones: Option<Vec<String>>,
}

/// Error from rollback operation
#[derive(Debug)]
pub enum RollbackError {
    /// Invalid request parameters
    InvalidRequest(String),
    /// Rollback blocked by safety checks
    Blocked {
        message: String,
        blocking_snapshots: Vec<String>,
        blocking_clones: Vec<String>,
    },
    /// ZFS operation failed
    ZfsError(String),
}

/// Scrub status information
/// Implementation via libzfs FFI bindings.
/// Extracts real scan progress from pool_scan_stat_t via nvlist.
pub struct ScrubStatus {
    pub pool_health: String,
    pub errors: Option<String>,
    pub state: String,
    pub function: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub to_examine: Option<u64>,
    pub examined: Option<u64>,
    pub scan_errors: Option<u64>,
}

/// Dataset properties returned from libzetta
/// Unified structure for filesystem, volume, and snapshot properties
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetProperties {
    pub name: String,
    pub dataset_type: String,
    // Common properties
    pub available: Option<i64>,
    pub used: Option<u64>,
    pub referenced: Option<u64>,
    pub compression: Option<String>,
    pub compression_ratio: Option<f64>,
    pub readonly: Option<bool>,
    pub creation: Option<i64>,
    pub quota: Option<u64>,
    pub reservation: Option<u64>,
    pub ref_quota: Option<u64>,
    pub ref_reservation: Option<u64>,
    pub record_size: Option<u64>,
    pub checksum: Option<String>,
    pub copies: Option<u8>,
    pub mountpoint: Option<String>,
    pub mounted: Option<bool>,
    pub atime: Option<bool>,
    pub exec: Option<bool>,
    pub setuid: Option<bool>,
    pub devices: Option<bool>,
    pub xattr: Option<bool>,
    pub canmount: Option<String>,
    pub snapdir: Option<String>,
    pub sync: Option<String>,
    pub dedup: Option<String>,
    pub primary_cache: Option<String>,
    pub secondary_cache: Option<String>,
    // Volume-specific
    pub volume_size: Option<u64>,
    pub volume_block_size: Option<u64>,
    // User/unknown properties
    pub user_properties: HashMap<String, String>,
}

impl DatasetProperties {
    pub fn from_libzetta(name: String, props: libzetta::zfs::Properties) -> Self {
        use libzetta::zfs::Properties;

        match props {
            Properties::Filesystem(fs) => DatasetProperties {
                name,
                dataset_type: "filesystem".to_string(),
                available: Some(*fs.available()),
                used: Some(*fs.used()),
                referenced: Some(*fs.referenced()),
                compression: Some(format!("{}", fs.compression())),
                compression_ratio: Some(*fs.compression_ratio()),
                readonly: Some(*fs.readonly()),
                creation: Some(*fs.creation()),
                quota: Some(*fs.quota()),
                reservation: Some(*fs.reservation()),
                ref_quota: Some(*fs.ref_quota()),
                ref_reservation: Some(*fs.ref_reservation()),
                record_size: Some(*fs.record_size()),
                checksum: Some(format!("{}", fs.checksum())),
                copies: Some(*fs.copies() as u8),
                mountpoint: fs
                    .mount_point()
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                mounted: Some(*fs.mounted()),
                atime: Some(*fs.atime()),
                exec: Some(*fs.exec()),
                setuid: Some(*fs.setuid()),
                devices: Some(*fs.devices()),
                xattr: Some(*fs.xattr()),
                canmount: Some(format!("{}", fs.can_mount())),
                snapdir: Some(format!("{}", fs.snap_dir())),
                sync: Some(format!("{}", fs.sync())),
                dedup: Some(format!("{}", fs.dedup())),
                primary_cache: Some(format!("{}", fs.primary_cache())),
                secondary_cache: Some(format!("{}", fs.secondary_cache())),
                volume_size: None,
                volume_block_size: None,
                user_properties: fs.unknown_properties().clone(),
            },
            Properties::Volume(vol) => DatasetProperties {
                name,
                dataset_type: "volume".to_string(),
                available: Some(*vol.available()),
                used: Some(*vol.used()),
                referenced: Some(*vol.referenced()),
                compression: Some(format!("{}", vol.compression())),
                compression_ratio: Some(*vol.compression_ratio()),
                readonly: Some(*vol.readonly()),
                creation: Some(*vol.creation()),
                quota: None,
                reservation: Some(*vol.reservation()),
                ref_quota: None,
                ref_reservation: Some(*vol.ref_reservation()),
                record_size: None,
                checksum: Some(format!("{}", vol.checksum())),
                copies: Some(*vol.copies() as u8),
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: Some(format!("{}", vol.sync())),
                dedup: Some(format!("{}", vol.dedup())),
                primary_cache: Some(format!("{}", vol.primary_cache())),
                secondary_cache: Some(format!("{}", vol.secondary_cache())),
                volume_size: Some(*vol.volume_size()),
                volume_block_size: Some(*vol.volume_block_size()),
                user_properties: vol.unknown_properties().clone(),
            },
            Properties::Snapshot(snap) => DatasetProperties {
                name,
                dataset_type: "snapshot".to_string(),
                available: None,
                used: Some(*snap.used()),
                referenced: Some(*snap.referenced()),
                compression: None,
                compression_ratio: Some(*snap.compression_ratio()),
                readonly: None,
                creation: Some(*snap.creation()),
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: Some(*snap.exec()),
                setuid: Some(*snap.setuid()),
                devices: Some(*snap.devices()),
                xattr: Some(*snap.xattr()),
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: Some(format!("{}", snap.primary_cache())),
                secondary_cache: Some(format!("{}", snap.secondary_cache())),
                volume_size: None,
                volume_block_size: None,
                user_properties: snap.unknown_properties().clone(),
            },
            Properties::Bookmark(bm) => DatasetProperties {
                name,
                dataset_type: "bookmark".to_string(),
                available: None,
                used: None,
                referenced: None,
                compression: None,
                compression_ratio: None,
                readonly: None,
                creation: Some(*bm.creation()),
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: None,
                secondary_cache: None,
                volume_size: None,
                volume_block_size: None,
                user_properties: bm.unknown_properties().clone(),
            },
            Properties::Unknown(props) => DatasetProperties {
                name,
                dataset_type: "unknown".to_string(),
                available: None,
                used: None,
                referenced: None,
                compression: None,
                compression_ratio: None,
                readonly: None,
                creation: None,
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: None,
                secondary_cache: None,
                volume_size: None,
                volume_block_size: None,
                user_properties: props,
            },
        }
    }
}
