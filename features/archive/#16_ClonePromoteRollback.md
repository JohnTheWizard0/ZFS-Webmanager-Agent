ACTIVE: #16_ClonePromoteRollback
STATUS: stable
DEPENDS: #15_SnapshotCRUD
---

## Purpose
Clone snapshots, promote clones, rollback datasets.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/snapshots/{ds}/{snap}/clone` | Clone snapshot |
| POST | `/v1/datasets/{path}/promote` | Promote clone |
| POST | `/v1/datasets/{path}/rollback` | Rollback to snapshot |

## Implementation
All FROM-SCRATCH FFI via `libzetta-zfs-core-sys`:
- `lzc_clone()` — Create writable clone
- `lzc_promote()` — Swap clone/origin relationship
- `lzc_rollback_to()` — Rollback with safety levels

## Rollback Safety Levels
| Level | Description |
|-------|-------------|
| default | Fail if newer snapshots exist |
| force_destroy_newer | Destroy newer snapshots |
| force_destroy_clones | Also destroy dependent clones |

## Clone Request
```json
{
  "target_dataset": "tank/clone1"
}
```
