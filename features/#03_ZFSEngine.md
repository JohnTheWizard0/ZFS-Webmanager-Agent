ACTIVE: #03_ZFSEngine
STATUS: perpetual
DEPENDS: none
---

## Purpose
Connector layer bridging REST handlers to ZFS. Wraps libzetta, libzfs FFI, and CLI fallbacks.

## Key Files
- `src/zfs_management.rs` — ZfsManager struct, all ZFS operations

## Implementation Hierarchy
1. **libzetta** — Preferred (Rust bindings)
2. **FROM-SCRATCH FFI** — When libzetta lacks feature (lzc_* functions)
3. **CLI** — Last resort, requires user approval, marked EXPERIMENTAL

## Current CLI Fallbacks
See `agent_docs/cli_reference.md` for full list:
- Dataset property SET
- Pool import with rename
- Replication receive
- Send size estimation (dry-run)

## Dependencies
- libzetta 0.5.0
- libzfs 0.6.16 (FFI)
- Root privileges required

## TODO
- [ ] Timeout handling
- [ ] Retry logic
- [ ] Detailed error codes
- [ ] Request logging
