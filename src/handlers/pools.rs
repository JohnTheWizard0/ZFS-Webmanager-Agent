// handlers/pools.rs
// Pool handlers: list, status, create, destroy, export, import

use crate::models::{
    ActionResponse, ClearPoolRequest, ClearPoolResponse, CreatePool, ExportPoolRequest,
    ImportPoolRequest, ImportablePoolInfo, ImportablePoolsResponse, PoolListResponse,
    PoolStatusResponse,
};
use crate::utils::{error_response, success_response};
use crate::zfs_management::ZfsManager;
use warp::{Rejection, Reply};

pub async fn list_pools_handler(zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_pools().await {
        Ok(pools) => Ok(success_response(PoolListResponse {
            status: "success".to_string(),
            pools,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list pools: {}", e))),
    }
}

pub async fn get_pool_status_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_pool_status(&name).await {
        Ok(status) => Ok(success_response(PoolStatusResponse {
            status: "success".to_string(),
            name: status.name,
            health: status.health,
            size: status.size,
            allocated: status.allocated,
            free: status.free,
            capacity: status.capacity,
            vdevs: status.vdevs,
            errors: status.errors,
        })),
        Err(e) => Ok(error_response(&format!("Failed to get pool status: {}", e))),
    }
}

pub async fn create_pool_handler(
    body: CreatePool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_pool(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Pool created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create pool: {}", e))),
    }
}

pub async fn destroy_pool_handler(
    name: String,
    force: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.destroy_pool(&name, force).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' destroyed successfully", name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to destroy pool: {}", e))),
    }
}

// =========================================================================
// Import/Export Handlers
// =========================================================================

/// Export a pool from the system
pub async fn export_pool_handler(
    pool: String,
    body: ExportPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.export_pool(&pool, body.force).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' exported successfully", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to export pool: {}", e))),
    }
}

/// List pools available for import
pub async fn list_importable_pools_handler(
    dir: Option<String>,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = match dir {
        Some(d) => zfs.list_importable_pools_from_dir(&d).await,
        None => zfs.list_importable_pools().await,
    };

    match result {
        Ok(pools) => Ok(success_response(ImportablePoolsResponse {
            status: "success".to_string(),
            pools: pools
                .into_iter()
                .map(|p| ImportablePoolInfo {
                    name: p.name,
                    health: p.health,
                })
                .collect(),
        })),
        Err(e) => Ok(error_response(&format!(
            "Failed to list importable pools: {}",
            e
        ))),
    }
}

/// Import a pool into the system
/// Supports renaming on import via new_name field
pub async fn import_pool_handler(
    body: ImportPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = match (&body.new_name, &body.dir) {
        (Some(new_name), Some(dir)) => {
            zfs.import_pool_with_name(&body.name, new_name, Some(dir.as_str()))
                .await
        }
        (Some(new_name), None) => zfs.import_pool_with_name(&body.name, new_name, None).await,
        (None, Some(dir)) => zfs.import_pool_from_dir(&body.name, dir).await,
        (None, None) => zfs.import_pool(&body.name).await,
    };

    let imported_name = body.new_name.as_ref().unwrap_or(&body.name);

    match result {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' imported successfully", imported_name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to import pool: {}", e))),
    }
}

// =========================================================================
// Pool Clear Handler (FFI)
// =========================================================================

/// Clear error counters on a pool or specific device
/// POST /v1/pools/{name}/clear
pub async fn clear_pool_handler(
    pool: String,
    body: ClearPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let device_ref = body.device.as_deref();

    match zfs.clear_pool(&pool, device_ref).await {
        Ok(_) => {
            let message = match &body.device {
                Some(dev) => format!("Error counters cleared for device '{}' in pool '{}'", dev, pool),
                None => format!("Error counters cleared for pool '{}'", pool),
            };
            Ok(success_response(ClearPoolResponse {
                status: "success".to_string(),
                pool,
                device: body.device,
                message,
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to clear pool errors: {}", e))),
    }
}
