//! # Integration Tests: Dataset Operations
//!
//! **Modules:** MF-002 (Dataset Operations)
//! **Dependencies:** MI-001, MI-002, MC-001, MF-001 (needs pool to exist)
//!
//! ## What is tested
//! - GET /datasets/{pool} → list datasets in pool
//! - POST /datasets → create new dataset
//! - DELETE /datasets/{name} → delete dataset
//!
//! ## Expected outcome
//! - Datasets created within existing pools
//! - Nested dataset paths work (tank/data/subdir)
//! - Properties applied on creation
//!
//! ## Prerequisites
//! - ZFS installed
//! - At least one pool exists
//!
//! ## BLOCKED
//! - Cannot run until crate compiles (zfs_management.rs bugs)
//! - Specifically blocked by create_dataset bugs (lines 135, 144-146)

/*
use serde_json::json;

#[tokio::test]
async fn test_list_datasets_empty_pool() {
    // GET /datasets/tank when pool has no child datasets
    // Expected: {"status": "success", "datasets": ["tank"]}
}

#[tokio::test]
async fn test_list_datasets_with_children() {
    // GET /datasets/tank
    // Expected: ["tank", "tank/data", "tank/backup"]
}

#[tokio::test]
async fn test_create_dataset_filesystem() {
    // POST /datasets {"name": "tank/test", "kind": "filesystem"}
    // Expected: success
}

#[tokio::test]
async fn test_create_dataset_volume() {
    // POST /datasets {"name": "tank/vol", "kind": "volume"}
    // Note: Volumes may need additional properties (size)
}

#[tokio::test]
async fn test_create_dataset_with_properties() {
    // POST /datasets {"name": "tank/compressed", "kind": "filesystem",
    //                 "properties": {"compression": "lz4"}}
}

#[tokio::test]
async fn test_create_dataset_invalid_kind() {
    // POST /datasets {"name": "tank/test", "kind": "invalid"}
    // Expected: error - must be filesystem or volume
}

#[tokio::test]
async fn test_delete_dataset() {
    // DELETE /datasets/tank/test
}

#[tokio::test]
async fn test_delete_dataset_nested_path() {
    // DELETE /datasets/tank/data/subdir
    // Tests that path tail matching works
}
*/

#[test]
#[ignore = "Integration test requires running server + ZFS - use zfs_parcour.sh"]
fn placeholder_dataset_tests() {
    // Real dataset integration tests run via tests/zfs_parcour.sh
}
