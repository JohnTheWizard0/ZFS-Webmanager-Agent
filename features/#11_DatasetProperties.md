ACTIVE: #11_DatasetProperties
STATUS: implementing
DEPENDS: #10_DatasetCRUD
---

## Purpose
Get and set dataset properties.

## Endpoints
| Method | Path | Implementation |
|--------|------|----------------|
| GET | `/v1/datasets/{path}/properties` | libzetta |
| PUT | `/v1/datasets/{path}/properties` | **CLI EXPERIMENTAL** |

## Common Properties
| Property | Read | Write |
|----------|------|-------|
| quota | yes | yes |
| reservation | yes | yes |
| compression | yes | yes |
| mountpoint | yes | yes |
| readonly | yes | yes |
| atime | yes | yes |

## CLI Fallback
SET uses `zfs set <prop>=<value> <dataset>` because:
- libzetta doesn't expose property setting
- libzfs FFI lacks `zfs_prop_set()` bindings

Property names validated against safe patterns to prevent injection.

## FFI Solution
`zfs_prop_set()` available via libzfs:
```c
int zfs_prop_set(zfs_handle_t *zhp, const char *propname, const char *value)
int zfs_prop_set_list(zfs_handle_t *zhp, nvlist_t *props)  // multiple
```
- Complexity: **Low** — direct string parameters
- Reference: `_resources/ZFS_documentation/10_properties.txt:61-62`

## TODO
- [ ] Implement `zfs_prop_set()` FFI binding
- [ ] Remove CLI dependency
