ACTIVE: #07_PoolVdevOps
STATUS: implementing
DEPENDS: #04_PoolCRUD
---

## Purpose
Vdev management: add and remove devices from pools.

## Endpoints
| Method | Path | Status | Description |
|--------|------|--------|-------------|
| DELETE | `/v1/pools/{name}/vdev/{device}` | done | Remove vdev |
| POST | `/v1/pools/{name}/vdev` | planned | Add vdev |

## Remove Vdev (done)
Implementation: libzetta `ZpoolEngine::remove()`

Constraints:
- Cannot remove raidz/draid vdevs
- Can remove: mirrors, single disks, cache, log, spare

## Add Vdev (planned)
Command: `zpool add <pool> <vdev-spec>`
Use case: Expand pool storage

## FFI Solution
`zpool_add()` available via libzfs:
```c
int zpool_add(zpool_handle_t *zhp, nvlist_t *nvroot, boolean_t check_ashift)
```
- `nvroot`: vdev tree nvlist to add
- `check_ashift`: Warn if ashift mismatch
- Complexity: **Medium** — requires building nvlist vdev tree
- Reference: `_resources/ZFS_documentation/14_vdev_topology.txt:73-85`

Building nvroot:
1. Create nvlist for each device (type, path)
2. Create parent nvlist (type=mirror/raidz) with children array
3. Create root nvlist with single child (the new vdev)

## TODO
- [ ] Implement add vdev endpoint via `zpool_add()` FFI
- [ ] Support vdev specs (mirror, raidz, etc.)
