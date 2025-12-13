// zfs_management/ffi.rs
// FFI declarations and RAII guards for ZFS operations

use nvpair_sys::nvlist_t;

// ============================================================================
// FFI Declarations for zpool_add and zpool_open_canfail
// ============================================================================
// These functions are NOT exposed by libzfs-sys but ARE exported by system libzfs.so
// Verified via: nm -D /lib/x86_64-linux-gnu/libzfs.so | grep -E "zpool_add|zpool_open_canfail"

/// Opaque handle to a ZFS pool (libzfs)
#[repr(C)]
pub struct zpool_handle_t {
    _private: [u8; 0],
}

#[link(name = "zfs")]
extern "C" {
    /// Open a pool by name, returning NULL on failure (no error printed)
    /// ```c
    /// zpool_handle_t *zpool_open_canfail(libzfs_handle_t *, const char *);
    /// ```
    pub fn zpool_open_canfail(
        hdl: *mut libzfs_sys::libzfs_handle_t,
        name: *const std::ffi::c_char,
    ) -> *mut zpool_handle_t;

    /// Close a pool handle
    /// ```c
    /// void zpool_close(zpool_handle_t *);
    /// ```
    pub fn zpool_close(zhp: *mut zpool_handle_t);

    /// Add vdevs to an existing pool
    /// ```c
    /// int zpool_add(zpool_handle_t *zhp, nvlist_t *nvroot, boolean_t check_ashift);
    /// ```
    /// - nvroot: Root nvlist containing ZPOOL_CONFIG_CHILDREN with the new vdev(s)
    /// - check_ashift: If true, warn when adding vdevs with different ashift
    /// - Returns: 0 on success, non-zero on error
    pub fn zpool_add(
        zhp: *mut zpool_handle_t,
        nvroot: *mut nvlist_t,
        check_ashift: i32,
    ) -> std::ffi::c_int;

    /// Remove a vdev from an existing pool
    /// ```c
    /// int zpool_vdev_remove(zpool_handle_t *zhp, const char *path);
    /// ```
    /// - path: Device path or GUID of vdev to remove
    /// - Returns: 0 on success, non-zero on error
    /// - Note: Cannot remove raidz/draid vdevs (ZFS limitation)
    /// - Can remove: mirrors, single disks, cache, log, spare
    pub fn zpool_vdev_remove(
        zhp: *mut zpool_handle_t,
        path: *const std::ffi::c_char,
    ) -> std::ffi::c_int;
}

// ============================================================================
// RAII Guards for resource cleanup
// ============================================================================

/// RAII guard for libzfs handle - calls libzfs_fini() on drop
pub struct LibzfsGuard(pub *mut libzfs_sys::libzfs_handle_t);

impl Drop for LibzfsGuard {
    fn drop(&mut self) {
        unsafe { libzfs_sys::libzfs_fini(self.0) }
    }
}

/// RAII guard for zpool handle - calls zpool_close() on drop
pub struct PoolGuard(pub *mut zpool_handle_t);

impl Drop for PoolGuard {
    fn drop(&mut self) {
        unsafe { zpool_close(self.0) }
    }
}

/// RAII guard for nvlist - calls nvlist_free() on drop
pub struct NvlistGuard(pub *mut nvlist_t);

impl Drop for NvlistGuard {
    fn drop(&mut self) {
        unsafe { nvpair_sys::nvlist_free(self.0) }
    }
}

// ============================================================================
// ZPOOL_CONFIG constants for nvlist building
// ============================================================================

/// Reference: /usr/include/libzfs/sys/fs/zfs.h
pub const ZPOOL_CONFIG_TYPE: &str = "type";
pub const ZPOOL_CONFIG_PATH: &str = "path";
pub const ZPOOL_CONFIG_CHILDREN: &str = "children";
pub const ZPOOL_CONFIG_NPARITY: &str = "nparity";

/// Allowed vdev types for validation
/// Data vdevs: disk, mirror, raidz, raidz2, raidz3
/// Special vdevs: log, cache, spare, special, dedup
pub const ALLOWED_VDEV_TYPES: &[&str] = &[
    "disk", "mirror", "raidz", "raidz1", "raidz2", "raidz3", "log", "cache", "spare",
    "special", "dedup",
];
