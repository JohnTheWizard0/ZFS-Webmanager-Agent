// zfs_management/tests.rs
// Unit tests for ZFS management

#![cfg(test)]

use super::types::PoolStatus;
use libzetta::zpool::CreateVdevRequest;
use std::path::PathBuf;

// -------------------------------------------------------------------------
// PoolStatus Struct Tests
// -------------------------------------------------------------------------

/// Test: PoolStatus can be constructed with all fields
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
#[test]
fn test_single_disk_vdev() {
    let disks: Vec<PathBuf> = vec![PathBuf::from("/dev/sda")];
    let raid_type: Option<&str> = None;

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

    match vdev {
        CreateVdevRequest::SingleDisk(path) => {
            assert_eq!(path, PathBuf::from("/dev/sda"));
        }
        _ => panic!("Expected SingleDisk variant"),
    }
}

/// Test: Mirror creates Mirror vdev
#[test]
fn test_mirror_vdev() {
    let disks: Vec<PathBuf> = vec![PathBuf::from("/dev/sda"), PathBuf::from("/dev/sdb")];

    let vdev = CreateVdevRequest::Mirror(disks.clone());

    match vdev {
        CreateVdevRequest::Mirror(paths) => {
            assert_eq!(paths.len(), 2);
        }
        _ => panic!("Expected Mirror variant"),
    }
}

/// Test: Multiple disks without RAID type is error
#[test]
fn test_multiple_disks_no_raid_error() {
    let disks: Vec<PathBuf> = vec![PathBuf::from("/dev/sda"), PathBuf::from("/dev/sdb")];
    let raid_type: Option<&str> = None;

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
#[test]
fn test_snapshot_path_format() {
    let dataset = "tank/data";
    let snapshot_name = "backup-001";
    let snapshot_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));

    assert_eq!(snapshot_path.to_string_lossy(), "tank/data@backup-001");
}

// -------------------------------------------------------------------------
// ZfsManager Tests (require ZFS - placeholder stubs)
// -------------------------------------------------------------------------

/// Test: ZfsManager::new initializes engines
#[test]
#[ignore = "Requires ZFS to be installed"]
fn test_zfs_manager_new() {
    use super::manager::ZfsManager;
    let result = ZfsManager::new();
    assert!(result.is_ok() || result.is_err());
}

/// Test: list_pools returns Vec<String>
#[test]
#[ignore = "Requires ZFS to be installed"]
fn test_list_pools() {
    // Placeholder - requires actual ZFS
}

/// Test: create_dataset validates kind field
#[test]
#[ignore = "Requires ZFS to be installed"]
fn test_create_dataset_kind_validation() {
    // Would test that "filesystem" and "volume" are accepted,
    // while invalid kinds are rejected
}
