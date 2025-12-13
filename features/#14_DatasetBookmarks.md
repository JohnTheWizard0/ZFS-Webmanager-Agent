ACTIVE: #14_DatasetBookmarks
STATUS: planning
DEPENDS: #10_DatasetCRUD, #15_SnapshotCRUD
---

## Purpose
Lightweight snapshot references for incremental send without keeping full snapshot.

## Planned Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/datasets/{path}/bookmarks` | List bookmarks |
| POST | `/v1/snapshots/{ds}/{snap}/bookmark` | Create bookmark |
| DELETE | `/v1/datasets/{path}/bookmarks/{name}` | Delete bookmark |

## Command Reference
- `zfs bookmark <snapshot> <bookmark>`
- `zfs destroy <dataset>#<bookmark>`
- `zfs list -t bookmark`

## Use Case
Keep bookmark after deleting snapshot to enable future incremental sends:
```
snap1 -> bookmark1 -> (delete snap1) -> snap2 -> send -i #bookmark1 snap2
```

## TODO
- [ ] Research libzetta/FFI support
- [ ] Design bookmark naming convention
- [ ] Implement endpoints
