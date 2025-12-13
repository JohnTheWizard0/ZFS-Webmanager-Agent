ACTIVE: #19_SnapshotDiff
STATUS: planning
DEPENDS: #15_SnapshotCRUD
---

## Purpose
Show file-level changes between snapshots.

## Planned Endpoint
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/snapshots/{ds}/{snap}/diff` | Get diff |

## Query Parameters
- `from` — Compare from this snapshot (optional, defaults to current state)

## Command Reference
- `zfs diff <snapshot>` — Changes since snapshot
- `zfs diff <snap1> <snap2>` — Changes between snapshots

## Response Format (proposed)
```json
{
  "status": "success",
  "changes": [
    {"type": "+", "path": "/tank/data/newfile.txt"},
    {"type": "-", "path": "/tank/data/deleted.txt"},
    {"type": "M", "path": "/tank/data/modified.txt"},
    {"type": "R", "path": "/tank/data/old.txt", "new_path": "/tank/data/new.txt"}
  ]
}
```

## TODO
- [ ] Research libzetta/FFI support
- [ ] Likely CLI fallback needed
- [ ] Implement endpoint
