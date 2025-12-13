// zfs_management/scrub.rs
// Scrub operations: start, pause, stop, status

use super::helpers::{scan_func_to_string, scan_state_to_string};
use super::manager::ZfsManager;
use super::types::{ScrubStatus, ZfsError};
use libzetta::zpool::ZpoolEngine;
use libzfs::Libzfs;

impl ZfsManager {
    /// Start or resume a scrub on the pool
    pub async fn start_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine
            .scrub(pool)
            .map_err(|e| format!("Failed to start scrub: {}", e))?;
        Ok(())
    }

    /// Pause an active scrub
    pub async fn pause_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine
            .pause_scrub(pool)
            .map_err(|e| format!("Failed to pause scrub: {}", e))?;
        Ok(())
    }

    /// Stop/cancel a scrub
    pub async fn stop_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine
            .stop_scrub(pool)
            .map_err(|e| format!("Failed to stop scrub: {}", e))?;
        Ok(())
    }

    /// Get scrub status from pool info
    /// Implementation via libzfs FFI bindings (bypasses libzetta limitation)
    pub async fn get_scrub_status(&self, pool: &str) -> Result<ScrubStatus, ZfsError> {
        // Guard against libzetta panic: check pool exists before calling status()
        if !self
            .zpool_engine
            .exists(pool)
            .map_err(|e| format!("Failed to check pool existence: {}", e))?
        {
            return Err(format!("Pool '{}' not found", pool));
        }

        // Get pool health via libzetta
        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpool_status = self
            .zpool_engine
            .status(pool, status_options)
            .map_err(|e| format!("Failed to get pool status: {}", e))?;

        let pool_health = format!("{:?}", zpool_status.health());
        let errors = zpool_status.errors().clone();

        // Use libzfs FFI to get actual scan stats from pool config
        let mut libzfs = Libzfs::new();
        let zpool = libzfs
            .pool_by_name(pool)
            .ok_or_else(|| format!("Pool '{}' not found via libzfs", pool))?;

        let config = zpool.get_config();

        // scan_stats is inside vdev_tree (nvroot) per ZFS docs
        let scan_stats = config
            .lookup_nv_list("vdev_tree")
            .and_then(|vdev_tree| vdev_tree.lookup_uint64_array("scan_stats"))
            .or_else(|_| config.lookup_uint64_array("scan_stats"));

        // scan_stats is a uint64 array with fields from pool_scan_stat_t
        // Indices: 0=func, 1=state, 2=start_time, 3=end_time, 4=to_examine,
        //          5=examined, 6=skipped, 7=processed, 8=errors, ...

        match scan_stats {
            Ok(stats) if !stats.is_empty() => {
                let pss_func = stats.first().copied();
                let pss_state = stats.get(1).copied();
                let pss_start_time = stats.get(2).copied();
                let pss_end_time = stats.get(3).copied();
                let pss_to_examine = stats.get(4).copied();
                let pss_examined = stats.get(5).copied();
                let pss_errors = stats.get(8).copied();

                Ok(ScrubStatus {
                    pool_health,
                    errors,
                    state: scan_state_to_string(pss_state),
                    function: scan_func_to_string(pss_func),
                    start_time: pss_start_time,
                    end_time: pss_end_time,
                    to_examine: pss_to_examine,
                    examined: pss_examined,
                    scan_errors: pss_errors,
                })
            }
            _ => {
                // No scan stats available (never scanned)
                Ok(ScrubStatus {
                    pool_health,
                    errors,
                    state: "none".to_string(),
                    function: None,
                    start_time: None,
                    end_time: None,
                    to_examine: None,
                    examined: None,
                    scan_errors: None,
                })
            }
        }
    }
}
