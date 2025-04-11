//-----------------------------------------------------
// POOL HANDLERS
//-----------------------------------------------------

use warp::{Reply, Rejection, Filter};
use crate::zfs_management::ZfsManager;
use crate::models::*;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// List all pools
pub async fn list_pools_handler(
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_pools().await {
        Ok(pools) => Ok(warp::reply::json(&PoolListResponse {
            pools,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Get pool status
pub async fn get_pool_status_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_pool_status(&name).await {
        Ok(status) => Ok(warp::reply::json(&status)),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Create a new pool
pub async fn create_pool_handler(
    body: CreatePool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_pool(body).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Pool created successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Destroy a pool
pub async fn destroy_pool_handler(
    name: String,
    force: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.destroy_pool(&name, force).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Pool destroyed successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

//-----------------------------------------------------
// SNAPSHOT HANDLERS
//-----------------------------------------------------

pub async fn list_snapshots_handler(
    dataset: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_snapshots(&dataset).await {
        Ok(snapshots) => Ok(warp::reply::json(&ListResponse {
            snapshots,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

pub async fn create_snapshot_handler(
    dataset: String,
    body: CreateSnapshot,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_snapshot(&dataset, &body.snapshot_name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Snapshot created successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

pub async fn delete_snapshot_handler(
    dataset: String,
    snapshot_name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_snapshot(&dataset, &snapshot_name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Snapshot deleted successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

//-----------------------------------------------------
// DATASET HANDLERS
//-----------------------------------------------------

pub async fn list_datasets_handler(
    pool: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(warp::reply::json(&DatasetResponse {
            datasets,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

pub async fn create_dataset_handler(
    body: CreateDataset,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_dataset(body).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Dataset created successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

pub async fn delete_dataset_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_dataset(&name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Dataset deleted successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

//-----------------------------------------------------
// MISC HANDLERS
//-----------------------------------------------------

// Health check handler
pub async fn health_check_handler(
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let last_action_data = if let Ok(action) = last_action.read() {
        action.clone()
    } else {
        None
    };
    
    Ok(warp::reply::json(&HealthResponse {
        status: "ok".to_string(),
        version,
        last_action: last_action_data,
    }))
}

// Add this handler function for the API endpoint
pub async fn execute_command_handler(
    body: CommandRequest,
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    // Convert Vec<String> to Vec<&str> for the arguments
    let args: Vec<&str> = match &body.args {
        Some(arg_vec) => arg_vec.iter().map(|s| s.as_str()).collect(),
        None => Vec::new(),
    };
    
    // Execute the command
    match execute_linux_command(&body.command, &args) {
        Ok((output, exit_code)) => {
            // Update the last action tracker
            if let Ok(mut last_action) = last_action.write() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                *last_action = Some(LastAction {
                    function: format!("execute_command: {}", body.command),
                    timestamp: now,
                });
            }
            
            Ok(warp::reply::json(&CommandResponse {
                status: if exit_code.unwrap_or(1) == 0 { "success".to_string() } else { "error".to_string() },
                output,
                exit_code,
            }))
        },
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: format!("Failed to execute command: {}", e),
        })),
    }
}