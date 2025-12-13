ACTIVE: #15_SnapshotCRUD
STATUS: stable
DEPENDS: #10_DatasetCRUD
---

## Purpose
Basic snapshot operations: list, create, delete.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/snapshots/{dataset}` | List snapshots |
| POST | `/v1/snapshots/{dataset}` | Create snapshot |
| DELETE | `/v1/snapshots/{dataset}/{name}` | Delete snapshot |

## Create Request
```json
{
  "snapshot_name": "backup-2025-12-10"
}
```

## Implementation
All via libzetta. Uses `DestroyTiming::RightNow` for deletion.

## Naming
Format: `dataset@snapshot_name`

## Tests
- `tests/api_snapshots.rs`
- `tests/zfs_parcour.sh`
