ACTIVE: #06_PoolImportExport
STATUS: stable
DEPENDS: #04_PoolCRUD
---

## Purpose
Pool import/export operations.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/pools/{name}/export` | Export pool |
| POST | `/v1/pools/import` | Import pool |
| GET | `/v1/pools/importable` | List importable pools |

## Implementation
- Standard import/export: libzetta
- Import with rename: **FFI** (libzfs-sys `zpool_import()`)

## Import Options
- `name` — Pool name to import
- `new_name` — Optional rename (uses FFI)
- `dir` — Optional device directory to scan

## FFI Implementation (Completed)
Import with rename now uses native FFI via `libzfs-sys`:

```c
int zpool_import(libzfs_handle_t *hdl, nvlist_t *config,
                 const char *newname, char *altroot)
```

### Flow
1. `libzfs_init()` — create handle
2. `import_args()` — set poolname filter (and optional dir)
3. `zpool_search_import()` — find pool config
4. `nvlist_lookup_nvlist()` — extract pool config by name
5. `zpool_import(..., newname, ...)` — import with new name
6. `libzfs_fini()` — cleanup (via RAII guard)

### Dependencies Added
- `libzfs-sys = "0.5.11"` — FFI bindings
- `nvpair-sys = "0.1"` — nvlist lookup

### Reference
`_resources/ZFS_documentation/09_import_export.txt:6-10`
