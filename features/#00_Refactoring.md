# Feature #00: Split handlers.rs and zfs_management.rs into Sub-Modules

**Status**: DONE (testing complete)
**Completed**: 2025-12-13

## Goal
Refactor two large files into directory-style modules organized by ZFS concept (pools, datasets, snapshots, etc.) while maintaining backward compatibility.

## Previous State (Before Refactoring)
| File | Lines | Content |
|------|-------|---------|
| `src/handlers.rs` | 1,535 | 24 HTTP handlers + 2 helpers |
| `src/zfs_management.rs` | 2,644 | 34 methods + FFI + types + tests |

## Target Structure

### src/handlers/
```
handlers/
├── mod.rs           # Re-exports all handlers
├── docs.rs          # ~450 lines: openapi, docs, health, features handlers
├── safety.rs        # ~50 lines: safety_status, safety_override
├── pools.rs         # ~280 lines: list, status, create, destroy, export, import
├── datasets.rs      # ~200 lines: list, create, delete, get/set properties
├── snapshots.rs     # ~250 lines: list, create, delete, clone, promote, rollback
├── scrub.rs         # ~100 lines: start, pause, stop, status
├── vdev.rs          # ~60 lines: add, remove
├── replication.rs   # ~250 lines: send_size, send, receive, replicate
└── utility.rs       # ~80 lines: execute_command, task_status, format_bytes
```

### src/zfs_management/
```
zfs_management/
├── mod.rs           # Re-exports ZfsManager + public types
├── manager.rs       # ~50 lines: ZfsManager struct + new()
├── ffi.rs           # ~250 lines: FFI declarations + RAII guards
├── types.rs         # ~250 lines: PoolStatus, ScrubStatus, RollbackError, DatasetProperties
├── helpers.rs       # ~30 lines: errno_to_string
├── pools.rs         # ~350 lines: list, status, create, destroy, export, import ops
├── datasets.rs      # ~300 lines: list, create, delete, properties
├── snapshots.rs     # ~400 lines: list, create, delete, clone, promote, rollback
├── scrub.rs         # ~150 lines: start, pause, stop, status + scan helpers
├── vdev.rs          # ~500 lines: add, remove + nvlist builders
├── replication.rs   # ~350 lines: send, receive, replicate, estimate_size
└── tests.rs         # ~175 lines: unit tests
```

## Implementation Order

### Phase 1: Create Directory Structure
1. Create `src/handlers/` and `src/zfs_management/` directories
2. Create empty `mod.rs` files

### Phase 2: Extract zfs_management (Higher Risk First)
Order matters - extract dependencies before dependents:

1. **types.rs** - Extract all public types:
   - `PoolStatus`, `ImportablePool`, `ZfsError`, `ScrubStatus`
   - `RollbackError`, `RollbackResult`, `DatasetProperties`

2. **helpers.rs** - Extract `errno_to_string`

3. **ffi.rs** - Extract FFI and RAII guards:
   - `zpool_handle_t`, extern "C" declarations
   - `LibzfsGuard`, `PoolGuard`, `NvlistGuard`

4. **manager.rs** - Extract `ZfsManager` struct + `new()`

5. **Extract impl blocks** (each file adds `impl ZfsManager { ... }`):
   - `scrub.rs` - Self-contained, good test
   - `pools.rs` - Pool operations
   - `datasets.rs` - Dataset operations
   - `snapshots.rs` - Snapshot + clone/promote/rollback
   - `vdev.rs` - Vdev ops + nvlist helpers
   - `replication.rs` - Send/receive operations

6. **tests.rs** - Move all `#[cfg(test)]` tests

7. Delete original `src/zfs_management.rs`

### Phase 3: Extract handlers (Lower Risk)
1. **utility.rs** - `execute_command_handler`, `get_task_status_handler`, `format_bytes`
2. **docs.rs** - Documentation/health handlers + `build_features_html`
3. **safety.rs** - Safety handlers
4. **scrub.rs** - Scrub handlers
5. **pools.rs** - Pool handlers
6. **datasets.rs** - Dataset handlers
7. **snapshots.rs** - Snapshot handlers
8. **vdev.rs** - Vdev handlers
9. **replication.rs** - Replication handlers
10. Delete original `src/handlers.rs`

### Phase 4: Verification
- `cargo check` after each file extraction
- `cargo test` to verify all tests pass
- `cargo clippy` for lint checks

## Key Design Decisions

### Backward Compatibility
- `mod.rs` re-exports everything with same names
- `main.rs` requires NO changes
- `use handlers::*` and `use zfs_management::ZfsManager` continue to work

### FFI Handling
- Keep FFI centralized in `ffi.rs` (used by multiple modules)
- Methods import FFI via `use super::ffi::*`
- No separate "FFI layer" - keeps impl with the functions that use it

### ZfsManager Pattern
```rust
// manager.rs defines the struct
pub struct ZfsManager { ... }
impl ZfsManager { pub fn new() -> Self { ... } }

// Other files extend with impl blocks
// pools.rs
impl ZfsManager {
    pub async fn list_pools(&self) -> ... { }
}
```

### Shared Utilities
- `format_bytes` stays in `handlers/utility.rs`, imported by replication.rs
- `errno_to_string` in `zfs_management/helpers.rs`

## Files to Modify
- `src/zfs_management.rs` → split into `src/zfs_management/` (delete original)
- `src/handlers.rs` → split into `src/handlers/` (delete original)
- `src/main.rs` → NO changes needed (backward compatible)

## Validation Checklist
- [x] `cargo check` passes
- [x] `cargo build` succeeds
- [x] `cargo test` runs all tests (53 passed, 3 ignored)
- [x] `cargo clippy` no new warnings
- [x] Each new file < 500 lines
- [x] No duplicate code
- [x] main.rs unchanged (backward compatible)
