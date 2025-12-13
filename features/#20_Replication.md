ACTIVE: #20_Replication
STATUS: implementing
DEPENDS: #15_SnapshotCRUD, #03_ZFSEngine
---

## Purpose
Snapshot-based replication: send, receive, replicate between pools.

## Endpoints
| Method | Path | Implementation |
|--------|------|----------------|
| POST | `/v1/snapshots/{ds}/{snap}/send` | libzetta |
| POST | `/v1/datasets/{path}/receive` | **CLI** |
| POST | `/v1/snapshots/{ds}/{snap}/replicate` | **HYBRID** |
| GET | `/v1/snapshots/{ds}/{snap}/send-size` | FROM-SCRATCH FFI |
| GET | `/v1/tasks/{task_id}` | In-memory |

## Send Options
| Option | Flag | Field |
|--------|------|-------|
| Incremental | `-i` | `from_snapshot` |
| Raw/encrypted | `-w` | `raw` |
| Compressed | `-c` | `compressed` |
| Large blocks | `-L` | `large_blocks` |
| Dry run | `-n` | `dry_run` |

## Receive Options
| Option | Flag | Field |
|--------|------|-------|
| Force | `-F` | `force` |
| Dry run | `-n` | `dry_run` |

## Implementation Notes
- **Send**: libzetta `send_full()` / `send_incremental()`
- **Receive**: CLI `zfs receive` (lzc_receive too low-level)
- **Replicate**: HYBRID — libzetta send piped to CLI receive
- **Size estimate**: FROM-SCRATCH `lzc_send_space()` FFI

## FFI Solution for Receive
`lzc_receive()` available via libzfs_core:
```c
int lzc_receive(const char *snapname, nvlist_t *props,
                const char *origin, boolean_t force,
                boolean_t raw, int fd)
```
- `snapname`: Target snapshot name to create
- `props`: Properties to set, or NULL
- `origin`: Clone origin for clone receive, or NULL
- `force`: Rollback if necessary
- `raw`: Expect raw encrypted stream
- `fd`: Input file descriptor
- Complexity: **High** — "too low-level" note suggests stream header parsing issues
- Reference: `_resources/ZFS_documentation/07_send_receive.txt:109-118`

Investigation needed: Why was lzc_receive deemed "too low-level"? May need manual DRR_BEGIN header parsing.

## Task System
- One task per pool (busy tracking)
- Both source AND target pools marked busy during replicate
- Tasks expire 1 hour after completion
- RAM-only storage

## Encrypted Dataset Rules
- MUST use `-w` (raw) flag
- NO `-F` (force) on receive
- Each zvol sent individually

## TODO
- [ ] Replace CLI receive with proper FFI
- [ ] Support recursive send (`-R`) — NOT supported by libzetta
