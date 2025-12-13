// zfs_management/datasets.rs
// Dataset operations: list, create, delete, properties

use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::{DatasetProperties, ZfsError};
use libzetta::zfs::{CreateDatasetRequest, DatasetKind, ZfsEngine};
use libzetta_zfs_core_sys::lzc_destroy;
use std::ffi::CString;
use std::path::PathBuf;

impl ZfsManager {
    pub async fn list_datasets(&self, pool: &str) -> Result<Vec<String>, ZfsError> {
        let datasets = self
            .zfs_engine
            .list_filesystems(pool)
            .map_err(|e| format!("Failed to list datasets: {}", e))?;

        Ok(datasets
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect())
    }

    pub async fn create_dataset(
        &self,
        dataset: crate::models::CreateDataset,
    ) -> Result<(), ZfsError> {
        let kind = match dataset.kind.as_str() {
            "filesystem" => DatasetKind::Filesystem,
            "volume" => DatasetKind::Volume,
            _ => return Err("Invalid dataset kind. Must be 'filesystem' or 'volume'".to_string()),
        };

        let crate::models::CreateDataset {
            name, properties, ..
        } = dataset;

        let request = CreateDatasetRequest::builder()
            .name(PathBuf::from(&name))
            .kind(kind)
            .user_properties(properties)
            .build()
            .map_err(|e| format!("Failed to build dataset request: {}", e))?;

        self.zfs_engine
            .create(request)
            .map_err(|e| format!("Failed to create dataset: {}", e))?;

        Ok(())
    }

    pub async fn delete_dataset(&self, name: &str) -> Result<(), ZfsError> {
        self.zfs_engine
            .destroy(PathBuf::from(name))
            .map_err(|e| format!("Failed to delete dataset: {}", e))?;

        Ok(())
    }

    /// Recursively delete a dataset and all its children/snapshots
    /// Implementation via libzetta-zfs-core-sys FFI (lzc_destroy)
    pub async fn delete_dataset_recursive(&self, name: &str) -> Result<(), ZfsError> {
        let pool = name
            .split('/')
            .next()
            .ok_or_else(|| "Invalid dataset path: no pool".to_string())?;

        let all_items = self
            .zfs_engine
            .list(PathBuf::from(pool))
            .map_err(|e| format!("Failed to list datasets: {}", e))?;

        let child_prefix = format!("{}/", name);
        let snap_prefix = format!("{}@", name);

        let mut to_delete: Vec<String> = all_items
            .into_iter()
            .map(|(_, path)| path.to_string_lossy().to_string())
            .filter(|p| p == name || p.starts_with(&child_prefix) || p.starts_with(&snap_prefix))
            .collect();

        // Sort by depth descending (deepest first)
        to_delete.sort_by(|a, b| {
            let depth_a = a.matches('/').count() + a.matches('@').count();
            let depth_b = b.matches('/').count() + b.matches('@').count();
            depth_b.cmp(&depth_a)
        });

        for item in &to_delete {
            let c_name = CString::new(item.as_str())
                .map_err(|_| format!("Invalid path: contains null byte: {}", item))?;

            let result = unsafe { lzc_destroy(c_name.as_ptr()) };

            if result != 0 {
                let err_msg = errno_to_string(result);
                return Err(format!(
                    "Failed to destroy '{}': {} (errno {})",
                    item, err_msg, result
                ));
            }
        }

        Ok(())
    }

    // =========================================================================
    // Dataset Properties Operations
    // =========================================================================

    /// Get all properties of a dataset (filesystem, volume, or snapshot)
    pub async fn get_dataset_properties(&self, name: &str) -> Result<DatasetProperties, ZfsError> {
        let props = self
            .zfs_engine
            .read_properties(PathBuf::from(name))
            .map_err(|e| format!("Failed to get dataset properties: {}", e))?;

        Ok(DatasetProperties::from_libzetta(name.to_string(), props))
    }

    /// Set a property on a dataset
    /// **EXPERIMENTAL**: Uses CLI (`zfs set`) as libzetta/libzfs FFI lacks property setting.
    pub async fn set_dataset_property(
        &self,
        name: &str,
        property: &str,
        value: &str,
    ) -> Result<(), ZfsError> {
        if !Self::is_valid_property_name(property) {
            return Err(format!("Invalid property name: {}", property));
        }

        if !self
            .zfs_engine
            .exists(PathBuf::from(name))
            .map_err(|e| format!("Failed to check dataset: {}", e))?
        {
            return Err(format!("Dataset '{}' does not exist", name));
        }

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
    fn is_valid_property_name(name: &str) -> bool {
        if name.is_empty() || name.len() > 256 {
            return false;
        }
        let first = name.chars().next().unwrap();
        if !first.is_ascii_lowercase() {
            return false;
        }
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == ':')
    }
}
