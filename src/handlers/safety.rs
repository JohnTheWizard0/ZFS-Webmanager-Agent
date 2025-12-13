// handlers/safety.rs
// Safety lock handlers: safety_status, safety_override

use crate::models::{SafetyOverrideRequest, SafetyOverrideResponse, SafetyStatusResponse};
use crate::safety::SafetyManager;
use warp::{Rejection, Reply};

/// GET /v1/safety - Get safety status
/// Always accessible (no auth, works even when locked)
pub async fn safety_status_handler(
    safety_manager: SafetyManager,
) -> Result<impl Reply, Rejection> {
    let state = safety_manager.get_state();

    Ok(warp::reply::json(&SafetyStatusResponse {
        status: "success".to_string(),
        locked: state.locked,
        compatible: state.compatible,
        zfs_version: state.zfs_version,
        agent_version: state.agent_version,
        approved_versions: state.approved_versions,
        lock_reason: state.lock_reason,
        override_at: state.override_at,
    }))
}

/// POST /v1/safety - Override safety lock
/// Always accessible (no auth, works even when locked)
pub async fn safety_override_handler(
    body: SafetyOverrideRequest,
    safety_manager: SafetyManager,
) -> Result<impl Reply, Rejection> {
    if body.action != "override" {
        return Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "error".to_string(),
            message: format!("Unknown action '{}'. Use 'override'.", body.action),
            locked: safety_manager.is_locked(),
        }));
    }

    match safety_manager.override_lock() {
        Ok(_) => Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "success".to_string(),
            message: "Safety lock disabled. All operations now permitted.".to_string(),
            locked: false,
        })),
        Err(e) => Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "error".to_string(),
            message: e,
            locked: safety_manager.is_locked(),
        })),
    }
}
