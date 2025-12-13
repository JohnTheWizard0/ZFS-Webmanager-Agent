// handlers/vdev.rs
// Vdev handlers: add, remove

use crate::models::{AddVdevRequest, AddVdevResponse, RemoveVdevResponse};
use crate::utils::{error_response, success_response};
use crate::zfs_management::ZfsManager;
use warp::{Rejection, Reply};

/// Add a vdev to an existing pool
/// POST /v1/pools/{name}/vdev
pub async fn add_vdev_handler(
    pool: String,
    body: AddVdevRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .add_vdev(
            &pool,
            &body.vdev_type,
            body.devices.clone(),
            body.force,
            body.check_ashift,
        )
        .await
    {
        Ok(_) => Ok(success_response(AddVdevResponse {
            status: "success".to_string(),
            pool: pool.clone(),
            vdev_type: body.vdev_type,
            devices: body.devices,
            message: format!("Vdev added to pool '{}' successfully", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to add vdev: {}", e))),
    }
}

/// Remove a vdev from an existing pool
/// DELETE /v1/pools/{name}/vdev/{device}
pub async fn remove_vdev_handler(
    pool: String,
    device: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.remove_vdev(&pool, &device).await {
        Ok(_) => Ok(success_response(RemoveVdevResponse {
            status: "success".to_string(),
            pool: pool.clone(),
            device: device.clone(),
            message: format!(
                "Device '{}' removed from pool '{}' successfully",
                device, pool
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to remove vdev: {}", e))),
    }
}
