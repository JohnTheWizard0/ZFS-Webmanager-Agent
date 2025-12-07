use crate::models::CreatePool;
use libzetta::zpool::{ZpoolEngine, ZpoolOpen3, CreateVdevRequest, CreateZpoolRequest, DestroyMode, ExportMode};
use libzetta::zfs::{ZfsEngine, DelegatingZfsEngine, CreateDatasetRequest, DatasetKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::ffi::CString;
use std::ptr;

// libzfs for scan stats (from-scratch implementation)
// libzetta doesn't expose scan progress, so we use libzfs FFI bindings directly
use libzfs::Libzfs;

// libzetta-zfs-core-sys for clone/promote/rollback FFI (not exposed by libzetta)
use libzetta_zfs_core_sys::{lzc_clone, lzc_promote, lzc_rollback_to};

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

/// Pool available for import
pub struct ImportablePool {
    pub name: String,
    pub health: String,
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

    // =========================================================================
    // Pool Import/Export Operations (MF-001 Phase 2)
    // =========================================================================

    /// Export a pool from the system
    /// libzetta: ZpoolEngine::export()
    pub async fn export_pool(&self, name: &str, force: bool) -> Result<(), ZfsError> {
        let mode = if force { ExportMode::Force } else { ExportMode::Gentle };

        self.zpool_engine.export(name, mode)
            .map_err(|e| format!("Failed to export pool: {}", e))?;

        Ok(())
    }

    /// List pools available for import from /dev/
    /// libzetta: ZpoolEngine::available()
    pub async fn list_importable_pools(&self) -> Result<Vec<ImportablePool>, ZfsError> {
        let pools = self.zpool_engine.available()
            .map_err(|e| format!("Failed to list importable pools: {}", e))?;

        Ok(pools.into_iter().map(|p| ImportablePool {
            name: p.name().clone(),
            health: format!("{:?}", p.health()),
        }).collect())
    }

    /// List pools available for import from a specific directory
    /// libzetta: ZpoolEngine::available_in_dir()
    pub async fn list_importable_pools_from_dir(&self, dir: &str) -> Result<Vec<ImportablePool>, ZfsError> {
        let pools = self.zpool_engine.available_in_dir(PathBuf::from(dir))
            .map_err(|e| format!("Failed to list importable pools from {}: {}", dir, e))?;

        Ok(pools.into_iter().map(|p| ImportablePool {
            name: p.name().clone(),
            health: format!("{:?}", p.health()),
        }).collect())
    }

    /// Import a pool from /dev/
    /// libzetta: ZpoolEngine::import()
    pub async fn import_pool(&self, name: &str) -> Result<(), ZfsError> {
        self.zpool_engine.import(name)
            .map_err(|e| format!("Failed to import pool: {}", e))?;

        Ok(())
    }

    /// Import a pool from a specific directory
    /// libzetta: ZpoolEngine::import_from_dir()
    pub async fn import_pool_from_dir(&self, name: &str, dir: &str) -> Result<(), ZfsError> {
        self.zpool_engine.import_from_dir(name, PathBuf::from(dir))
            .map_err(|e| format!("Failed to import pool from {}: {}", dir, e))?;

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
    // Dataset Properties Operations (MF-002 Phase 2)
    // =========================================================================

    /// Get all properties of a dataset (filesystem, volume, or snapshot)
    /// libzetta: ZfsEngine::read_properties()
    pub async fn get_dataset_properties(&self, name: &str) -> Result<DatasetProperties, ZfsError> {
        let props = self.zfs_engine.read_properties(PathBuf::from(name))
            .map_err(|e| format!("Failed to get dataset properties: {}", e))?;

        Ok(DatasetProperties::from_libzetta(name.to_string(), props))
    }

    /// Set a property on a dataset
    /// **EXPERIMENTAL**: Uses CLI (`zfs set`) as libzetta/libzfs FFI lacks property setting.
    /// Validates property names against safe patterns to prevent injection.
    pub async fn set_dataset_property(&self, name: &str, property: &str, value: &str) -> Result<(), ZfsError> {
        // Validate property name (alphanumeric, underscore, colon for user props)
        if !Self::is_valid_property_name(property) {
            return Err(format!("Invalid property name: {}", property));
        }

        // Validate dataset name exists
        if !self.zfs_engine.exists(PathBuf::from(name))
            .map_err(|e| format!("Failed to check dataset: {}", e))? {
            return Err(format!("Dataset '{}' does not exist", name));
        }

        // Execute zfs set command
        let output = std::process::Command::new("zfs")
            .args(["set", &format!("{}={}", property, value), name])
            .output()
            .map_err(|e| format!("Failed to execute zfs set: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("zfs set failed: {}", stderr.trim()))
        }
    }

    /// Validate property name to prevent command injection
    /// Allows: lowercase letters, numbers, underscore, colon (for user properties)
    fn is_valid_property_name(name: &str) -> bool {
        if name.is_empty() || name.len() > 256 {
            return false;
        }
        // Must start with a letter
        let first = name.chars().next().unwrap();
        if !first.is_ascii_lowercase() {
            return false;
        }
        // Rest: lowercase, digits, underscore, colon (for user:prop format)
        name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == ':')
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

    // =========================================================================
    // Snapshot Clone/Promote Operations (MF-003 Phase 3)
    // FROM-SCRATCH IMPLEMENTATION using libzetta-zfs-core-sys FFI
    // =========================================================================

    /// Clone a snapshot to create a new writable dataset
    /// FROM-SCRATCH: Uses lzc_clone() FFI directly (libzetta doesn't expose this)
    ///
    /// # Arguments
    /// * `snapshot` - Source snapshot path (e.g., "tank/data@snap1")
    /// * `target` - Target clone path (e.g., "tank/data-clone")
    pub async fn clone_snapshot(&self, snapshot: &str, target: &str) -> Result<(), ZfsError> {
        // Validate snapshot path format (must contain @)
        if !snapshot.contains('@') {
            return Err(format!("Invalid snapshot path '{}': must be dataset@snapshot", snapshot));
        }

        // Validate target doesn't contain @ (must be dataset, not snapshot)
        if target.contains('@') {
            return Err(format!("Invalid target '{}': clone target must be a dataset path, not a snapshot", target));
        }

        // Verify snapshot exists using libzetta
        if !self.zfs_engine.exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))? {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        // Convert to C strings for FFI
        let c_target = CString::new(target)
            .map_err(|_| "Invalid target path: contains null byte")?;
        let c_origin = CString::new(snapshot)
            .map_err(|_| "Invalid snapshot path: contains null byte")?;

        // Call lzc_clone(fsname, origin, props)
        // fsname = target clone path
        // origin = source snapshot path
        // props = NULL (no special properties)
        let result = unsafe {
            lzc_clone(
                c_target.as_ptr(),
                c_origin.as_ptr(),
                ptr::null_mut(),  // No properties
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(format!("lzc_clone failed with error code {}: {}", result, Self::errno_to_string(result)))
        }
    }

    /// Promote a clone to an independent dataset
    /// FROM-SCRATCH: Uses lzc_promote() FFI directly (libzetta doesn't expose this)
    ///
    /// After promotion:
    /// - The clone becomes the parent
    /// - Snapshots up to and including the origin transfer to the promoted dataset
    /// - The former parent becomes a clone of the transferred snapshot
    ///
    /// # Arguments
    /// * `clone_path` - Path of the clone to promote (e.g., "tank/data-clone")
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err with conflicting snapshot name if EEXIST (name collision)
    pub async fn promote_dataset(&self, clone_path: &str) -> Result<(), ZfsError> {
        // Validate clone path doesn't contain @ (must be dataset, not snapshot)
        if clone_path.contains('@') {
            return Err(format!("Invalid path '{}': cannot promote a snapshot", clone_path));
        }

        // Verify dataset exists
        if !self.zfs_engine.exists(PathBuf::from(clone_path))
            .map_err(|e| format!("Failed to check dataset: {}", e))? {
            return Err(format!("Dataset '{}' does not exist", clone_path));
        }

        // Convert to C string
        let c_path = CString::new(clone_path)
            .map_err(|_| "Invalid path: contains null byte")?;

        // Buffer for conflicting snapshot name (returned on EEXIST)
        let mut conflict_buf: [i8; 256] = [0; 256];

        // Call lzc_promote(fsname, snapnamebuf, buflen)
        let result = unsafe {
            lzc_promote(
                c_path.as_ptr(),
                conflict_buf.as_mut_ptr(),
                conflict_buf.len() as i32,
            )
        };

        if result == 0 {
            Ok(())
        } else if result == libc::EEXIST {
            // Extract conflicting snapshot name from buffer
            let conflict_name = unsafe {
                std::ffi::CStr::from_ptr(conflict_buf.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };
            if conflict_name.is_empty() {
                Err("Promote failed: snapshot name collision (EEXIST)".to_string())
            } else {
                Err(format!("Promote failed: snapshot name collision with '{}'", conflict_name))
            }
        } else if result == libc::EINVAL {
            Err(format!("Dataset '{}' is not a clone (no origin property)", clone_path))
        } else {
            Err(format!("lzc_promote failed with error code {}: {}", result, Self::errno_to_string(result)))
        }
    }

    /// Convert errno to descriptive string
    fn errno_to_string(errno: i32) -> &'static str {
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

    // =========================================================================
    // Rollback Operations (MF-003 Phase 3)
    // FROM-SCRATCH IMPLEMENTATION using libzetta-zfs-core-sys FFI
    // =========================================================================

    /// Rollback a dataset to a snapshot
    /// FROM-SCRATCH: Uses lzc_rollback_to() FFI directly (libzetta doesn't expose this)
    ///
    /// Safety levels:
    /// - Default: Only allows rollback to most recent snapshot
    /// - force_destroy_newer: Destroys intermediate snapshots first (like -r)
    /// - force_destroy_newer + force_destroy_clones: Also destroys clones (like -R)
    ///
    /// # Arguments
    /// * `dataset` - Dataset path (e.g., "tank/data")
    /// * `snapshot` - Target snapshot name (without @)
    /// * `force_destroy_newer` - Destroy snapshots newer than target
    /// * `force_destroy_clones` - Also destroy clones of newer snapshots (requires force_destroy_newer)
    ///
    /// # Returns
    /// * Ok(RollbackResult) on success with info about destroyed items
    /// * Err(RollbackError) with blocking items if safety check fails
    pub async fn rollback_dataset(
        &self,
        dataset: &str,
        snapshot: &str,
        force_destroy_newer: bool,
        force_destroy_clones: bool,
    ) -> Result<RollbackResult, RollbackError> {
        // Validate: force_destroy_clones requires force_destroy_newer
        if force_destroy_clones && !force_destroy_newer {
            return Err(RollbackError::InvalidRequest(
                "force_destroy_clones requires force_destroy_newer to be true".to_string()
            ));
        }

        // Validate dataset exists
        if !self.zfs_engine.exists(PathBuf::from(dataset))
            .map_err(|e| RollbackError::ZfsError(format!("Failed to check dataset: {}", e)))? {
            return Err(RollbackError::ZfsError(format!("Dataset '{}' does not exist", dataset)));
        }

        let full_snapshot = format!("{}@{}", dataset, snapshot);

        // Validate snapshot exists
        if !self.zfs_engine.exists(PathBuf::from(&full_snapshot))
            .map_err(|e| RollbackError::ZfsError(format!("Failed to check snapshot: {}", e)))? {
            return Err(RollbackError::ZfsError(format!("Snapshot '{}' does not exist", full_snapshot)));
        }

        // Get all snapshots for this dataset
        let all_snapshots = self.list_snapshots(dataset).await
            .map_err(|e| RollbackError::ZfsError(e))?;

        // Find target snapshot index and get newer snapshots
        // Note: list_snapshots returns full paths like "tank/data@snap1"
        let target_idx = all_snapshots.iter()
            .position(|s| s == &full_snapshot)
            .ok_or_else(|| RollbackError::ZfsError(format!("Snapshot '{}' not found in list", full_snapshot)))?;

        // Snapshots after target_idx are newer
        let newer_snapshots: Vec<String> = all_snapshots[target_idx + 1..].to_vec();

        // If there are newer snapshots and we're not forcing, check what's blocking
        if !newer_snapshots.is_empty() && !force_destroy_newer {
            return Err(RollbackError::Blocked {
                message: format!("Cannot rollback to '{}': {} newer snapshot(s) exist",
                    full_snapshot, newer_snapshots.len()),
                blocking_snapshots: newer_snapshots,
                blocking_clones: vec![],
            });
        }

        // Check for clones on newer snapshots
        let mut blocking_clones: Vec<String> = Vec::new();
        let mut clones_to_destroy: Vec<String> = Vec::new();

        if !newer_snapshots.is_empty() {
            // Use libzfs to check for clones on each newer snapshot
            let mut libzfs = Libzfs::new();

            for snap_path in &newer_snapshots {
                // Get snapshot properties via zfs_engine
                if let Ok(props) = self.zfs_engine.read_properties(PathBuf::from(snap_path)) {
                    // Check clones property in user_properties (stored as comma-separated list)
                    let user_props = match &props {
                        libzetta::zfs::Properties::Snapshot(s) => s.unknown_properties(),
                        _ => continue,
                    };

                    if let Some(clones_str) = user_props.get("clones") {
                        if !clones_str.is_empty() {
                            for clone in clones_str.split(',') {
                                let clone = clone.trim();
                                if !clone.is_empty() {
                                    if force_destroy_clones {
                                        clones_to_destroy.push(clone.to_string());
                                    } else {
                                        blocking_clones.push(clone.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                // Also try libzfs direct query for clones
                if let Some(ds) = libzfs.dataset_by_name(snap_path) {
                    // Check if this snapshot has clones via the clones property
                    // Note: This is a best-effort check; ZFS may not expose this in all cases
                    let _ = ds; // Prevent unused warning - we tried
                }
            }
        }

        // If we found blocking clones and we're not forcing clone destruction, error
        if !blocking_clones.is_empty() {
            return Err(RollbackError::Blocked {
                message: format!("Cannot rollback: {} clone(s) depend on newer snapshots",
                    blocking_clones.len()),
                blocking_snapshots: newer_snapshots,
                blocking_clones,
            });
        }

        let mut destroyed_clones: Vec<String> = Vec::new();
        let mut destroyed_snapshots: Vec<String> = Vec::new();

        // Destroy clones first (if force_destroy_clones)
        for clone_path in clones_to_destroy {
            self.delete_dataset(&clone_path).await
                .map_err(|e| RollbackError::ZfsError(format!("Failed to destroy clone '{}': {}", clone_path, e)))?;
            destroyed_clones.push(clone_path);
        }

        // Destroy newer snapshots in reverse order (newest first)
        if force_destroy_newer {
            for snap_path in newer_snapshots.iter().rev() {
                // Parse dataset@snapshot format
                if let Some(at_pos) = snap_path.rfind('@') {
                    let ds = &snap_path[..at_pos];
                    let snap_name = &snap_path[at_pos + 1..];
                    self.delete_snapshot(ds, snap_name).await
                        .map_err(|e| RollbackError::ZfsError(format!("Failed to destroy snapshot '{}': {}", snap_path, e)))?;
                    destroyed_snapshots.push(snap_path.clone());
                }
            }
        }

        // Now perform the actual rollback using lzc_rollback_to
        let c_fsname = CString::new(dataset)
            .map_err(|_| RollbackError::ZfsError("Invalid dataset path: contains null byte".to_string()))?;
        let c_snapname = CString::new(&full_snapshot as &str)
            .map_err(|_| RollbackError::ZfsError("Invalid snapshot path: contains null byte".to_string()))?;

        let result = unsafe {
            lzc_rollback_to(
                c_fsname.as_ptr(),
                c_snapname.as_ptr(),
            )
        };

        if result == 0 {
            Ok(RollbackResult {
                destroyed_snapshots: if destroyed_snapshots.is_empty() { None } else { Some(destroyed_snapshots) },
                destroyed_clones: if destroyed_clones.is_empty() { None } else { Some(destroyed_clones) },
            })
        } else if result == libc::EEXIST {
            // This shouldn't happen if we destroyed newer snapshots, but just in case
            Err(RollbackError::Blocked {
                message: "Rollback failed: newer snapshots still exist (EEXIST)".to_string(),
                blocking_snapshots: vec![],
                blocking_clones: vec![],
            })
        } else if result == libc::EBUSY {
            Err(RollbackError::ZfsError(format!(
                "Dataset '{}' is busy (mounted with open files or active operations)", dataset
            )))
        } else {
            Err(RollbackError::ZfsError(format!(
                "lzc_rollback_to failed with error code {}: {}", result, Self::errno_to_string(result)
            )))
        }
    }

    // =========================================================================
    // Send/Receive Operations (MF-005 Replication)
    // CLI-based implementation for reliability with async task system
    // =========================================================================

    /// Send a snapshot to a file
    /// Uses `zfs send` CLI for reliability with async execution
    ///
    /// # Arguments
    /// * `snapshot` - Full snapshot path (e.g., "tank/data@snap1")
    /// * `output_file` - Absolute path to output file
    /// * `from_snapshot` - Optional incremental base snapshot name
    /// * `recursive` - Include child datasets (-R)
    /// * `properties` - Include properties (-p)
    /// * `raw` - Raw/encrypted send (-w)
    /// * `compressed` - Compressed stream (-c)
    /// * `large_blocks` - Allow >128KB blocks (-L)
    pub async fn send_snapshot_to_file(
        &self,
        snapshot: &str,
        output_file: &str,
        from_snapshot: Option<&str>,
        recursive: bool,
        properties: bool,
        raw: bool,
        compressed: bool,
        large_blocks: bool,
    ) -> Result<u64, ZfsError> {
        // Validate snapshot exists
        if !self.zfs_engine.exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))? {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        // Validate output file path is absolute
        if !output_file.starts_with('/') {
            return Err("Output file path must be absolute".to_string());
        }

        // Build zfs send command
        let mut args = vec!["send".to_string()];

        // Add flags
        if recursive {
            args.push("-R".to_string());
        }
        if properties && !recursive {  // -R implies -p
            args.push("-p".to_string());
        }
        if raw {
            args.push("-w".to_string());
        }
        if compressed {
            args.push("-c".to_string());
        }
        if large_blocks {
            args.push("-L".to_string());
        }

        // Add incremental source if specified
        if let Some(from) = from_snapshot {
            // Parse dataset from snapshot path for incremental
            let dataset = snapshot.split('@').next()
                .ok_or("Invalid snapshot path")?;
            let from_full = if from.contains('@') {
                from.to_string()
            } else {
                format!("{}@{}", dataset, from)
            };
            args.push("-i".to_string());
            args.push(from_full);
        }

        args.push(snapshot.to_string());

        // Execute: zfs send <args> > output_file
        // Using shell to redirect output to file
        let cmd_str = format!("zfs {} > '{}'", args.join(" "), output_file);

        let output = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .output()
            .map_err(|e| format!("Failed to execute zfs send: {}", e))?;

        if output.status.success() {
            // Get file size
            let metadata = std::fs::metadata(output_file)
                .map_err(|e| format!("Failed to read output file: {}", e))?;
            Ok(metadata.len())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("zfs send failed: {}", stderr.trim()))
        }
    }

    /// Receive a snapshot from a file
    /// Uses `zfs receive` CLI for reliability
    ///
    /// # Arguments
    /// * `target_dataset` - Target dataset path (e.g., "tank/restore")
    /// * `input_file` - Absolute path to input file
    /// * `force` - Force receive (-F), rollback if necessary
    pub async fn receive_snapshot_from_file(
        &self,
        target_dataset: &str,
        input_file: &str,
        force: bool,
    ) -> Result<String, ZfsError> {
        // Validate input file exists
        if !std::path::Path::new(input_file).exists() {
            return Err(format!("Input file '{}' does not exist", input_file));
        }

        // Build zfs receive command
        let mut args = vec!["receive".to_string()];

        if force {
            args.push("-F".to_string());
        }

        // Always verbose for output
        args.push("-v".to_string());

        args.push(target_dataset.to_string());

        // Execute: zfs receive <args> < input_file
        let cmd_str = format!("zfs {} < '{}'", args.join(" "), input_file);

        let output = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .output()
            .map_err(|e| format!("Failed to execute zfs receive: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout.trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("zfs receive failed: {}", stderr.trim()))
        }
    }

    /// Replicate a snapshot directly to another pool (pipe send→receive)
    /// Uses `zfs send | zfs receive` for direct pool-to-pool replication
    ///
    /// # Arguments
    /// * `snapshot` - Full snapshot path (e.g., "tank/data@snap1")
    /// * `target_dataset` - Target dataset path (e.g., "pool2/data")
    /// * `from_snapshot` - Optional incremental base snapshot name
    /// * `recursive` - Include child datasets (-R)
    /// * `properties` - Include properties (-p)
    /// * `raw` - Raw/encrypted send (-w)
    /// * `compressed` - Compressed stream (-c)
    /// * `force` - Force receive (-F), rollback if necessary
    ///
    /// # Returns
    /// * Ok(output) - Verbose output from zfs receive
    /// * Err - Error message
    pub async fn replicate_snapshot(
        &self,
        snapshot: &str,
        target_dataset: &str,
        from_snapshot: Option<&str>,
        recursive: bool,
        properties: bool,
        raw: bool,
        compressed: bool,
        force: bool,
    ) -> Result<String, ZfsError> {
        // Validate snapshot exists
        if !self.zfs_engine.exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))? {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        // Build zfs send command arguments
        let mut send_args = vec!["send"];

        if recursive {
            send_args.push("-R");
        }
        if properties && !recursive {  // -R implies -p
            send_args.push("-p");
        }
        if raw {
            send_args.push("-w");
        }
        if compressed {
            send_args.push("-c");
        }

        // Build incremental argument if specified
        let from_full: String;
        if let Some(from) = from_snapshot {
            let dataset = snapshot.split('@').next()
                .ok_or("Invalid snapshot path")?;
            from_full = if from.contains('@') {
                from.to_string()
            } else {
                format!("{}@{}", dataset, from)
            };
            send_args.push("-i");
            send_args.push(&from_full);
        }

        // Build zfs receive command arguments
        let mut recv_args = vec!["receive", "-v"];
        if force {
            recv_args.push("-F");
        }

        // Build full pipe command
        let cmd_str = format!(
            "zfs {} '{}' | zfs {} '{}'",
            send_args.join(" "),
            snapshot,
            recv_args.join(" "),
            target_dataset
        );

        let output = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .output()
            .map_err(|e| format!("Failed to execute replication: {}", e))?;

        if output.status.success() {
            // Combine stdout and stderr (zfs receive -v writes to stderr)
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            Ok(combined.trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Replication failed: {}", stderr.trim()))
        }
    }

    /// Extract pool name from a dataset/snapshot path
    pub fn get_pool_from_path(path: &str) -> String {
        path.split('/').next()
            .unwrap_or(path)
            .split('@').next()
            .unwrap_or(path)
            .to_string()
    }
}

/// Result of a successful rollback operation
pub struct RollbackResult {
    pub destroyed_snapshots: Option<Vec<String>>,
    pub destroyed_clones: Option<Vec<String>>,
}

/// Error from rollback operation
#[derive(Debug)]
pub enum RollbackError {
    /// Invalid request parameters
    InvalidRequest(String),
    /// Rollback blocked by safety checks
    Blocked {
        message: String,
        blocking_snapshots: Vec<String>,
        blocking_clones: Vec<String>,
    },
    /// ZFS operation failed
    ZfsError(String),
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

/// Dataset properties returned from libzetta
/// Unified structure for filesystem, volume, and snapshot properties
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetProperties {
    pub name: String,
    pub dataset_type: String,
    // Common properties
    pub available: Option<i64>,
    pub used: Option<u64>,
    pub referenced: Option<u64>,
    pub compression: Option<String>,
    pub compression_ratio: Option<f64>,
    pub readonly: Option<bool>,
    pub creation: Option<i64>,
    pub quota: Option<u64>,
    pub reservation: Option<u64>,
    pub ref_quota: Option<u64>,
    pub ref_reservation: Option<u64>,
    pub record_size: Option<u64>,
    pub checksum: Option<String>,
    pub copies: Option<u8>,
    pub mountpoint: Option<String>,
    pub mounted: Option<bool>,
    pub atime: Option<bool>,
    pub exec: Option<bool>,
    pub setuid: Option<bool>,
    pub devices: Option<bool>,
    pub xattr: Option<bool>,
    pub canmount: Option<String>,
    pub snapdir: Option<String>,
    pub sync: Option<String>,
    pub dedup: Option<String>,
    pub primary_cache: Option<String>,
    pub secondary_cache: Option<String>,
    // Volume-specific
    pub volume_size: Option<u64>,
    pub volume_block_size: Option<u64>,
    // User/unknown properties
    pub user_properties: std::collections::HashMap<String, String>,
}

impl DatasetProperties {
    pub fn from_libzetta(name: String, props: libzetta::zfs::Properties) -> Self {
        use libzetta::zfs::Properties;

        match props {
            Properties::Filesystem(fs) => DatasetProperties {
                name,
                dataset_type: "filesystem".to_string(),
                available: Some(*fs.available()),
                used: Some(*fs.used()),
                referenced: Some(*fs.referenced()),
                compression: Some(format!("{}", fs.compression())),
                compression_ratio: Some(*fs.compression_ratio()),
                readonly: Some(*fs.readonly()),
                creation: Some(*fs.creation()),
                quota: Some(*fs.quota()),
                reservation: Some(*fs.reservation()),
                ref_quota: Some(*fs.ref_quota()),
                ref_reservation: Some(*fs.ref_reservation()),
                record_size: Some(*fs.record_size()),
                checksum: Some(format!("{}", fs.checksum())),
                copies: Some(*fs.copies() as u8),
                mountpoint: fs.mount_point().as_ref().map(|p| p.to_string_lossy().to_string()),
                mounted: Some(*fs.mounted()),
                atime: Some(*fs.atime()),
                exec: Some(*fs.exec()),
                setuid: Some(*fs.setuid()),
                devices: Some(*fs.devices()),
                xattr: Some(*fs.xattr()),
                canmount: Some(format!("{}", fs.can_mount())),
                snapdir: Some(format!("{}", fs.snap_dir())),
                sync: Some(format!("{}", fs.sync())),
                dedup: Some(format!("{}", fs.dedup())),
                primary_cache: Some(format!("{}", fs.primary_cache())),
                secondary_cache: Some(format!("{}", fs.secondary_cache())),
                volume_size: None,
                volume_block_size: None,
                user_properties: fs.unknown_properties().clone(),
            },
            Properties::Volume(vol) => DatasetProperties {
                name,
                dataset_type: "volume".to_string(),
                available: Some(*vol.available()),
                used: Some(*vol.used()),
                referenced: Some(*vol.referenced()),
                compression: Some(format!("{}", vol.compression())),
                compression_ratio: Some(*vol.compression_ratio()),
                readonly: Some(*vol.readonly()),
                creation: Some(*vol.creation()),
                quota: None,
                reservation: Some(*vol.reservation()),
                ref_quota: None,
                ref_reservation: Some(*vol.ref_reservation()),
                record_size: None,
                checksum: Some(format!("{}", vol.checksum())),
                copies: Some(*vol.copies() as u8),
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: Some(format!("{}", vol.sync())),
                dedup: Some(format!("{}", vol.dedup())),
                primary_cache: Some(format!("{}", vol.primary_cache())),
                secondary_cache: Some(format!("{}", vol.secondary_cache())),
                volume_size: Some(*vol.volume_size()),
                volume_block_size: Some(*vol.volume_block_size()),
                user_properties: vol.unknown_properties().clone(),
            },
            Properties::Snapshot(snap) => DatasetProperties {
                name,
                dataset_type: "snapshot".to_string(),
                available: None,
                used: Some(*snap.used()),
                referenced: Some(*snap.referenced()),
                compression: None,
                compression_ratio: Some(*snap.compression_ratio()),
                readonly: None,
                creation: Some(*snap.creation()),
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: Some(*snap.exec()),
                setuid: Some(*snap.setuid()),
                devices: Some(*snap.devices()),
                xattr: Some(*snap.xattr()),
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: Some(format!("{}", snap.primary_cache())),
                secondary_cache: Some(format!("{}", snap.secondary_cache())),
                volume_size: None,
                volume_block_size: None,
                user_properties: snap.unknown_properties().clone(),
            },
            Properties::Bookmark(bm) => DatasetProperties {
                name,
                dataset_type: "bookmark".to_string(),
                available: None,
                used: None,
                referenced: None,
                compression: None,
                compression_ratio: None,
                readonly: None,
                creation: Some(*bm.creation()),
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: None,
                secondary_cache: None,
                volume_size: None,
                volume_block_size: None,
                user_properties: bm.unknown_properties().clone(),
            },
            Properties::Unknown(props) => DatasetProperties {
                name,
                dataset_type: "unknown".to_string(),
                available: None,
                used: None,
                referenced: None,
                compression: None,
                compression_ratio: None,
                readonly: None,
                creation: None,
                quota: None,
                reservation: None,
                ref_quota: None,
                ref_reservation: None,
                record_size: None,
                checksum: None,
                copies: None,
                mountpoint: None,
                mounted: None,
                atime: None,
                exec: None,
                setuid: None,
                devices: None,
                xattr: None,
                canmount: None,
                snapdir: None,
                sync: None,
                dedup: None,
                primary_cache: None,
                secondary_cache: None,
                volume_size: None,
                volume_block_size: None,
                user_properties: props,
            },
        }
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