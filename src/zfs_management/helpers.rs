// zfs_management/helpers.rs
// Helper functions for ZFS management

/// Convert errno to descriptive string
pub fn errno_to_string(errno: i32) -> &'static str {
    match errno {
        libc::ENOENT => "dataset or snapshot not found",
        libc::EEXIST => "dataset already exists",
        libc::EBUSY => "dataset is busy",
        libc::EINVAL => "invalid argument",
        libc::EPERM => "permission denied",
        libc::ENOSPC => "no space left on device",
        libc::EDQUOT => "quota exceeded",
        _ => "unknown error",
    }
}

/// Convert dsl_scan_state_t to string
/// DSS_NONE=0, DSS_SCANNING=1, DSS_FINISHED=2, DSS_CANCELED=3
pub fn scan_state_to_string(state: Option<u64>) -> String {
    match state {
        Some(0) => "none".to_string(),
        Some(1) => "scanning".to_string(),
        Some(2) => "finished".to_string(),
        Some(3) => "canceled".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Convert pool_scan_func_t to string
/// POOL_SCAN_NONE=0, POOL_SCAN_SCRUB=1, POOL_SCAN_RESILVER=2, POOL_SCAN_ERRORSCRUB=3
pub fn scan_func_to_string(func: Option<u64>) -> Option<String> {
    match func {
        Some(0) => None,
        Some(1) => Some("scrub".to_string()),
        Some(2) => Some("resilver".to_string()),
        Some(3) => Some("errorscrub".to_string()),
        _ => None,
    }
}
