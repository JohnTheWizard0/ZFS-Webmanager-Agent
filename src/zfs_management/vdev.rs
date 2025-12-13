// zfs_management/vdev.rs
// Vdev operations: add, remove + nvlist builders

use super::ffi::*;
use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::ZfsError;
use libzetta::zpool::ZpoolEngine;
use libzfs_sys::{libzfs_error_description, libzfs_init};
use nvpair_sys::{
    nvlist_alloc, nvlist_add_nvlist_array, nvlist_add_string, nvlist_add_uint64, nvlist_free,
    nvlist_t, NV_UNIQUE_NAME,
};
use std::ffi::CString;
use std::ptr;

impl ZfsManager {
    /// Build an nvlist for a single disk device
    fn build_disk_nvlist(path: &str) -> Result<*mut nvlist_t, ZfsError> {
        if !path.starts_with('/') {
            return Err(format!(
                "Invalid device path '{}': must be absolute path",
                path
            ));
        }
        if path.contains('\0') || path.contains(';') || path.contains('&') || path.contains('|') {
            return Err(format!(
                "Invalid device path '{}': contains forbidden characters",
                path
            ));
        }

        let c_type =
            CString::new("disk").map_err(|_| "Failed to create type CString".to_string())?;
        let c_path =
            CString::new(path).map_err(|_| format!("Invalid path '{}': contains null byte", path))?;

        unsafe {
            let mut nvl: *mut nvlist_t = ptr::null_mut();

            let ret = nvlist_alloc(&mut nvl, NV_UNIQUE_NAME, 0);
            if ret != 0 || nvl.is_null() {
                return Err(format!("Failed to allocate nvlist for disk: errno {}", ret));
            }

            let c_type_key = CString::new(ZPOOL_CONFIG_TYPE).unwrap();
            let ret = nvlist_add_string(nvl, c_type_key.as_ptr(), c_type.as_ptr());
            if ret != 0 {
                nvlist_free(nvl);
                return Err(format!("Failed to add type to disk nvlist: errno {}", ret));
            }

            let c_path_key = CString::new(ZPOOL_CONFIG_PATH).unwrap();
            let ret = nvlist_add_string(nvl, c_path_key.as_ptr(), c_path.as_ptr());
            if ret != 0 {
                nvlist_free(nvl);
                return Err(format!("Failed to add path to disk nvlist: errno {}", ret));
            }

            Ok(nvl)
        }
    }

    /// Build an nvlist for a vdev (mirror, raidz, or single disk)
    fn build_vdev_nvlist(
        vdev_type: &str,
        devices: &[String],
        nparity: Option<u8>,
    ) -> Result<*mut nvlist_t, ZfsError> {
        // Handle single disk case
        if vdev_type == "disk" {
            if devices.len() != 1 {
                return Err(format!(
                    "vdev_type 'disk' requires exactly 1 device, got {}",
                    devices.len()
                ));
            }
            return Self::build_disk_nvlist(&devices[0]);
        }

        // Handle special vdevs (log, cache, spare)
        if vdev_type == "log" || vdev_type == "cache" || vdev_type == "spare" {
            if devices.len() == 1 {
                return Self::build_disk_nvlist(&devices[0]);
            }
            return Self::build_vdev_nvlist("mirror", devices, None);
        }

        // Handle special allocation class vdevs
        if vdev_type == "special" || vdev_type == "dedup" {
            if devices.len() == 1 {
                return Self::build_disk_nvlist(&devices[0]);
            }
            return Self::build_vdev_nvlist("mirror", devices, None);
        }

        // Validate device count for redundancy vdevs
        let min_devices = match vdev_type {
            "mirror" => 2,
            "raidz" | "raidz1" => 2,
            "raidz2" => 3,
            "raidz3" => 4,
            _ => {
                return Err(format!("Unknown vdev type: {}", vdev_type));
            }
        };

        if devices.len() < min_devices {
            return Err(format!(
                "vdev_type '{}' requires at least {} devices, got {}",
                vdev_type,
                min_devices,
                devices.len()
            ));
        }

        // Determine actual type and nparity for raidz variants
        let (actual_type, actual_nparity) = match vdev_type {
            "mirror" => ("mirror", None),
            "raidz" | "raidz1" => ("raidz", Some(1u64)),
            "raidz2" => ("raidz", Some(2u64)),
            "raidz3" => ("raidz", Some(3u64)),
            _ => (vdev_type, nparity.map(|n| n as u64)),
        };

        let c_type = CString::new(actual_type)
            .map_err(|_| format!("Invalid vdev type: {}", actual_type))?;

        unsafe {
            let mut child_nvls: Vec<*mut nvlist_t> = Vec::with_capacity(devices.len());

            for device in devices {
                match Self::build_disk_nvlist(device) {
                    Ok(nvl) => child_nvls.push(nvl),
                    Err(e) => {
                        for nvl in child_nvls {
                            nvlist_free(nvl);
                        }
                        return Err(e);
                    }
                }
            }

            let mut nvl: *mut nvlist_t = ptr::null_mut();
            let ret = nvlist_alloc(&mut nvl, NV_UNIQUE_NAME, 0);
            if ret != 0 || nvl.is_null() {
                for child in child_nvls {
                    nvlist_free(child);
                }
                return Err(format!("Failed to allocate vdev nvlist: errno {}", ret));
            }

            let c_type_key = CString::new(ZPOOL_CONFIG_TYPE).unwrap();
            let ret = nvlist_add_string(nvl, c_type_key.as_ptr(), c_type.as_ptr());
            if ret != 0 {
                for child in child_nvls {
                    nvlist_free(child);
                }
                nvlist_free(nvl);
                return Err(format!("Failed to add type to vdev nvlist: errno {}", ret));
            }

            if let Some(parity) = actual_nparity {
                let c_nparity_key = CString::new(ZPOOL_CONFIG_NPARITY).unwrap();
                let ret = nvlist_add_uint64(nvl, c_nparity_key.as_ptr(), parity);
                if ret != 0 {
                    for child in child_nvls {
                        nvlist_free(child);
                    }
                    nvlist_free(nvl);
                    return Err(format!(
                        "Failed to add nparity to vdev nvlist: errno {}",
                        ret
                    ));
                }
            }

            let c_children_key = CString::new(ZPOOL_CONFIG_CHILDREN).unwrap();
            let ret = nvlist_add_nvlist_array(
                nvl,
                c_children_key.as_ptr(),
                child_nvls.as_mut_ptr(),
                child_nvls.len() as u32,
            );

            for child in child_nvls {
                nvlist_free(child);
            }

            if ret != 0 {
                nvlist_free(nvl);
                return Err(format!(
                    "Failed to add children to vdev nvlist: errno {}",
                    ret
                ));
            }

            Ok(nvl)
        }
    }

    /// Build the root nvlist for zpool_add()
    fn build_root_nvlist(
        child: *mut nvlist_t,
        vdev_type: &str,
    ) -> Result<*mut nvlist_t, ZfsError> {
        let c_root_type =
            CString::new("root").map_err(|_| "Failed to create root type CString".to_string())?;

        unsafe {
            let mut nvl: *mut nvlist_t = ptr::null_mut();

            let ret = nvlist_alloc(&mut nvl, NV_UNIQUE_NAME, 0);
            if ret != 0 || nvl.is_null() {
                return Err(format!("Failed to allocate root nvlist: errno {}", ret));
            }

            let c_type_key = CString::new(ZPOOL_CONFIG_TYPE).unwrap();
            let ret = nvlist_add_string(nvl, c_type_key.as_ptr(), c_root_type.as_ptr());
            if ret != 0 {
                nvlist_free(nvl);
                return Err(format!("Failed to add type to root nvlist: errno {}", ret));
            }

            let actual_child = if vdev_type == "log" || vdev_type == "cache" || vdev_type == "spare"
                || vdev_type == "special" || vdev_type == "dedup"
            {
                let mut wrapper: *mut nvlist_t = ptr::null_mut();
                let ret = nvlist_alloc(&mut wrapper, NV_UNIQUE_NAME, 0);
                if ret != 0 || wrapper.is_null() {
                    nvlist_free(nvl);
                    return Err(format!(
                        "Failed to allocate wrapper nvlist: errno {}",
                        ret
                    ));
                }

                let c_special_type = CString::new(vdev_type).unwrap();
                let ret = nvlist_add_string(wrapper, c_type_key.as_ptr(), c_special_type.as_ptr());
                if ret != 0 {
                    nvlist_free(wrapper);
                    nvlist_free(nvl);
                    return Err(format!(
                        "Failed to add type to wrapper nvlist: errno {}",
                        ret
                    ));
                }

                let c_children_key = CString::new(ZPOOL_CONFIG_CHILDREN).unwrap();
                let mut children: [*mut nvlist_t; 1] = [child];
                let ret = nvlist_add_nvlist_array(
                    wrapper,
                    c_children_key.as_ptr(),
                    children.as_mut_ptr(),
                    1,
                );
                if ret != 0 {
                    nvlist_free(wrapper);
                    nvlist_free(nvl);
                    return Err(format!(
                        "Failed to add children to wrapper nvlist: errno {}",
                        ret
                    ));
                }

                wrapper
            } else {
                child
            };

            let c_children_key = CString::new(ZPOOL_CONFIG_CHILDREN).unwrap();
            let mut children: [*mut nvlist_t; 1] = [actual_child];
            let ret =
                nvlist_add_nvlist_array(nvl, c_children_key.as_ptr(), children.as_mut_ptr(), 1);

            if ret != 0 {
                if actual_child != child {
                    nvlist_free(actual_child);
                }
                nvlist_free(nvl);
                return Err(format!(
                    "Failed to add children to root nvlist: errno {}",
                    ret
                ));
            }

            Ok(nvl)
        }
    }

    /// Add a vdev to an existing pool
    pub async fn add_vdev(
        &self,
        pool: &str,
        vdev_type: &str,
        devices: Vec<String>,
        force: bool,
        check_ashift: bool,
    ) -> Result<(), ZfsError> {
        if !ALLOWED_VDEV_TYPES.contains(&vdev_type) {
            return Err(format!(
                "Invalid vdev_type '{}'. Allowed: {:?}",
                vdev_type,
                ALLOWED_VDEV_TYPES
            ));
        }

        if devices.is_empty() {
            return Err("At least one device is required".to_string());
        }

        if !self
            .zpool_engine
            .exists(pool)
            .map_err(|e| format!("Failed to check pool existence: {}", e))?
        {
            return Err(format!("Pool '{}' does not exist", pool));
        }

        let c_pool = CString::new(pool)
            .map_err(|_| format!("Invalid pool name '{}': contains null byte", pool))?;

        let hdl = unsafe { libzfs_init() };
        if hdl.is_null() {
            return Err("Failed to initialize libzfs handle".to_string());
        }

        let _libzfs_guard = LibzfsGuard(hdl);

        let zhp = unsafe { zpool_open_canfail(hdl, c_pool.as_ptr()) };
        if zhp.is_null() {
            let err_desc = unsafe {
                let err_ptr = libzfs_error_description(hdl);
                if !err_ptr.is_null() {
                    std::ffi::CStr::from_ptr(err_ptr)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    "pool not found".to_string()
                }
            };
            return Err(format!("Failed to open pool '{}': {}", pool, err_desc));
        }

        let _pool_guard = PoolGuard(zhp);

        let vdev_nvl = Self::build_vdev_nvlist(vdev_type, &devices, None)?;
        let _vdev_guard = NvlistGuard(vdev_nvl);

        let root_nvl = Self::build_root_nvlist(vdev_nvl, vdev_type)?;
        let _root_guard = NvlistGuard(root_nvl);

        let _ = force; // Acknowledge force flag

        let result = unsafe { zpool_add(zhp, root_nvl, if check_ashift { 1 } else { 0 }) };

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
                "Failed to add {} vdev to pool '{}': {}",
                vdev_type, pool, err_desc
            ))
        }
    }

    /// Remove a vdev from an existing pool
    pub async fn remove_vdev(&self, pool: &str, device: &str) -> Result<(), ZfsError> {
        if !device.starts_with('/') && device.parse::<u64>().is_err() {
            return Err(format!(
                "Invalid device '{}': must be absolute path or GUID",
                device
            ));
        }

        if device.starts_with('/') {
            let dangerous_chars = [';', '|', '&', '$', '`', '(', ')', '{', '}', '[', ']', '<', '>'];
            if device.chars().any(|c| dangerous_chars.contains(&c)) {
                return Err(format!(
                    "Invalid device path '{}': contains dangerous characters",
                    device
                ));
            }
        }

        if !self
            .zpool_engine
            .exists(pool)
            .map_err(|e| format!("Failed to check pool existence: {}", e))?
        {
            return Err(format!("Pool '{}' does not exist", pool));
        }

        let c_pool = CString::new(pool)
            .map_err(|_| format!("Invalid pool name '{}': contains null byte", pool))?;
        let c_device = CString::new(device)
            .map_err(|_| format!("Invalid device '{}': contains null byte", device))?;

        let hdl = unsafe { libzfs_init() };
        if hdl.is_null() {
            return Err("Failed to initialize libzfs handle".to_string());
        }

        let _libzfs_guard = LibzfsGuard(hdl);

        let zhp = unsafe { zpool_open_canfail(hdl, c_pool.as_ptr()) };
        if zhp.is_null() {
            let err_desc = unsafe {
                let err_ptr = libzfs_error_description(hdl);
                if !err_ptr.is_null() {
                    std::ffi::CStr::from_ptr(err_ptr)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    "pool not found".to_string()
                }
            };
            return Err(format!("Failed to open pool '{}': {}", pool, err_desc));
        }

        let _pool_guard = PoolGuard(zhp);

        let result = unsafe { zpool_vdev_remove(zhp, c_device.as_ptr()) };

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
                "Failed to remove device '{}' from pool '{}': {}",
                device, pool, err_desc
            ))
        }
    }
}
