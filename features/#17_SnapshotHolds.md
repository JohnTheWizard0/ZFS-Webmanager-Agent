ACTIVE: #17_SnapshotHolds
STATUS: planning
DEPENDS: #15_SnapshotCRUD
---

## Purpose
Prevent accidental snapshot deletion via holds.

## Planned Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/snapshots/{ds}/{snap}/holds` | List holds |
| POST | `/v1/snapshots/{ds}/{snap}/hold` | Add hold |
| DELETE | `/v1/snapshots/{ds}/{snap}/holds/{tag}` | Release hold |

## Command Reference
- `zfs hold <tag> <snapshot>`
- `zfs release <tag> <snapshot>`
- `zfs holds <snapshot>`

## Use Case
Protect snapshots used for replication from accidental deletion.

## TODO
- [ ] Research libzetta/FFI support (`lzc_hold`, `lzc_release`)
- [ ] Design tag naming convention
- [ ] Implement endpoints
