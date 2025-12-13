ACTIVE: #18_SnapshotRecursive
STATUS: planning
DEPENDS: #15_SnapshotCRUD
---

## Purpose
Create snapshots across entire dataset hierarchy.

## Planned Endpoint
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/snapshots/{dataset}` | Create recursive snapshot |

## Request (extended)
```json
{
  "snapshot_name": "backup-2025-12-10",
  "recursive": true
}
```

## Command Reference
- `zfs snapshot -r <pool/dataset>@<name>`

## Use Case
Consistent point-in-time across parent and all child datasets.

## Note
libzetta does NOT support recursive send (`-R`). This is separate from recursive snapshot creation.

## TODO
- [ ] Research libzetta support for recursive snapshot
- [ ] Determine if CLI fallback needed
- [ ] Extend existing endpoint or create new
