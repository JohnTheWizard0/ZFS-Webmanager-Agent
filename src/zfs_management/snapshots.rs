// zfs_management/snapshots.rs
// Snapshot operations: list, create, delete, clone, promote, rollback

use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::{RollbackError, RollbackResult, ZfsError};
use libzetta::zfs::ZfsEngine;
use libzetta_zfs_core_sys::{lzc_clone, lzc_promote, lzc_rollback_to};
use libzfs::Libzfs;
use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;

impl ZfsManager {
    pub async fn list_snapshots(&self, dataset: &str) -> Result<Vec<String>, ZfsError> {
        let snapshots = self
            .zfs_engine
            .list_snapshots(dataset)
            .map_err(|e| format!("Failed to list snapshots: {}", e))?;

        Ok(snapshots
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect())
    }

    pub async fn create_snapshot(
        &self,
        dataset: &str,
        snapshot_name: &str,
    ) -> Result<(), ZfsError> {
        let snapshot_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));

        self.zfs_engine
            .snapshot(&[snapshot_path], None)
            .map_err(|e| format!("Failed to create snapshot: {}", e))?;

        Ok(())
    }

    pub async fn delete_snapshot(
        &self,
        dataset: &str,
        snapshot_name: &str,
    ) -> Result<(), ZfsError> {
        let full_snapshot_name = format!("{}@{}", dataset, snapshot_name);

        let existing_snapshots = self.list_snapshots(dataset).await?;
        if !existing_snapshots.contains(&full_snapshot_name) {
            return Err(format!("Snapshot '{}' does not exist", full_snapshot_name));
        }

        let snapshot_path = PathBuf::from(&full_snapshot_name);
        self.zfs_engine
            .destroy_snapshots(&[snapshot_path], libzetta::zfs::DestroyTiming::RightNow)
            .map_err(|e| format!("Failed to delete snapshot: {}", e))?;

        Ok(())
    }

    // =========================================================================
    // Snapshot Clone/Promote Operations
    // =========================================================================

    /// Clone a snapshot to create a new writable dataset
    pub async fn clone_snapshot(&self, snapshot: &str, target: &str) -> Result<(), ZfsError> {
        if !snapshot.contains('@') {
            return Err(format!(
                "Invalid snapshot path '{}': must be dataset@snapshot",
                snapshot
            ));
        }

        if target.contains('@') {
            return Err(format!(
                "Invalid target '{}': clone target must be a dataset path, not a snapshot",
                target
            ));
        }

        if !self
            .zfs_engine
            .exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))?
        {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        let c_target =
            CString::new(target).map_err(|_| "Invalid target path: contains null byte")?;
        let c_origin =
            CString::new(snapshot).map_err(|_| "Invalid snapshot path: contains null byte")?;

        let result = unsafe {
            lzc_clone(
                c_target.as_ptr(),
                c_origin.as_ptr(),
                ptr::null_mut(),
            )
        };

        if result == 0 {
            Ok(())
        } else {
            Err(format!(
                "lzc_clone failed with error code {}: {}",
                result,
                errno_to_string(result)
            ))
        }
    }

    /// Promote a clone to an independent dataset
    pub async fn promote_dataset(&self, clone_path: &str) -> Result<(), ZfsError> {
        if clone_path.contains('@') {
            return Err(format!(
                "Invalid path '{}': cannot promote a snapshot",
                clone_path
            ));
        }

        if !self
            .zfs_engine
            .exists(PathBuf::from(clone_path))
            .map_err(|e| format!("Failed to check dataset: {}", e))?
        {
            return Err(format!("Dataset '{}' does not exist", clone_path));
        }

        let c_path = CString::new(clone_path).map_err(|_| "Invalid path: contains null byte")?;

        let mut conflict_buf: [i8; 256] = [0; 256];

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
            let conflict_name = unsafe {
                std::ffi::CStr::from_ptr(conflict_buf.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };
            if conflict_name.is_empty() {
                Err("Promote failed: snapshot name collision (EEXIST)".to_string())
            } else {
                Err(format!(
                    "Promote failed: snapshot name collision with '{}'",
                    conflict_name
                ))
            }
        } else if result == libc::EINVAL {
            Err(format!(
                "Dataset '{}' is not a clone (no origin property)",
                clone_path
            ))
        } else {
            Err(format!(
                "lzc_promote failed with error code {}: {}",
                result,
                errno_to_string(result)
            ))
        }
    }

    // =========================================================================
    // Rollback Operations
    // =========================================================================

    /// Rollback a dataset to a snapshot
    pub async fn rollback_dataset(
        &self,
        dataset: &str,
        snapshot: &str,
        force_destroy_newer: bool,
        force_destroy_clones: bool,
    ) -> Result<RollbackResult, RollbackError> {
        if force_destroy_clones && !force_destroy_newer {
            return Err(RollbackError::InvalidRequest(
                "force_destroy_clones requires force_destroy_newer to be true".to_string(),
            ));
        }

        if !self
            .zfs_engine
            .exists(PathBuf::from(dataset))
            .map_err(|e| RollbackError::ZfsError(format!("Failed to check dataset: {}", e)))?
        {
            return Err(RollbackError::ZfsError(format!(
                "Dataset '{}' does not exist",
                dataset
            )));
        }

        let full_snapshot = format!("{}@{}", dataset, snapshot);

        if !self
            .zfs_engine
            .exists(PathBuf::from(&full_snapshot))
            .map_err(|e| RollbackError::ZfsError(format!("Failed to check snapshot: {}", e)))?
        {
            return Err(RollbackError::ZfsError(format!(
                "Snapshot '{}' does not exist",
                full_snapshot
            )));
        }

        let all_snapshots = self
            .list_snapshots(dataset)
            .await
            .map_err(RollbackError::ZfsError)?;

        let target_idx = all_snapshots
            .iter()
            .position(|s| s == &full_snapshot)
            .ok_or_else(|| {
                RollbackError::ZfsError(format!("Snapshot '{}' not found in list", full_snapshot))
            })?;

        let newer_snapshots: Vec<String> = all_snapshots[target_idx + 1..].to_vec();

        if !newer_snapshots.is_empty() && !force_destroy_newer {
            return Err(RollbackError::Blocked {
                message: format!(
                    "Cannot rollback to '{}': {} newer snapshot(s) exist",
                    full_snapshot,
                    newer_snapshots.len()
                ),
                blocking_snapshots: newer_snapshots,
                blocking_clones: vec![],
            });
        }

        let mut blocking_clones: Vec<String> = Vec::new();
        let mut clones_to_destroy: Vec<String> = Vec::new();

        if !newer_snapshots.is_empty() {
            let mut libzfs = Libzfs::new();

            for snap_path in &newer_snapshots {
                if let Ok(props) = self.zfs_engine.read_properties(PathBuf::from(snap_path)) {
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

                if let Some(ds) = libzfs.dataset_by_name(snap_path) {
                    let _ = ds;
                }
            }
        }

        if !blocking_clones.is_empty() {
            return Err(RollbackError::Blocked {
                message: format!(
                    "Cannot rollback: {} clone(s) depend on newer snapshots",
                    blocking_clones.len()
                ),
                blocking_snapshots: newer_snapshots,
                blocking_clones,
            });
        }

        let mut destroyed_clones: Vec<String> = Vec::new();
        let mut destroyed_snapshots: Vec<String> = Vec::new();

        for clone_path in clones_to_destroy {
            self.delete_dataset(&clone_path).await.map_err(|e| {
                RollbackError::ZfsError(format!("Failed to destroy clone '{}': {}", clone_path, e))
            })?;
            destroyed_clones.push(clone_path);
        }

        if force_destroy_newer {
            for snap_path in newer_snapshots.iter().rev() {
                if let Some(at_pos) = snap_path.rfind('@') {
                    let ds = &snap_path[..at_pos];
                    let snap_name = &snap_path[at_pos + 1..];
                    self.delete_snapshot(ds, snap_name).await.map_err(|e| {
                        RollbackError::ZfsError(format!(
                            "Failed to destroy snapshot '{}': {}",
                            snap_path, e
                        ))
                    })?;
                    destroyed_snapshots.push(snap_path.clone());
                }
            }
        }

        let c_fsname = CString::new(dataset).map_err(|_| {
            RollbackError::ZfsError("Invalid dataset path: contains null byte".to_string())
        })?;
        let c_snapname = CString::new(&full_snapshot as &str).map_err(|_| {
            RollbackError::ZfsError("Invalid snapshot path: contains null byte".to_string())
        })?;

        let result = unsafe { lzc_rollback_to(c_fsname.as_ptr(), c_snapname.as_ptr()) };

        if result == 0 {
            Ok(RollbackResult {
                destroyed_snapshots: if destroyed_snapshots.is_empty() {
                    None
                } else {
                    Some(destroyed_snapshots)
                },
                destroyed_clones: if destroyed_clones.is_empty() {
                    None
                } else {
                    Some(destroyed_clones)
                },
            })
        } else if result == libc::EEXIST {
            Err(RollbackError::Blocked {
                message: "Rollback failed: newer snapshots still exist (EEXIST)".to_string(),
                blocking_snapshots: vec![],
                blocking_clones: vec![],
            })
        } else if result == libc::EBUSY {
            Err(RollbackError::ZfsError(format!(
                "Dataset '{}' is busy (mounted with open files or active operations)",
                dataset
            )))
        } else {
            Err(RollbackError::ZfsError(format!(
                "lzc_rollback_to failed with error code {}: {}",
                result,
                errno_to_string(result)
            )))
        }
    }

    // delete_dataset is in datasets.rs, but we need it for rollback
    // It's accessible via self since it's an impl on ZfsManager
}
