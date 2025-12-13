// handlers/snapshots.rs
// Snapshot handlers: list, create, delete, clone, promote, rollback

use crate::models::{
    ActionResponse, CloneResponse, CloneSnapshotRequest, CreateSnapshot, ListResponse,
    PromoteResponse, RollbackBlockedResponse, RollbackRequest, RollbackResponse,
};
use crate::utils::{error_response, success_response, validate_snapshot_name};
use crate::zfs_management::{RollbackError, ZfsManager};
use warp::{Rejection, Reply};

pub async fn list_snapshots_handler(
    dataset: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_snapshots(&dataset).await {
        Ok(snapshots) => Ok(success_response(ListResponse {
            status: "success".to_string(),
            items: snapshots,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list snapshots: {}", e))),
    }
}

pub async fn create_snapshot_handler(
    dataset: String,
    body: CreateSnapshot,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Validate snapshot name before attempting creation
    if let Err(msg) = validate_snapshot_name(&body.snapshot_name) {
        return Ok(error_response(&format!("Invalid snapshot name: {}", msg)));
    }

    match zfs.create_snapshot(&dataset, &body.snapshot_name).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Snapshot '{}@{}' created successfully",
                dataset, body.snapshot_name
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create snapshot: {}", e))),
    }
}

/// Delete snapshot handler that parses path as "dataset/path/snapshot_name"
/// Last segment is the snapshot name, everything before is the dataset path
pub async fn delete_snapshot_by_path_handler(
    path: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    if let Some(pos) = path.rfind('/') {
        let dataset = path[..pos].to_string();
        let snapshot_name = path[pos + 1..].to_string();
        match zfs.delete_snapshot(&dataset, &snapshot_name).await {
            Ok(_) => Ok(success_response(ActionResponse {
                status: "success".to_string(),
                message: format!(
                    "Snapshot '{}@{}' deleted successfully",
                    dataset, snapshot_name
                ),
            })),
            Err(e) => Ok(error_response(&format!("Failed to delete snapshot: {}", e))),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name",
        ))
    }
}

// =========================================================================
// Snapshot Clone/Promote Handlers
// =========================================================================

/// Clone a snapshot to create a new writable dataset
pub async fn clone_snapshot_handler(
    snapshot_path: String, // Full path: dataset/snapshot_name
    body: CloneSnapshotRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        match zfs.clone_snapshot(&full_snapshot, &body.target).await {
            Ok(_) => Ok(success_response(CloneResponse {
                status: "success".to_string(),
                origin: full_snapshot,
                clone: body.target,
            })),
            Err(e) => Ok(error_response(&format!("Failed to clone snapshot: {}", e))),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name/clone",
        ))
    }
}

/// Promote a clone to an independent dataset
pub async fn promote_dataset_handler(
    clone_path: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.promote_dataset(&clone_path).await {
        Ok(_) => Ok(success_response(PromoteResponse {
            status: "success".to_string(),
            dataset: clone_path.clone(),
            message: format!(
                "Dataset '{}' promoted successfully. Former parent is now a clone.",
                clone_path
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to promote dataset: {}", e))),
    }
}

/// Rollback a dataset to a snapshot
pub async fn rollback_dataset_handler(
    dataset: String,
    body: RollbackRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .rollback_dataset(
            &dataset,
            &body.snapshot,
            body.force_destroy_newer,
            body.force_destroy_clones,
        )
        .await
    {
        Ok(result) => Ok(success_response(RollbackResponse {
            status: "success".to_string(),
            dataset: dataset.clone(),
            snapshot: body.snapshot,
            message: format!("Dataset '{}' rolled back successfully", dataset),
            destroyed_snapshots: result.destroyed_snapshots,
            destroyed_clones: result.destroyed_clones,
        })),
        Err(RollbackError::InvalidRequest(msg)) => {
            Ok(error_response(&format!("Invalid request: {}", msg)))
        }
        Err(RollbackError::Blocked {
            message,
            blocking_snapshots,
            blocking_clones,
        }) => {
            // Return structured blocked response with blocking items
            Ok(success_response(RollbackBlockedResponse {
                status: "error".to_string(),
                message,
                blocking_snapshots,
                blocking_clones,
            }))
        }
        Err(RollbackError::ZfsError(msg)) => {
            Ok(error_response(&format!("Rollback failed: {}", msg)))
        }
    }
}
