// handlers/scrub.rs
// Scrub handlers: start, pause, stop, status

use crate::models::{ActionResponse, ScrubStatusResponse};
use crate::utils::{error_response, success_response};
use crate::zfs_management::ZfsManager;
use warp::{Rejection, Reply};

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
pub async fn get_scrub_status_handler(
    pool: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_scrub_status(&pool).await {
        Ok(scrub) => {
            // Calculate percent_done: (examined / to_examine) * 100
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
        Err(e) => Ok(error_response(&format!(
            "Failed to get scrub status: {}",
            e
        ))),
    }
}
