// zfs_management/pools.rs
// Pool operations: list, status, create, destroy, export, import

use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::{ImportablePool, PoolStatus, ZfsError};
use libzetta::zpool::{CreateVdevRequest, CreateZpoolRequest, DestroyMode, ExportMode, ZpoolEngine};
use libzfs_sys::{
    import_args, libzfs_error_description, libzfs_init, zpool_import, zpool_search_import,
};
use nvpair_sys::nvlist_lookup_nvlist;
use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;

impl ZfsManager {
    pub async fn list_pools(&self) -> Result<Vec<String>, ZfsError> {
        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpools = self
            .zpool_engine
            .status_all(status_options)
            .map_err(|e| format!("Failed to list pools: {}", e))?;

        let pool_names = zpools
            .into_iter()
            .map(|zpool| zpool.name().clone())
            .collect();

        Ok(pool_names)
    }

    pub async fn get_pool_status(&self, name: &str) -> Result<PoolStatus, ZfsError> {
        // Guard against libzetta panic: check pool exists before calling status()
        if !self
            .zpool_engine
            .exists(name)
            .map_err(|e| format!("Failed to check pool existence: {}", e))?
        {
            return Err(format!("Pool '{}' not found", name));
        }

        let status_options = libzetta::zpool::open3::StatusOptions::default();
        let zpool = self
            .zpool_engine
            .status(name, status_options)
            .map_err(|e| format!("Failed to get pool status: {}", e))?;

        let properties = self
            .zpool_engine
            .read_properties(name)
            .map_err(|e| format!("Failed to read pool properties: {}", e))?;

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

    pub async fn create_pool(&self, pool: crate::models::CreatePool) -> Result<(), ZfsError> {
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

        self.zpool_engine
            .create(request)
            .map_err(|e| format!("Failed to create pool: {}", e))?;

        Ok(())
    }

    pub async fn destroy_pool(&self, name: &str, force: bool) -> Result<(), ZfsError> {
        let pools = self
            .zpool_engine
            .available()
            .map_err(|e| format!("Failed to list pools: {}", e))?;

        if !pools.iter().any(|p| p.name() == name) {
            return Err(format!("Pool '{}' does not exist", name));
        }

        let mode = if force {
            DestroyMode::Force
        } else {
            DestroyMode::Gentle
        };

        self.zpool_engine
            .destroy(name, mode)
            .map_err(|e| format!("Failed to destroy pool: {}", e))?;

        Ok(())
    }

    // =========================================================================
    // Pool Import/Export Operations
    // =========================================================================

    /// Export a pool from the system
    pub async fn export_pool(&self, name: &str, force: bool) -> Result<(), ZfsError> {
        let mode = if force {
            ExportMode::Force
        } else {
            ExportMode::Gentle
        };

        self.zpool_engine
            .export(name, mode)
            .map_err(|e| format!("Failed to export pool: {}", e))?;

        Ok(())
    }

    /// List pools available for import from /dev/
    pub async fn list_importable_pools(&self) -> Result<Vec<ImportablePool>, ZfsError> {
        let pools = self
            .zpool_engine
            .available()
            .map_err(|e| format!("Failed to list importable pools: {}", e))?;

        Ok(pools
            .into_iter()
            .map(|p| ImportablePool {
                name: p.name().clone(),
                health: format!("{:?}", p.health()),
            })
            .collect())
    }

    /// List pools available for import from a specific directory
    pub async fn list_importable_pools_from_dir(
        &self,
        dir: &str,
    ) -> Result<Vec<ImportablePool>, ZfsError> {
        let pools = self
            .zpool_engine
            .available_in_dir(PathBuf::from(dir))
            .map_err(|e| format!("Failed to list importable pools from {}: {}", dir, e))?;

        Ok(pools
            .into_iter()
            .map(|p| ImportablePool {
                name: p.name().clone(),
                health: format!("{:?}", p.health()),
            })
            .collect())
    }

    /// Import a pool from /dev/
    pub async fn import_pool(&self, name: &str) -> Result<(), ZfsError> {
        self.zpool_engine
            .import(name)
            .map_err(|e| format!("Failed to import pool: {}", e))?;

        Ok(())
    }

    /// Import a pool from a specific directory
    pub async fn import_pool_from_dir(&self, name: &str, dir: &str) -> Result<(), ZfsError> {
        self.zpool_engine
            .import_from_dir(name, PathBuf::from(dir))
            .map_err(|e| format!("Failed to import pool from {}: {}", dir, e))?;

        Ok(())
    }

    /// Import a pool with a new name (rename on import)
    /// FFI implementation using libzfs-sys zpool_import()
    pub async fn import_pool_with_name(
        &self,
        name: &str,
        new_name: &str,
        dir: Option<&str>,
    ) -> Result<(), ZfsError> {
        let c_poolname = CString::new(name)
            .map_err(|_| format!("Invalid pool name '{}': contains null byte", name))?;
        let c_newname = CString::new(new_name)
            .map_err(|_| format!("Invalid new name '{}': contains null byte", new_name))?;

        let c_dir = dir
            .map(|d| {
                CString::new(d)
                    .map_err(|_| format!("Invalid directory '{}': contains null byte", d))
            })
            .transpose()?;

        let hdl = unsafe { libzfs_init() };
        if hdl.is_null() {
            return Err("Failed to initialize libzfs handle".to_string());
        }

        // RAII guard for cleanup
        struct HandleGuard(*mut libzfs_sys::libzfs_handle_t);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                unsafe { libzfs_sys::libzfs_fini(self.0) }
            }
        }
        let _guard = HandleGuard(hdl);

        let mut args = import_args();
        args.poolname = c_poolname.as_ptr() as *mut _;

        let mut dir_ptr: *mut i8 = c_dir
            .as_ref()
            .map(|d| d.as_ptr() as *mut i8)
            .unwrap_or(ptr::null_mut());
        if c_dir.is_some() {
            args.path = &mut dir_ptr as *mut *mut _;
            args.paths = 1;
        }

        let pools_nvl = unsafe { zpool_search_import(hdl, &mut args) };

        if pools_nvl.is_null() {
            return Err(format!(
                "Pool '{}' not found for import{}",
                name,
                dir.map(|d| format!(" in directory '{}'", d))
                    .unwrap_or_default()
            ));
        }

        let mut config_ptr: *mut nvpair_sys::nvlist_t = ptr::null_mut();
        let lookup_result = unsafe {
            nvlist_lookup_nvlist(pools_nvl, c_poolname.as_ptr(), &mut config_ptr)
        };

        if lookup_result != 0 || config_ptr.is_null() {
            return Err(format!(
                "Pool '{}' not found in importable pools (may already be imported)",
                name
            ));
        }

        let result = unsafe {
            zpool_import(
                hdl,
                config_ptr,
                c_newname.as_ptr(),
                ptr::null_mut(),
            )
        };

        if result == 0 {
            Ok(())
        } else {
            let err_desc = unsafe {
                let err_ptr = libzfs_error_description(hdl);
                if !err_ptr.is_null() {
                    std::ffi::CStr::from_ptr(err_ptr)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    errno_to_string(result).to_string()
                }
            };
            Err(format!(
                "Failed to import pool '{}' as '{}': {}",
                name, new_name, err_desc
            ))
        }
    }
}
