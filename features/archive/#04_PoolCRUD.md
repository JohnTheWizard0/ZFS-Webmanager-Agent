ACTIVE: #04_PoolCRUD
STATUS: stable
DEPENDS: #01_APIFramework, #02_Auth, #03_ZFSEngine
---

## Purpose
Basic pool operations: list, status, create, destroy.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/pools` | List all pool names |
| GET | `/v1/pools/{name}` | Pool status (health, size, vdevs) |
| POST | `/v1/pools` | Create pool |
| DELETE | `/v1/pools/{name}` | Destroy pool |

## RAID Types
| Type | Min Disks |
|------|-----------|
| single | 1 |
| mirror | 2 |
| raidz | 3 |
| raidz2 | 4 |
| raidz3 | 5 |

## Implementation
All via libzetta.

## Tests
- `tests/api_pools.rs`
- `tests/zfs_parcour.sh` — Pool lifecycle
