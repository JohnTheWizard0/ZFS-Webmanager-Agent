// zfs_management/manager.rs
// ZfsManager struct definition and constructor

use super::types::ZfsError;
use libzetta::zfs::DelegatingZfsEngine;
use libzetta::zpool::ZpoolOpen3;
use std::sync::Arc;

/// Main ZFS management interface
/// Wraps libzetta engines for pool and dataset operations
#[derive(Clone)]
pub struct ZfsManager {
    pub(crate) zpool_engine: Arc<ZpoolOpen3>,
    pub(crate) zfs_engine: Arc<DelegatingZfsEngine>,
}

impl ZfsManager {
    /// Create a new ZfsManager instance
    /// Initializes libzetta engines for pool and dataset operations
    pub fn new() -> Result<Self, ZfsError> {
        let zpool_engine = Arc::new(ZpoolOpen3::default());
        let zfs_engine = Arc::new(
            DelegatingZfsEngine::new()
                .map_err(|e| format!("Failed to initialize ZFS engine: {}", e))?,
        );

        Ok(ZfsManager {
            zpool_engine,
            zfs_engine,
        })
    }

    /// Extract pool name from a dataset/snapshot path
    pub fn get_pool_from_path(path: &str) -> String {
        path.split('/')
            .next()
            .unwrap_or(path)
            .split('@')
            .next()
            .unwrap_or(path)
            .to_string()
    }
}
