use crate::models::*;
use crate::utils::{success_response, error_response};
use crate::zfs_management::ZfsManager;
use warp::{Rejection, Reply};
use std::sync::{Arc, RwLock};
use std::process::Command;

// Embed OpenAPI spec at compile time
const OPENAPI_SPEC: &str = include_str!("../openapi.yaml");

/// Serve OpenAPI spec as YAML
pub async fn openapi_handler() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_header(
        OPENAPI_SPEC,
        "Content-Type",
        "application/yaml",
    ))
}

/// Serve Swagger UI HTML page
pub async fn docs_handler() -> Result<impl Reply, Rejection> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>ZFS Web Manager API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        html { box-sizing: border-box; overflow: -moz-scrollbars-vertical; overflow-y: scroll; }
        *, *:before, *:after { box-sizing: inherit; }
        body { margin: 0; background: #fafafa; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" charset="UTF-8"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: "/v1/openapi.yaml",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis
                ]
            });
        };
    </script>
</body>
</html>"#;

    Ok(warp::reply::with_header(
        html,
        "Content-Type",
        "text/html; charset=utf-8",
    ))
}

pub async fn health_check_handler(
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    let last_action_data = last_action.read().unwrap().clone();
    
    let response = HealthResponse {
        status: "success".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        last_action: last_action_data,
    };
    
    Ok(warp::reply::json(&response))
}

pub async fn list_pools_handler(zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_pools().await {
        Ok(pools) => Ok(success_response(PoolListResponse {
            status: "success".to_string(),
            pools,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list pools: {}", e))),
    }
}

pub async fn get_pool_status_handler(name: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
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

pub async fn create_pool_handler(body: CreatePool, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.create_pool(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Pool created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create pool: {}", e))),
    }
}

pub async fn destroy_pool_handler(name: String, force: bool, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.destroy_pool(&name, force).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' destroyed successfully", name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to destroy pool: {}", e))),
    }
}

pub async fn list_snapshots_handler(dataset: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
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
    match zfs.create_snapshot(&dataset, &body.snapshot_name).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Snapshot '{}@{}' created successfully", dataset, body.snapshot_name),
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
        let snapshot_name = path[pos+1..].to_string();
        match zfs.delete_snapshot(&dataset, &snapshot_name).await {
            Ok(_) => Ok(success_response(ActionResponse {
                status: "success".to_string(),
                message: format!("Snapshot '{}@{}' deleted successfully", dataset, snapshot_name),
            })),
            Err(e) => Ok(error_response(&format!("Failed to delete snapshot: {}", e))),
        }
    } else {
        Ok(error_response("Invalid snapshot path: expected /snapshots/dataset/snapshot_name"))
    }
}

pub async fn list_datasets_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(success_response(DatasetResponse {
            status: "success".to_string(),
            datasets,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list datasets: {}", e))),
    }
}

pub async fn create_dataset_handler(body: CreateDataset, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.create_dataset(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Dataset created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create dataset: {}", e))),
    }
}

pub async fn delete_dataset_handler(name: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.delete_dataset(&name).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Dataset '{}' deleted successfully", name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to delete dataset: {}", e))),
    }
}

// =========================================================================
// Scrub Handlers (MF-001 Phase 2)
// =========================================================================

/// Start a scrub on the pool
pub async fn start_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.start_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub started on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to start scrub: {}", e))),
    }
}

/// Pause a scrub on the pool
pub async fn pause_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.pause_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub paused on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to pause scrub: {}", e))),
    }
}

/// Stop a scrub on the pool
pub async fn stop_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.stop_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub stopped on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to stop scrub: {}", e))),
    }
}

/// Get scrub status for the pool
/// FROM-SCRATCH implementation using libzfs FFI bindings.
/// Returns actual scan progress extracted from pool_scan_stat_t.
pub async fn get_scrub_status_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.get_scrub_status(&pool).await {
        Ok(scrub) => {
            // Calculate percent_done: (examined / to_examine) * 100
            // Per ZFS docs: check state == finished rather than percent == 100
            let percent_done = match (scrub.examined, scrub.to_examine) {
                (Some(examined), Some(to_examine)) if to_examine > 0 => {
                    Some((examined as f64 / to_examine as f64) * 100.0)
                }
                _ => None,
            };

            Ok(success_response(ScrubStatusResponse {
                status: "success".to_string(),
                pool: pool.clone(),
                pool_health: scrub.pool_health,
                pool_errors: scrub.errors,
                scan_state: scrub.state,
                scan_function: scrub.function,
                start_time: scrub.start_time,
                end_time: scrub.end_time,
                to_examine: scrub.to_examine,
                examined: scrub.examined,
                scan_errors: scrub.scan_errors,
                percent_done,
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to get scrub status: {}", e))),
    }
}

// =========================================================================
// Import/Export Handlers (MF-001 Phase 2)
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
            pools: pools.into_iter().map(|p| ImportablePoolInfo {
                name: p.name,
                health: p.health,
            }).collect(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to list importable pools: {}", e))),
    }
}

/// Import a pool into the system
pub async fn import_pool_handler(
    body: ImportPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = match body.dir {
        Some(ref d) => zfs.import_pool_from_dir(&body.name, d).await,
        None => zfs.import_pool(&body.name).await,
    };

    match result {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' imported successfully", body.name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to import pool: {}", e))),
    }
}

pub async fn execute_command_handler(
    body: CommandRequest,
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    // Update last action
    if let Ok(mut action) = last_action.write() {
        *action = Some(LastAction::new("execute_command".to_string()));
    }

    let mut cmd = Command::new(&body.command);
    
    if let Some(args) = body.args {
        cmd.args(args);
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined_output = format!("{}{}", stdout, stderr);
            
            Ok(success_response(CommandResponse {
                status: "success".to_string(),
                output: combined_output,
                exit_code: output.status.code().unwrap_or(-1),
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to execute command: {}", e))),
    }
}