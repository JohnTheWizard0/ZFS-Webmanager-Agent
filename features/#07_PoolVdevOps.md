ACTIVE: #07_PoolVdevOps
STATUS: done
DEPENDS: #04_PoolCRUD
---

## Purpose
Vdev management: add and remove devices from pools.

## Endpoints
| Method | Path | Status | Description |
|--------|------|--------|-------------|
| DELETE | `/v1/pools/{name}/vdev/{device}` | done | Remove vdev |
| POST | `/v1/pools/{name}/vdev` | **done** | Add vdev |

## Remove Vdev (done)
Implementation: libzetta `ZpoolEngine::remove()`

Constraints:
- Cannot remove raidz/draid vdevs
- Can remove: mirrors, single disks, cache, log, spare

## Add Vdev (done - 2025-12-13)
Implementation: **Custom FFI** via `zpool_add()` with manual nvlist construction

Supported vdev types:
- `disk` - Single disk (stripe)
- `mirror` - Mirrored vdev (2+ disks)
- `raidz`, `raidz2`, `raidz3` - RAID-Z with 1/2/3 parity
- `log` - ZIL (ZFS Intent Log) device
- `cache` - L2ARC cache device
- `spare` - Hot spare
- `special` - Metadata/small block allocation class
- `dedup` - Deduplication table storage

Security validations:
- Pool name validation (must exist)
- Device path validation (absolute, no shell metacharacters)
- Vdev type whitelist validation
- RAII guards for memory management (nvlist, pool handle, libzfs handle)

## FFI Solution (Research Notes)

### Target C API
```c
int zpool_add(zpool_handle_t *zhp, nvlist_t *nvroot, boolean_t check_ashift)
```
- `nvroot`: vdev tree nvlist to add
- `check_ashift`: Warn if ashift mismatch
- Complexity: **Medium** — requires building nvlist vdev tree
- Reference: `_resources/ZFS_documentation/14_vdev_topology.txt:73-85`

### libzfs-sys Crate (v0.5.11)
**Finding**: `zpool_add()` is NOT exposed in libzfs-sys bindings

Available functions:
- `zpool_export`, `zpool_import`, `zpool_search_import`
- `zpool_get_config`, `zpool_get_prop`, `zpool_iter`
- `zpool_close`, `zpool_open_canfail`

### System libzfs.so (CONFIRMED 2025-12-13)
**Finding**: `zpool_add()` IS EXPORTED from system library!

```bash
$ nm -D /lib/x86_64-linux-gnu/libzfs.so | grep zpool_add
000000000002f330 T zpool_add
```

Also available (for future features):
- `zpool_vdev_attach`, `zpool_vdev_detach`, `zpool_vdev_remove`
- `zpool_vdev_offline`, `zpool_vdev_online`, `zpool_vdev_split`

### nvlist Building (nvpair-sys v0.4.0)
Available functions (already a dependency):
- `nvlist_alloc` - allocate nvlist
- `nvlist_add_string` - add type, path
- `nvlist_add_uint64` - add ashift, nparity
- `nvlist_add_nvlist_array` - add children array
- `nvlist_free` - cleanup

### ZPOOL_CONFIG Constants
Available in `/usr/include/libzfs/sys/fs/zfs.h`:
- `ZPOOL_CONFIG_TYPE` = "type"
- `ZPOOL_CONFIG_PATH` = "path"
- `ZPOOL_CONFIG_CHILDREN` = "children"
- `ZPOOL_CONFIG_NPARITY` = "nparity"
- `ZPOOL_CONFIG_ASHIFT` = "ashift"

### nvroot Structure for Add
```
root (type="root")
  └── children[]
       └── vdev (type="mirror"|"raidz"|"disk")
            ├── children[] (for mirror/raidz)
            │    └── disk (type="disk", path="/dev/...")
            └── [nparity for raidz]
```

### libzetta Analysis
**Finding**: libzetta `ZpoolEngine::add_vdev()` EXISTS but uses CLI wrapper
- Located: `libzetta-0.5.0/src/zpool/open3.rs:428-448`
- Executes: `zpool add [-f] <pool> <vdev-args>`
- Uses `CreateVdevRequest` enum for vdev specification

## Implementation Options Analysis

| Option | Pros | Cons |
|--------|------|------|
| **1. Custom FFI** | Native performance, proper error codes, consistent with import_pool | Requires manual nvlist building, more complex |
| **2. libzetta** | Already implemented, good error handling | CLI wrapper (not true FFI), libzetta dependency |
| **3. Direct CLI** | Simplest | Least integrated, error parsing fragile |

## RECOMMENDATION: Option 1 (Custom FFI)

**Rationale:**
1. `zpool_add()` confirmed available in system libzfs.so
2. nvpair-sys already a dependency with all needed functions
3. Consistent with existing FFI pattern (import_pool_with_name)
4. Proper error handling via libzfs error API
5. Future-proof for additional vdev ops (attach, detach, replace)

**Implementation Plan:**
1. Declare extern FFI binding for `zpool_add()`
2. Create `build_vdev_nvlist()` helper for nvlist construction
3. Add `add_vdev()` method to ZfsManager
4. Create handler and models for API endpoint

## Implementation Progress (2025-12-13)

### Completed
- [x] Research libzetta for add_vdev support
- [x] Research web for Rust ZFS FFI patterns
- [x] Decide implementation approach → **Custom FFI**
- [x] Implement FFI binding for zpool_add
- [x] Implement nvlist builder for vdev specs
- [x] Implement ZfsManager::add_vdev() method
- [x] Add models (AddVdevRequest, AddVdevResponse)
- [x] Support vdev types: single, mirror, raidz1/2/3
- [x] Support special vdevs: log, cache, spare, special, dedup
- [x] Add handler: POST /v1/pools/{name}/vdev
- [x] Update OpenAPI spec
- [x] Add integration tests (VD1-VD5 short, VD1-VD10 long)
- [x] Code compiles successfully (`cargo build`)

### Files Modified
| File | Changes |
|------|---------|
| `src/zfs_management.rs` | FFI bindings (`zpool_add`), nvlist builders (`build_disk_nvlist`, `build_vdev_nvlist`, `build_root_nvlist`), `add_vdev()` method, RAII guards (`NvlistGuard`, `PoolGuard`, `LibzfsGuard`) |
| `src/models.rs` | `AddVdevRequest`, `AddVdevResponse`, `default_true()` helper |
| `src/handlers.rs` | `add_vdev_handler()` |
| `src/main.rs` | Route: `POST /v1/pools/{name}/vdev` |
| `openapi.yaml` | Schemas + endpoint documentation |
| `tests/zfs_stress_b_short.sh` | VD1-VD5 error case tests |
| `tests/zfs_stress_b_long.sh` | VD1-VD10 comprehensive tests |

## Test Results (2025-12-13)

### Short Tests (`zfs_stress_b_short.sh`)
**Result: ALL PASS (5/5 vdev tests)**
- VD1: Pool does not exist ✓
- VD2: Invalid device path ✓
- VD3: Invalid vdev_type ✓
- VD4: Device already in pool ✓
- VD5: Pool status after errors ✓

### Long Tests (`zfs_stress_b_long.sh`)
**Result: 70 PASS, 1 FAIL (EI4 - pre-existing, unrelated)**

Vdev Tests (VD1-VD10) - ALL PASS:
- VD1: Add vdev to non-existent pool ✓
- VD2: Add vdev with invalid device path ✓
- VD3: Add vdev with invalid vdev_type ✓
- VD4: Add vdev with device already in pool ✓
- VD5: Verify pool status after vdev attempts ✓
- VD6: **Add mirror vdev (2 devices)** ✓ — Successfully added `/dev/loop0` + `/dev/loop1`
- VD7: **Add single disk vdev** ✓ — Successfully added `/dev/loop2`
- VD8: Add vdev with force=true ✓
- VD9: Add vdev with check_ashift=false ✓
- VD10: Add vdev with malformed JSON ✓

### Test Environment
- Loop devices used as spare disks for testing
- Created: `/dev/loop0-4` → `/tmp/zfs_test_disks/disk5-9.img` (100MB each)

## Commit Phase (on user request)
- [ ] Commit to develop branch
- [ ] Update version if needed
- [ ] Tag release after main merge
