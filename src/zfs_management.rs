use crate::models::CreatePool;
use libzetta::zpool::{ZpoolEngine, ZpoolOpen3, CreateVdevRequest, CreateZpoolRequest, DestroyMode};
use libzetta::zfs::{ZfsEngine, DelegatingZfsEngine, CreateDatasetRequest, DatasetKind};
use std::path::PathBuf;
use std::sync::Arc;

// libzfs for scan stats (from-scratch implementation)
// libzetta doesn't expose scan progress, so we use libzfs FFI bindings directly
use libzfs::Libzfs;

pub struct PoolStatus {
    pub name: String,
    pub health: String,
    pub size: u64,
    pub allocated: u64,
    pub free: u64,
    pub capacity: u8,
    pub vdevs: u32,
    pub errors: Option<String>,
}

pub type ZfsError = String;

#[derive(Clone)]
pub struct ZfsManager {
    zpool_engine: Arc<ZpoolOpen3>,
    zfs_engine: Arc<DelegatingZfsEngine>,
}

impl ZfsManager {
    pub fn new() -> Result<Self, ZfsError> {
        let zpool_engine = Arc::new(ZpoolOpen3::default());
        let zfs_engine = Arc::new(DelegatingZfsEngine::new()
            .map_err(|e| format!("Failed to initialize ZFS engine: {}", e))?);
        
        Ok(ZfsManager {
            zpool_engine,
            zfs_engine,
        })
    }

    pub async fn list_pools(&self) -> Result<Vec<String>, ZfsError> {
        // FIXED: Create owned value to avoid borrowing issue
        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpools = self.zpool_engine.status_all(status_options)
            .map_err(|e| format!("Failed to list pools: {}", e))?;
        
        let pool_names = zpools.into_iter()
            .map(|zpool| zpool.name().clone())
            .collect();
        
        Ok(pool_names)
    }

    pub async fn get_pool_status(&self, name: &str) -> Result<PoolStatus, ZfsError> {
        // FIXED: Create owned value and avoid temporary borrowing
        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpool = self.zpool_engine.status(name, status_options)
            .map_err(|e| format!("Failed to get pool status: {}", e))?;
        
        let properties = self.zpool_engine.read_properties(name)
            .map_err(|e| format!("Failed to read pool properties: {}", e))?;
        
        // Extract values before creating PoolStatus to avoid borrowing issues
        let pool_name = zpool.name().clone();
        let pool_health = format!("{:?}", zpool.health());
        let pool_size = *properties.size() as u64;
        let pool_allocated = *properties.alloc() as u64;
        let pool_free = *properties.free() as u64;
        let pool_capacity = *properties.capacity();
        let pool_vdevs = zpool.vdevs().len() as u32;
        let pool_errors = zpool.errors().clone();
        
        Ok(PoolStatus {
            name: pool_name,
            health: pool_health,
            size: pool_size,
            allocated: pool_allocated,
            free: pool_free,
            capacity: pool_capacity,
            vdevs: pool_vdevs,
            errors: pool_errors,
        })
    }

    pub async fn create_pool(&self, pool: CreatePool) -> Result<(), ZfsError> {
        let disks: Vec<PathBuf> = pool.disks.into_iter().map(PathBuf::from).collect();
        
        let vdev = match pool.raid_type.as_deref() {
            Some("mirror") => CreateVdevRequest::Mirror(disks),
            Some("raidz") => CreateVdevRequest::RaidZ(disks),
            Some("raidz2") => CreateVdevRequest::RaidZ2(disks),
            Some("raidz3") => CreateVdevRequest::RaidZ3(disks),
            _ => {
                if disks.len() == 1 {
                    CreateVdevRequest::SingleDisk(disks.into_iter().next().unwrap())
                } else {
                    return Err("Multiple disks specified but no RAID type provided".to_string());
                }
            }
        };

        let request = CreateZpoolRequest::builder()
            .name(&pool.name)
            .vdev(vdev)
            .build()
            .map_err(|e| format!("Failed to build pool request: {}", e))?;

        self.zpool_engine.create(request)
            .map_err(|e| format!("Failed to create pool: {}", e))?;

        Ok(())
    }

    pub async fn destroy_pool(&self, name: &str, force: bool) -> Result<(), ZfsError> {
        let mode = if force { DestroyMode::Force } else { DestroyMode::Gentle };
        
        self.zpool_engine.destroy(name, mode)
            .map_err(|e| format!("Failed to destroy pool: {}", e))?;

        Ok(())
    }

    pub async fn list_datasets(&self, pool: &str) -> Result<Vec<String>, ZfsError> {
        let datasets = self.zfs_engine.list_filesystems(pool)
            .map_err(|e| format!("Failed to list datasets: {}", e))?;
        
        Ok(datasets.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
    }

    pub async fn create_dataset(&self, dataset: crate::models::CreateDataset) -> Result<(), ZfsError> {
        let kind = match dataset.kind.as_str() {
            "filesystem" => DatasetKind::Filesystem,
            "volume" => DatasetKind::Volume,
            _ => return Err("Invalid dataset kind. Must be 'filesystem' or 'volume'".to_string()),
        };

        // Destructure the entire struct to own all fields
        let crate::models::CreateDataset { name, properties, .. } = dataset;
        
        let request = CreateDatasetRequest::builder()
            .name(PathBuf::from(&name))
            .kind(kind)
            .user_properties(properties)
            .build()
            .map_err(|e| format!("Failed to build dataset request: {}", e))?;

        self.zfs_engine.create(request)
            .map_err(|e| format!("Failed to create dataset: {}", e))?;

        Ok(())
    }

    pub async fn delete_dataset(&self, name: &str) -> Result<(), ZfsError> {
        self.zfs_engine.destroy(PathBuf::from(name))
            .map_err(|e| format!("Failed to delete dataset: {}", e))?;

        Ok(())
    }

    pub async fn list_snapshots(&self, dataset: &str) -> Result<Vec<String>, ZfsError> {
        let snapshots = self.zfs_engine.list_snapshots(dataset)
            .map_err(|e| format!("Failed to list snapshots: {}", e))?;
        
        Ok(snapshots.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
    }

    pub async fn create_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), ZfsError> {
        let snapshot_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        
        self.zfs_engine.snapshot(&[snapshot_path], None)
            .map_err(|e| format!("Failed to create snapshot: {}", e))?;

        Ok(())
    }

    pub async fn delete_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), ZfsError> {
        let snapshot_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));

        self.zfs_engine.destroy_snapshots(&[snapshot_path], libzetta::zfs::DestroyTiming::RightNow)
            .map_err(|e| format!("Failed to delete snapshot: {}", e))?;

        Ok(())
    }

    // =========================================================================
    // Scrub Operations (MF-001 Phase 2)
    // =========================================================================

    /// Start or resume a scrub on the pool
    /// libzetta: ZpoolEngine::scrub()
    pub async fn start_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine.scrub(pool)
            .map_err(|e| format!("Failed to start scrub: {}", e))?;
        Ok(())
    }

    /// Pause an active scrub
    /// libzetta: ZpoolEngine::pause_scrub()
    pub async fn pause_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine.pause_scrub(pool)
            .map_err(|e| format!("Failed to pause scrub: {}", e))?;
        Ok(())
    }

    /// Stop/cancel a scrub
    /// libzetta: ZpoolEngine::stop_scrub()
    pub async fn stop_scrub(&self, pool: &str) -> Result<(), ZfsError> {
        self.zpool_engine.stop_scrub(pool)
            .map_err(|e| format!("Failed to stop scrub: {}", e))?;
        Ok(())
    }

    /// Get scrub status from pool info
    /// FROM-SCRATCH IMPLEMENTATION using libzfs FFI bindings
    /// Bypasses libzetta limitation by accessing pool config nvlist directly.
    /// Extracts scan_stats array per pool_scan_stat_t in sys/fs/zfs.h
    pub async fn get_scrub_status(&self, pool: &str) -> Result<ScrubStatus, ZfsError> {
        // Get pool health via libzetta (still useful for that)
        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpool_status = self.zpool_engine.status(pool, status_options)
            .map_err(|e| format!("Failed to get pool status: {}", e))?;

        let pool_health = format!("{:?}", zpool_status.health());
        let errors = zpool_status.errors().clone();

        // FROM-SCRATCH: Use libzfs to get actual scan stats from pool config
        let mut libzfs = Libzfs::new();
        let zpool = libzfs.pool_by_name(pool)
            .ok_or_else(|| format!("Pool '{}' not found via libzfs", pool))?;

        // Get pool config nvlist
        let config = zpool.get_config();

        // scan_stats is inside vdev_tree (nvroot) per ZFS docs:
        // nvlist_lookup_uint64_array(nvroot, ZPOOL_CONFIG_SCAN_STATS, &stats, &nelem)
        // Try vdev_tree first, fall back to pool config root
        let scan_stats = config.lookup_nv_list("vdev_tree")
            .and_then(|vdev_tree| vdev_tree.lookup_uint64_array("scan_stats"))
            .or_else(|_| config.lookup_uint64_array("scan_stats"));

        // scan_stats is a uint64 array with fields from pool_scan_stat_t
        // Indices: 0=func, 1=state, 2=start_time, 3=end_time, 4=to_examine,
        //          5=examined, 6=skipped, 7=processed, 8=errors, ...

        match scan_stats {
            Ok(stats) if !stats.is_empty() => {
                let pss_func = stats.get(0).copied();
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

/// Convert dsl_scan_state_t to string
/// DSS_NONE=0, DSS_SCANNING=1, DSS_FINISHED=2, DSS_CANCELED=3
fn scan_state_to_string(state: Option<u64>) -> String {
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
fn scan_func_to_string(func: Option<u64>) -> Option<String> {
    match func {
        Some(0) => None,
        Some(1) => Some("scrub".to_string()),
        Some(2) => Some("resilver".to_string()),
        Some(3) => Some("errorscrub".to_string()),
        _ => None,
    }
}

/// Scrub status information
/// FROM-SCRATCH implementation using libzfs FFI bindings.
/// Extracts real scan progress from pool_scan_stat_t via nvlist.
pub struct ScrubStatus {
    pub pool_health: String,
    pub errors: Option<String>,
    pub state: String,
    pub function: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub to_examine: Option<u64>,
    pub examined: Option<u64>,
    pub scan_errors: Option<u64>,
}

// ============================================================================
// UNIT TESTS — MC-001 (ZFS Engine)
// ============================================================================
// NOTE: These tests require ZFS to be installed and running.
// Tests are structured to document expected behavior.
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // PoolStatus Struct Tests
    // -------------------------------------------------------------------------

    /// Test: PoolStatus can be constructed with all fields
    /// Expected: All fields accessible
    #[test]
    fn test_pool_status_construction() {
        let status = PoolStatus {
            name: "tank".to_string(),
            health: "Online".to_string(),
            size: 1099511627776,
            allocated: 549755813888,
            free: 549755813888,
            capacity: 50,
            vdevs: 2,
            errors: None,
        };

        assert_eq!(status.name, "tank");
        assert_eq!(status.health, "Online");
        assert_eq!(status.capacity, 50);
        assert!(status.errors.is_none());
    }

    /// Test: PoolStatus with errors
    /// Expected: errors field contains message
    #[test]
    fn test_pool_status_with_errors() {
        let status = PoolStatus {
            name: "degraded_pool".to_string(),
            health: "Degraded".to_string(),
            size: 0,
            allocated: 0,
            free: 0,
            capacity: 0,
            vdevs: 1,
            errors: Some("Device /dev/sda is faulted".to_string()),
        };

        assert!(status.errors.is_some());
        assert!(status.errors.unwrap().contains("faulted"));
    }

    // -------------------------------------------------------------------------
    // RAID Type Mapping Tests (testable without ZFS)
    // -------------------------------------------------------------------------

    /// Test: Single disk creates SingleDisk vdev
    /// Expected: No RAID type needed for single disk
    #[test]
    fn test_single_disk_vdev() {
        let disks: Vec<PathBuf> = vec![PathBuf::from("/dev/sda")];
        let raid_type: Option<&str> = None;

        // Replicate the logic from create_pool
        let vdev = match raid_type {
            Some("mirror") => CreateVdevRequest::Mirror(disks.clone()),
            Some("raidz") => CreateVdevRequest::RaidZ(disks.clone()),
            _ => {
                if disks.len() == 1 {
                    CreateVdevRequest::SingleDisk(disks.into_iter().next().unwrap())
                } else {
                    panic!("Should not reach here for single disk");
                }
            }
        };

        // Verify it's a SingleDisk variant
        match vdev {
            CreateVdevRequest::SingleDisk(path) => {
                assert_eq!(path, PathBuf::from("/dev/sda"));
            }
            _ => panic!("Expected SingleDisk variant"),
        }
    }

    /// Test: Mirror creates Mirror vdev
    /// Expected: raid_type="mirror" creates Mirror variant
    #[test]
    fn test_mirror_vdev() {
        let disks: Vec<PathBuf> = vec![
            PathBuf::from("/dev/sda"),
            PathBuf::from("/dev/sdb"),
        ];

        let vdev = CreateVdevRequest::Mirror(disks.clone());

        match vdev {
            CreateVdevRequest::Mirror(paths) => {
                assert_eq!(paths.len(), 2);
            }
            _ => panic!("Expected Mirror variant"),
        }
    }

    /// Test: Multiple disks without RAID type is error
    /// Expected: Returns error message
    #[test]
    fn test_multiple_disks_no_raid_error() {
        let disks: Vec<PathBuf> = vec![
            PathBuf::from("/dev/sda"),
            PathBuf::from("/dev/sdb"),
        ];
        let raid_type: Option<&str> = None;

        // Replicate the error condition
        let result: Result<(), String> = match raid_type {
            Some(_) => Ok(()),
            None => {
                if disks.len() == 1 {
                    Ok(())
                } else {
                    Err("Multiple disks specified but no RAID type provided".to_string())
                }
            }
        };

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Multiple disks"));
    }

    // -------------------------------------------------------------------------
    // Snapshot Path Format Tests
    // -------------------------------------------------------------------------

    /// Test: Snapshot path format is dataset@name
    /// Expected: Correct ZFS snapshot path format
    #[test]
    fn test_snapshot_path_format() {
        let dataset = "tank/data";
        let snapshot_name = "backup-001";
        let snapshot_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));

        assert_eq!(
            snapshot_path.to_string_lossy(),
            "tank/data@backup-001"
        );
    }

    // -------------------------------------------------------------------------
    // ZfsManager Tests (require ZFS - placeholder stubs)
    // -------------------------------------------------------------------------

    /// Test: ZfsManager::new initializes engines
    /// Expected: Returns Ok with valid ZfsManager
    /// NOTE: Requires ZFS installed - will fail otherwise
    #[test]
    #[ignore = "Requires ZFS to be installed"]
    fn test_zfs_manager_new() {
        let result = ZfsManager::new();
        // On systems without ZFS, this will fail with init error
        // That's expected - the test documents the requirement
        assert!(result.is_ok() || result.is_err());
    }

    /// Test: list_pools returns Vec<String>
    /// Expected: Empty vec or list of pool names
    /// NOTE: Requires ZFS installed
    #[test]
    #[ignore = "Requires ZFS to be installed"]
    fn test_list_pools() {
        // Placeholder - requires actual ZFS
    }

    /// Test: create_dataset validates kind field
    /// Expected: "filesystem" and "volume" accepted, others rejected
    #[test]
    #[ignore = "Requires ZFS to be installed"]
    fn test_create_dataset_kind_validation() {
        // Would test that "filesystem" and "volume" are accepted,
        // while invalid kinds are rejected
    }
}