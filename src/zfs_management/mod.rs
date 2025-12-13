// zfs_management/mod.rs
// Re-exports for backward compatibility

mod datasets;
mod ffi;
mod helpers;
mod manager;
mod pools;
mod replication;
mod scrub;
mod snapshots;
mod types;
mod vdev;

#[cfg(test)]
mod tests;

// Re-export main interface
pub use manager::ZfsManager;

// Re-export types used by handlers
pub use types::{DatasetProperties, RollbackError};
// RollbackResult is returned by methods but not directly used by handlers
