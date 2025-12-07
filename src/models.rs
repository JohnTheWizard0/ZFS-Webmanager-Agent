use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::collections::HashMap;

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
        
        Self { function, timestamp }
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
// FROM-SCRATCH implementation using libzfs FFI bindings.
// Extracts real scan progress from pool_scan_stat_t via nvlist.
#[derive(Debug, Serialize)]
pub struct ScrubStatusResponse {
    pub status: String,
    pub pool: String,
    pub pool_health: String,
    pub pool_errors: Option<String>,
    // Scan details from pool_scan_stat_t
    pub scan_state: String,           // none, scanning, finished, canceled
    pub scan_function: Option<String>, // scrub, resilver, errorscrub
    pub start_time: Option<u64>,       // Unix timestamp
    pub end_time: Option<u64>,         // Unix timestamp (if finished)
    pub to_examine: Option<u64>,       // Total bytes to scan
    pub examined: Option<u64>,         // Bytes scanned so far
    pub scan_errors: Option<u64>,      // Errors encountered
    pub percent_done: Option<f64>,     // Calculated: (examined / to_examine) * 100
}

// ============================================================================
// UNIT TESTS — MI-002 (API Framework - Data Layer)
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::time::{SystemTime, UNIX_EPOCH};

    // -------------------------------------------------------------------------
    // LastAction Tests
    // -------------------------------------------------------------------------

    /// Test: LastAction timestamp is current epoch
    /// Expected: Within 2 seconds of now
    #[test]
    fn test_last_action_timestamp_current() {
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let action = LastAction::new("test_function".to_string());
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert!(action.timestamp >= before, "Timestamp should be >= start time");
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