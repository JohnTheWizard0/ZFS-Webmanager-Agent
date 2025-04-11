//-----------------------------------------------------
// DATA MODELS
//-----------------------------------------------------

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::collections::HashMap;

// Add these data structures for the Linux command API
#[derive(Deserialize)]
pub struct CommandRequest {
    command: String,
    args: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct CommandResponse {
    status: String,
    output: String,
    exit_code: Option<i32>,
}

// Define a struct to track the last action
#[derive(Clone, Serialize)]
pub struct LastAction {
    function: String,
    timestamp: u64,
}

// Define the health response struct
#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
    last_action: Option<LastAction>,
}

// Response structures
#[derive(Serialize)]
pub struct ListResponse {
    snapshots: Vec<String>,
    status: String,
}

#[derive(Serialize)]
pub struct ActionResponse {
    status: String,
    message: String,
}

#[derive(Deserialize)]
pub struct CreateSnapshot {
    snapshot_name: String,
}

// Dataset structures
#[derive(Deserialize)]
pub struct CreateDataset {
    name: String,
    kind: String,  // "filesystem" or "volume"
    properties: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct DatasetResponse {
    datasets: Vec<String>,
    status: String,
}

// Pool status response
#[derive(Serialize)]
pub struct PoolStatus {
    name: String,
    health: String,
    size: u64,
    allocated: u64,
    free: i64,         // Changed from u64 to i64
    capacity: u8,       // Changed from u64 to u8
    vdevs: u32,
    errors: Option<String>,
}

// Pool list response
#[derive(Serialize)]
pub struct PoolListResponse {
    pools: Vec<String>,
    status: String,
}

// Pool creation request
#[derive(Deserialize)]
pub struct CreatePool {
    name: String,
    disks: Vec<String>,
    raid_type: Option<String>, // "mirror", "raidz", "raidz2", "raidz3", or null for individual disks
}