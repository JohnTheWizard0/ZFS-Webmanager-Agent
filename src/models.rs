use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastAction {
    pub function: String,
    pub timestamp: u64,
}

impl LastAction {
    pub fn new(function: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            function,
            timestamp,
        }
    }
}

// Response structures
#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PoolListResponse {
    pub status: String,
    pub pools: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub status: String,
    pub items: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DatasetResponse {
    pub status: String,
    pub datasets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub last_action: Option<LastAction>,
}

#[derive(Debug, Serialize)]
pub struct PoolStatusResponse {
    pub status: String,
    pub name: String,
    pub health: String,
    pub size: u64,
    pub allocated: u64,
    pub free: u64,
    pub capacity: u8,
    pub vdevs: u32,
    pub errors: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub status: String,
    pub output: String,
    pub exit_code: i32,
}

// Request structures
#[derive(Debug, Deserialize)]
pub struct CreatePool {
    pub name: String,
    pub disks: Vec<String>,
    pub raid_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSnapshot {
    pub snapshot_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDataset {
    pub name: String,
    pub kind: String,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    pub args: Option<Vec<String>>,
}

// Scrub status response
// Implementation via libzfs FFI bindings.
// Extracts real scan progress from pool_scan_stat_t via nvlist.
#[derive(Debug, Serialize)]
pub struct ScrubStatusResponse {
    pub status: String,
    pub pool: String,
    pub pool_health: String,
    pub pool_errors: Option<String>,
    // Scan details from pool_scan_stat_t
    pub scan_state: String,            // none, scanning, finished, canceled
    pub scan_function: Option<String>, // scrub, resilver, errorscrub
    pub start_time: Option<u64>,       // Unix timestamp
    pub end_time: Option<u64>,         // Unix timestamp (if finished)
    pub to_examine: Option<u64>,       // Total bytes to scan
    pub examined: Option<u64>,         // Bytes scanned so far
    pub scan_errors: Option<u64>,      // Errors encountered
    pub percent_done: Option<f64>,     // Calculated: (examined / to_examine) * 100
}

// Import/Export request structures
#[derive(Debug, Deserialize)]
pub struct ExportPoolRequest {
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize)]
pub struct ImportPoolRequest {
    pub name: String,
    pub dir: Option<String>,      // Optional: directory to search for pool
    pub new_name: Option<String>, // Optional: rename pool on import (CLI-based)
}

// Import/Export response structures
#[derive(Debug, Serialize)]
pub struct ImportablePoolInfo {
    pub name: String,
    pub health: String,
}

#[derive(Debug, Serialize)]
pub struct ImportablePoolsResponse {
    pub status: String,
    pub pools: Vec<ImportablePoolInfo>,
}

// Dataset properties response
#[derive(Debug, Serialize)]
pub struct DatasetPropertiesResponse {
    pub status: String,
    #[serde(flatten)]
    pub properties: crate::zfs_management::DatasetProperties,
}

// Dataset property set request
// **EXPERIMENTAL**: Uses CLI as FFI lacks property setting
#[derive(Debug, Deserialize)]
pub struct SetPropertyRequest {
    pub property: String,
    pub value: String,
}

// Clone snapshot request
#[derive(Debug, Deserialize)]
pub struct CloneSnapshotRequest {
    pub target: String, // Target clone path (e.g., "tank/data-clone")
}

// Clone response
#[derive(Debug, Serialize)]
pub struct CloneResponse {
    pub status: String,
    pub origin: String, // Source snapshot
    pub clone: String,  // New clone path
}

// Promote response
#[derive(Debug, Serialize)]
pub struct PromoteResponse {
    pub status: String,
    pub dataset: String, // Promoted dataset path
    pub message: String,
}

// Rollback request
#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub snapshot: String, // Target snapshot name (without @)
    #[serde(default)]
    pub force_destroy_newer: bool,
    #[serde(default)]
    pub force_destroy_clones: bool,
}

// Rollback response
#[derive(Debug, Serialize)]
pub struct RollbackResponse {
    pub status: String,
    pub dataset: String,
    pub snapshot: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destroyed_snapshots: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destroyed_clones: Option<Vec<String>>,
}

// Rollback blocked response
#[derive(Debug, Serialize)]
pub struct RollbackBlockedResponse {
    pub status: String,
    pub message: String,
    pub blocking_snapshots: Vec<String>,
    pub blocking_clones: Vec<String>,
}

// ============================================================================
// ZFS Features Discovery
// ============================================================================

/// Implementation method for a ZFS feature
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationMethod {
    /// Uses libzetta library bindings
    Libzetta,
    /// Direct FFI implementation (lzc_* functions via libzetta-zfs-core-sys)
    Ffi,
    /// Uses libzfs FFI bindings directly
    Libzfs,
    /// EXPERIMENTAL: Falls back to CLI (zfs/zpool commands)
    CliExperimental,
    /// Hybrid: libzetta send + CLI receive (lzc_receive too low-level)
    Hybrid,
    /// Not yet implemented
    Planned,
}

/// Feature category
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureCategory {
    Pool,
    Dataset,
    Snapshot,
    Property,
    Replication,
    System,
}

/// Individual ZFS feature info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsFeatureInfo {
    pub name: String,
    pub category: FeatureCategory,
    pub implemented: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub implementation: Option<ImplementationMethod>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub notes: Option<String>,
}

/// JSON file structure for features
#[derive(Debug, Deserialize)]
pub struct FeaturesFile {
    pub version: String,
    pub features: Vec<ZfsFeatureInfo>,
}

/// Summary counts by category
#[derive(Debug, Clone, Serialize)]
pub struct FeatureSummary {
    pub total: u32,
    pub implemented: u32,
    pub planned: u32,
}

/// ZFS features response
#[derive(Debug, Serialize)]
pub struct ZfsFeaturesResponse {
    pub status: String,
    pub version: String,
    pub summary: FeatureSummary,
    pub features: Vec<ZfsFeatureInfo>,
}

/// Embedded default features.json (fallback if file not found)
const DEFAULT_FEATURES_JSON: &str = include_str!("../features.json");

impl ZfsFeaturesResponse {
    /// Load features from features.json file (or embedded default)
    fn load_features() -> Vec<ZfsFeatureInfo> {
        // Try to load from file first (allows runtime updates without recompile)
        let json_content = std::fs::read_to_string("features.json")
            .or_else(|_| std::fs::read_to_string("/etc/zfs-agent/features.json"))
            .unwrap_or_else(|_| DEFAULT_FEATURES_JSON.to_string());

        match serde_json::from_str::<FeaturesFile>(&json_content) {
            Ok(file) => file.features,
            Err(e) => {
                eprintln!("Warning: Failed to parse features.json: {}", e);
                // Return empty on parse error (shouldn't happen with embedded default)
                vec![]
            }
        }
    }

    /// Build the features response with all known features
    pub fn build() -> Self {
        let features = Self::load_features();

        let implemented = features.iter().filter(|f| f.implemented).count() as u32;
        let planned = features.iter().filter(|f| !f.implemented).count() as u32;

        ZfsFeaturesResponse {
            status: "success".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            summary: FeatureSummary {
                total: features.len() as u32,
                implemented,
                planned,
            },
            features,
        }
    }
}

// ============================================================================
// Replication / Task System
// ============================================================================

/// Task status for async operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Task operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOperation {
    Send,
    Receive,
    Replicate,
}

/// Progress information for running tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub bytes_processed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f32>,
}

/// Complete task state
#[derive(Debug, Clone, Serialize)]
pub struct TaskState {
    pub task_id: String,
    pub status: TaskStatus,
    pub operation: TaskOperation,
    pub pools_involved: Vec<String>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Task response returned to client
#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub status: String,
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Task status response (for GET /tasks/{id})
#[derive(Debug, Serialize)]
pub struct TaskStatusResponse {
    pub status: String, // "pending", "running", "completed", "failed"
    pub task_id: String,
    pub operation: TaskOperation,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&TaskState> for TaskStatusResponse {
    fn from(state: &TaskState) -> Self {
        TaskStatusResponse {
            status: match state.status {
                TaskStatus::Pending => "pending".to_string(),
                TaskStatus::Running => "running".to_string(),
                TaskStatus::Completed => "completed".to_string(),
                TaskStatus::Failed => "failed".to_string(),
            },
            task_id: state.task_id.clone(),
            operation: state.operation.clone(),
            started_at: state.started_at,
            completed_at: state.completed_at,
            progress: state.progress.clone(),
            result: state.result.clone(),
            error: state.error.clone(),
        }
    }
}

// ============================================================================
// Replication Request Types
// ============================================================================

/// Request to send snapshot to file
#[derive(Debug, Deserialize)]
pub struct SendSnapshotRequest {
    pub output_file: String,
    #[serde(default)]
    pub from_snapshot: Option<String>, // incremental base
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub properties: bool,
    #[serde(default)]
    pub raw: bool,
    #[serde(default)]
    pub compressed: bool,
    #[serde(default)]
    pub large_blocks: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub overwrite: bool,
}

/// Request to receive from file
#[derive(Debug, Deserialize)]
pub struct ReceiveSnapshotRequest {
    pub input_file: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// Request to replicate snapshot to another pool
#[derive(Debug, Deserialize)]
pub struct ReplicateSnapshotRequest {
    pub target_dataset: String,
    #[serde(default)]
    pub from_snapshot: Option<String>, // incremental base
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub properties: bool,
    #[serde(default)]
    pub raw: bool,
    #[serde(default)]
    pub compressed: bool,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// Query params for dataset deletion
#[derive(Debug, Deserialize)]
pub struct DeleteDatasetQuery {
    #[serde(default)]
    pub recursive: bool, // -r flag for recursive delete (children + snapshots)
}

/// Query params for send size estimation
#[derive(Debug, Deserialize)]
pub struct SendSizeQuery {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub raw: bool,
}

// ============================================================================
// Pool Vdev Operations
// ============================================================================

/// Helper for serde default true
fn default_true() -> bool {
    true
}

/// Request to add a vdev to an existing pool
/// POST /v1/pools/{name}/vdev
///
/// Supports adding:
/// - Data vdevs: disk, mirror, raidz, raidz2, raidz3
/// - Special vdevs: log, cache, spare, special, dedup
///
/// # Examples
/// ```json
/// // Add a mirror vdev
/// {"vdev_type": "mirror", "devices": ["/dev/sdc", "/dev/sdd"]}
///
/// // Add a single disk
/// {"vdev_type": "disk", "devices": ["/dev/sde"]}
///
/// // Add a raidz2 vdev
/// {"vdev_type": "raidz2", "devices": ["/dev/sdf", "/dev/sdg", "/dev/sdh", "/dev/sdi"]}
///
/// // Add a SLOG device
/// {"vdev_type": "log", "devices": ["/dev/nvme0n1"]}
///
/// // Add a cache device (L2ARC)
/// {"vdev_type": "cache", "devices": ["/dev/nvme1n1"]}
/// ```
#[derive(Debug, Deserialize)]
pub struct AddVdevRequest {
    /// Type of vdev to add
    /// Data vdevs: "disk", "mirror", "raidz", "raidz2", "raidz3"
    /// Special vdevs: "log", "cache", "spare", "special", "dedup"
    pub vdev_type: String,

    /// Device paths (e.g., ["/dev/sdc", "/dev/sdd"])
    pub devices: Vec<String>,

    /// Force add even if devices appear in use (-f flag)
    #[serde(default)]
    pub force: bool,

    /// Check and warn on ashift mismatch (default: true)
    /// Mismatched ashift can prevent future vdev removal
    #[serde(default = "default_true")]
    pub check_ashift: bool,
}

/// Response after successfully adding a vdev
#[derive(Debug, Serialize)]
pub struct AddVdevResponse {
    pub status: String,
    pub pool: String,
    pub vdev_type: String,
    pub devices: Vec<String>,
    pub message: String,
}

/// Response after successfully removing a vdev
#[derive(Debug, Serialize)]
pub struct RemoveVdevResponse {
    pub status: String,
    pub pool: String,
    pub device: String,
    pub message: String,
}

/// Response for send size estimation
#[derive(Debug, Serialize)]
pub struct SendSizeResponse {
    pub status: String,
    pub snapshot: String,
    pub estimated_bytes: u64,
    pub estimated_human: String,
    pub incremental: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_snapshot: Option<String>,
}

// ============================================================================
// Safety Lock System
// ============================================================================

/// Detected ZFS version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsVersionInfo {
    /// Full version string (e.g., "2.1.5-1ubuntu6~22.04.1")
    pub full_version: String,
    /// Parsed semantic version (e.g., "2.1.5")
    pub semantic_version: String,
    /// Major version number
    pub major: u32,
    /// Minor version number
    pub minor: u32,
    /// Patch version number (if available)
    pub patch: Option<u32>,
    /// Detection method used
    pub detection_method: String,
}

/// Safety lock state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyState {
    /// Whether safety lock is currently active (blocking mutations)
    pub locked: bool,
    /// ZFS version detected at startup
    pub zfs_version: ZfsVersionInfo,
    /// Agent version (from Cargo.toml)
    pub agent_version: String,
    /// List of approved ZFS versions
    pub approved_versions: Vec<String>,
    /// Whether the detected version is compatible
    pub compatible: bool,
    /// Reason for lock (if locked)
    pub lock_reason: Option<String>,
    /// Timestamp when lock was overridden (if applicable)
    pub override_at: Option<u64>,
}

/// GET /v1/safety response
#[derive(Debug, Serialize)]
pub struct SafetyStatusResponse {
    pub status: String,
    pub locked: bool,
    pub compatible: bool,
    pub zfs_version: ZfsVersionInfo,
    pub agent_version: String,
    pub approved_versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_at: Option<u64>,
}

/// POST /v1/safety request
#[derive(Debug, Deserialize)]
pub struct SafetyOverrideRequest {
    pub action: String, // Currently only "override" is supported
}

/// POST /v1/safety response
#[derive(Debug, Serialize)]
pub struct SafetyOverrideResponse {
    pub status: String,
    pub message: String,
    pub locked: bool,
}

// ============================================================================
// UNIT TESTS
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    // -------------------------------------------------------------------------
    // LastAction Tests
    // -------------------------------------------------------------------------

    /// Test: LastAction timestamp is current epoch
    /// Expected: Within 2 seconds of now
    #[test]
    fn test_last_action_timestamp_current() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let action = LastAction::new("test_function".to_string());
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert!(
            action.timestamp >= before,
            "Timestamp should be >= start time"
        );
        assert!(action.timestamp <= after, "Timestamp should be <= end time");
    }

    /// Test: LastAction stores function name
    /// Expected: function field matches input
    #[test]
    fn test_last_action_function_name() {
        let action = LastAction::new("list_pools".to_string());
        assert_eq!(action.function, "list_pools");
    }

    /// Test: LastAction serializes to JSON
    /// Expected: Valid JSON with function and timestamp fields
    #[test]
    fn test_last_action_serialization() {
        let action = LastAction::new("create_snapshot".to_string());
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"function\":\"create_snapshot\""));
        assert!(json.contains("\"timestamp\":"));
    }

    // -------------------------------------------------------------------------
    // Request Deserialization Tests
    // -------------------------------------------------------------------------

    /// Test: CreatePool - minimal valid payload
    /// Expected: name and disks required, raid_type optional
    #[test]
    fn test_create_pool_minimal() {
        let json = r#"{"name": "tank", "disks": ["/dev/sda"]}"#;
        let pool: CreatePool = serde_json::from_str(json).unwrap();
        assert_eq!(pool.name, "tank");
        assert_eq!(pool.disks, vec!["/dev/sda"]);
        assert!(pool.raid_type.is_none());
    }

    /// Test: CreatePool - with raid_type
    /// Expected: raid_type captured correctly
    #[test]
    fn test_create_pool_with_raid() {
        let json = r#"{"name": "tank", "disks": ["/dev/sda", "/dev/sdb"], "raid_type": "mirror"}"#;
        let pool: CreatePool = serde_json::from_str(json).unwrap();
        assert_eq!(pool.raid_type, Some("mirror".to_string()));
    }

    /// Test: CreatePool - missing required field fails
    /// Expected: Deserialization error
    #[test]
    fn test_create_pool_missing_disks() {
        let json = r#"{"name": "tank"}"#;
        let result: Result<CreatePool, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Missing 'disks' should fail");
    }

    /// Test: CreateDataset - minimal valid payload
    /// Expected: name and kind required, properties optional
    #[test]
    fn test_create_dataset_minimal() {
        let json = r#"{"name": "tank/data", "kind": "filesystem"}"#;
        let ds: CreateDataset = serde_json::from_str(json).unwrap();
        assert_eq!(ds.name, "tank/data");
        assert_eq!(ds.kind, "filesystem");
        assert!(ds.properties.is_none());
    }

    /// Test: CreateDataset - with properties
    /// Expected: properties HashMap populated
    #[test]
    fn test_create_dataset_with_properties() {
        let json = r#"{"name": "tank/data", "kind": "filesystem", "properties": {"compression": "lz4", "quota": "10G"}}"#;
        let ds: CreateDataset = serde_json::from_str(json).unwrap();
        let props = ds.properties.unwrap();
        assert_eq!(props.get("compression"), Some(&"lz4".to_string()));
        assert_eq!(props.get("quota"), Some(&"10G".to_string()));
    }

    /// Test: CreateSnapshot - valid payload
    /// Expected: snapshot_name captured
    #[test]
    fn test_create_snapshot() {
        let json = r#"{"snapshot_name": "backup-2025-01-01"}"#;
        let snap: CreateSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snap.snapshot_name, "backup-2025-01-01");
    }

    /// Test: CommandRequest - minimal valid payload
    /// Expected: command required, args optional
    #[test]
    fn test_command_request_minimal() {
        let json = r#"{"command": "zpool"}"#;
        let cmd: CommandRequest = serde_json::from_str(json).unwrap();
        assert_eq!(cmd.command, "zpool");
        assert!(cmd.args.is_none());
    }

    /// Test: CommandRequest - with args
    /// Expected: args Vec populated
    #[test]
    fn test_command_request_with_args() {
        let json = r#"{"command": "zpool", "args": ["status", "-v"]}"#;
        let cmd: CommandRequest = serde_json::from_str(json).unwrap();
        assert_eq!(cmd.args, Some(vec!["status".to_string(), "-v".to_string()]));
    }

    // -------------------------------------------------------------------------
    // Response Serialization Tests
    // -------------------------------------------------------------------------

    /// Test: ActionResponse serializes correctly
    /// Expected: JSON with status and message
    #[test]
    fn test_action_response_serialization() {
        let resp = ActionResponse {
            status: "success".to_string(),
            message: "Pool created".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"message\":\"Pool created\""));
    }

    /// Test: HealthResponse serializes with optional last_action
    /// Expected: last_action can be null or object
    #[test]
    fn test_health_response_serialization() {
        let resp_none = HealthResponse {
            status: "success".to_string(),
            version: "0.3.2".to_string(),
            last_action: None,
        };
        let json = serde_json::to_string(&resp_none).unwrap();
        assert!(json.contains("\"last_action\":null"));

        let resp_some = HealthResponse {
            status: "success".to_string(),
            version: "0.3.2".to_string(),
            last_action: Some(LastAction::new("test".to_string())),
        };
        let json = serde_json::to_string(&resp_some).unwrap();
        assert!(json.contains("\"last_action\":{"));
    }

    /// Test: PoolStatusResponse serializes all fields
    /// Expected: All 9 fields present in JSON
    #[test]
    fn test_pool_status_response_serialization() {
        let resp = PoolStatusResponse {
            status: "success".to_string(),
            name: "tank".to_string(),
            health: "Online".to_string(),
            size: 1099511627776,
            allocated: 549755813888,
            free: 549755813888,
            capacity: 50,
            vdevs: 2,
            errors: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"name\":\"tank\""));
        assert!(json.contains("\"health\":\"Online\""));
        assert!(json.contains("\"capacity\":50"));
    }
}
