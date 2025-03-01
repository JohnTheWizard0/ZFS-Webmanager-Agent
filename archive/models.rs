use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Response structures
#[derive(Serialize)]
pub struct ListResponse {
    pub snapshots: Vec<String>,
    pub status: String,
}

#[derive(Serialize)]
pub struct ActionResponse {
    pub status: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct CreateSnapshot {
    pub snapshot_name: String,
}

// Dataset structures
#[derive(Deserialize)]
pub struct CreateDataset {
    pub name: String,
    pub kind: String,  // "filesystem" or "volume"
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct DatasetResponse {
    pub datasets: Vec<String>,
    pub status: String,
}

// Error helpers
pub fn success_response(message: &str) -> ActionResponse {
    ActionResponse {
        status: "success".to_string(),
        message: message.to_string(),
    }
}

pub fn error_response(error: &dyn std::error::Error) -> ActionResponse {
    ActionResponse {
        status: "error".to_string(),
        message: error.to_string(),
    }
}