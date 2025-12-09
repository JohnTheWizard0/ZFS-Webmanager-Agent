# Changelog

All notable changes to this project will be documented in this file.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
- `tests/zfs_stress_a_short.sh` — NEW: Dataset/Snapshot/Property stress tests (short)
- `tests/zfs_stress_a_long.sh` — NEW: Dataset/Snapshot/Property stress tests (long)
- `tests/zfs_stress_b_short.sh` — NEW: Pool/Replication/Auth stress tests (short)
- `tests/zfs_stress_b_long.sh` — NEW: Pool/Replication/Auth stress tests (long)
- `tests/STRESS_TESTS.md` — NEW: Comprehensive stress test documentation
- `src/zfs_management.rs` — Recursive dataset delete
  - `delete_dataset_recursive()` — Uses lzc_destroy() FFI per item
  - Lists children/snapshots, sorts by depth, deletes deepest first
- `src/zfs_management.rs` — Import with rename
  - `import_pool_with_name()` — CLI-based rename on import
- `src/models.rs` — New request types
  - `DeleteDatasetQuery` — Query param for recursive delete
  - `ImportPoolRequest.new_name` — Optional rename field
- `src/main.rs` — Updated routes
  - `DELETE /v1/datasets/{name}?recursive=true` — Recursive delete support
- `src/task_manager.rs` — NEW: Async task management (MF-005 Replication)
  - `TaskManager` — Pool busy tracking, task creation, progress updates
  - `TaskState` — Task status with progress and result tracking
  - Auto-expiry of completed tasks (1 hour)
  - Both source AND target pools marked busy during replication
- `src/zfs_management.rs` — Replication operations (MF-005)
  - `send_snapshot_to_file()` — **libzetta** `send_full()`/`send_incremental()` with SendFlags
  - `receive_snapshot_from_file()` — **CLI** `zfs receive` (lzc_receive too low-level)
  - `replicate_snapshot()` — **Hybrid** libzetta send + CLI receive via pipe
  - `estimate_send_size()` — **FFI** `lzc_send_space()` for accurate stream size estimation
  - `get_pool_from_path()` — Pool extraction helper
- `src/handlers.rs` — Replication HTTP handlers
  - `get_task_status_handler` — Task status lookup
  - `send_size_handler` — Size estimation via **FFI** `lzc_send_space()`
  - `send_snapshot_handler` — Send to file with task tracking
  - `receive_snapshot_handler` — Receive from file with task tracking
  - `replicate_snapshot_handler` — Direct replication with both pools busy
- `src/models.rs` — Replication request/response types
  - `TaskStatus`, `TaskOperation`, `TaskProgress`, `TaskState`
  - `TaskResponse`, `TaskStatusResponse`
  - `SendSnapshotRequest`, `ReceiveSnapshotRequest`, `ReplicateSnapshotRequest`
  - `SendSizeQuery`, `SendSizeResponse`
- `src/main.rs` — Replication API routes
  - `GET /v1/tasks/{task_id}` — Task status
  - `GET /v1/snapshots/{ds}/{snap}/send-size` — Size estimation
  - `POST /v1/snapshots/{ds}/{snap}/send` — Send to file
  - `POST /v1/datasets/{path}/receive` — Receive from file
  - `POST /v1/replication/{ds}/{snap}` — Direct pool-to-pool replication
- `src/zfs_management.rs` — Dataset properties operations (MF-002 Phase 2)
  - `get_dataset_properties()` — via libzetta `ZfsEngine::read_properties()`
  - `set_dataset_property()` — **EXPERIMENTAL** via CLI (`zfs set`)
  - `DatasetProperties` struct with comprehensive field mapping
  - `is_valid_property_name()` — input validation for security
- `src/handlers.rs` — Dataset properties HTTP handlers
  - `get_dataset_properties_handler`
  - `set_dataset_property_handler`
- `src/models.rs` — Dataset properties request/response types
  - `DatasetPropertiesResponse`
  - `SetPropertyRequest`
- `src/main.rs` — Dataset properties API routes
  - `GET /v1/datasets/{path}/properties`
  - `PUT /v1/datasets/{path}/properties` (**EXPERIMENTAL**)
- `openapi.yaml` — Dataset properties endpoint documentation
- `src/handlers.rs` — Swagger UI documentation endpoint
  - `docs_handler` — Serves interactive Swagger UI at `/v1/docs`
  - `openapi_handler` — Serves OpenAPI spec at `/v1/openapi.yaml`
- `src/zfs_management.rs` — Pool import/export operations via libzetta
  - `export_pool()` with force option
  - `import_pool()` and `import_pool_from_dir()`
  - `list_importable_pools()` and `list_importable_pools_from_dir()`
  - `ImportablePool` struct
- `src/handlers.rs` — Import/export HTTP handlers
  - `export_pool_handler`
  - `import_pool_handler`
  - `list_importable_pools_handler`
- `src/models.rs` — Import/export request/response types
  - `ExportPoolRequest`
  - `ImportPoolRequest`
  - `ImportablePoolInfo`
  - `ImportablePoolsResponse`
- `src/main.rs` — New API routes under `/v1` prefix
  - `GET /v1/docs` — OpenAPI spec (YAML)
  - `POST /v1/pools/{name}/export`
  - `POST /v1/pools/import`
  - `GET /v1/pools/importable`
- `openapi.yaml` — OpenAPI 3.0 specification (620 lines)
- `CHANGELOG.md` — Version history tracking

### Changed
- `src/zfs_management.rs` — Replication refactored from CLI to libzetta/FFI
  - Send: CLI → **libzetta** `ZfsEngine::send_full()`/`send_incremental()`
  - Size estimation: CLI `zfs send -nP` → **FFI** `lzc_send_space()`
  - Replicate: Full CLI → **Hybrid** (libzetta send + CLI receive via pipe)
  - NOTE: `lzc_receive()` is too low-level (no stream header parsing), kept CLI
- `src/models.rs` — Added `ImplementationMethod::Hybrid` enum variant
- `src/handlers.rs` — Features page now shows Hybrid badge with gradient style
- `rust-toolchain.toml` — Pinned to stable (Rust 1.91.1)
- `src/zfs_management.rs` — Snapshot clone/promote operations (MF-003 Phase 3)
  - `clone_snapshot()` — FROM-SCRATCH via `lzc_clone()` FFI
  - `promote_dataset()` — FROM-SCRATCH via `lzc_promote()` FFI
  - `errno_to_string()` — Helper for FFI error codes
- `src/handlers.rs` — Clone/promote HTTP handlers
  - `clone_snapshot_handler`
  - `promote_dataset_handler`
- `src/models.rs` — Clone/promote request/response types
  - `CloneSnapshotRequest`
  - `CloneResponse`
  - `PromoteResponse`
- `src/main.rs` — Clone/promote API routes
  - `POST /v1/snapshots/{dataset}/{snapshot}/clone`
  - `POST /v1/datasets/{path}/promote`
- `openapi.yaml` — Clone/promote endpoint documentation
- `Cargo.toml` — Added `libzetta-zfs-core-sys` and `libc` dependencies for FFI
- `src/zfs_management.rs` — Snapshot rollback operations (MF-003 Phase 3)
  - `rollback_dataset()` — FROM-SCRATCH via `lzc_rollback_to()` FFI
  - `RollbackResult` struct with destruction tracking
  - `RollbackError` enum for detailed error handling
  - Three safety levels: default (most recent only), force_destroy_newer (-r), force_destroy_clones (-R)
- `src/handlers.rs` — Rollback HTTP handler
  - `rollback_dataset_handler` with blocked response for safety violations
- `src/models.rs` — Rollback request/response types
  - `RollbackRequest` with force flags
  - `RollbackResponse` with destruction tracking
  - `RollbackBlockedResponse` for safety violations
- `src/main.rs` — Rollback API route
  - `POST /v1/datasets/{path}/rollback`
- `openapi.yaml` — Rollback endpoint documentation with all schemas
- `src/models.rs` — ZFS features discovery types
  - `ImplementationMethod` enum (libzetta, ffi, libzfs, cli_experimental, planned)
  - `FeatureCategory` enum (pool, dataset, snapshot, property, replication, system)
  - `ZfsFeatureInfo` struct with feature details
  - `ZfsFeaturesResponse` with summary and features list
- `src/handlers.rs` — ZFS features handler
  - `zfs_features_handler` — returns all features with implementation status
- `src/main.rs` — ZFS features route
  - `GET /v1/features` (no auth required)
  - HTML view by default, JSON via `?format=json`
- `src/handlers.rs` — `build_features_html()` for visual dashboard
- `openapi.yaml` — ZFS features endpoint documentation

### Changed
- `src/main.rs` — All API routes now under `/v1/` prefix
- `src/zfs_management.rs` — Added `ExportMode` import from libzetta
- `openapi.yaml` — Server URL updated to `http://localhost:9876/v1`

---

## [0.3.3] - 2025-12-07

### Added
- `Cargo.toml` — Added `libzfs = "0.6.16"` dependency for FFI bindings
- `src/zfs_management.rs` — FROM-SCRATCH scan stats extraction using libzfs FFI
  - Bypasses libzetta limitation for scrub progress
  - Extracts `pool_scan_stat_t` fields from nvlist
  - `ScrubStatus` struct with full scan details
  - `get_scrub_status()` method
  - `scan_state_to_string()` and `scan_func_to_string()` helpers
- `src/zfs_management.rs` — Scrub control operations via libzetta
  - `start_scrub()`
  - `pause_scrub()`
  - `stop_scrub()`
- `src/handlers.rs` — Scrub HTTP handlers
  - `start_scrub_handler`
  - `pause_scrub_handler`
  - `stop_scrub_handler`
  - `get_scrub_status_handler` with percent calculation
- `src/models.rs` — Extended `ScrubStatusResponse` with all fields
  - `scan_state`, `scan_function`, `start_time`, `end_time`
  - `to_examine`, `examined`, `scan_errors`, `percent_done`
- `src/main.rs` — Scrub API routes
  - `POST /pools/{name}/scrub` (start)
  - `POST /pools/{name}/scrub/pause`
  - `POST /pools/{name}/scrub/stop`
  - `GET /pools/{name}/scrub` (status)
- `tests/zfs_parcour.sh` — End-to-end test script for ZFS operations
- `run_tests.sh` — Test runner with unit/integration options
- `_resources/ZFS-Features.md` — Feature coverage tracking document

### Fixed
- `src/main.rs` — Snapshot routes now use `path::tail()` for multi-segment dataset paths
- `src/zfs_management.rs` — Removed undefined `request_builder` dead code (B002)
- `src/zfs_management.rs` — Fixed `CreateDataset` import path (B001)

### Changed
- Unit tests moved in-file with `#[cfg(test)]` modules
- Integration tests moved to `tests/` directory

---

## [0.3.2] - 2025-12-06

### Added
- Initial ZFS agent implementation
- Pool CRUD operations (list, status, create, destroy)
- Dataset CRUD operations (list, create, delete)
- Snapshot CRUD operations (list, create, delete)
- API key authentication
- Health endpoint with version and last action tracking
- Command execution endpoint

---

## Version History

| Version | Date | Summary |
|---------|------|---------|
| 0.3.3 | 2025-12-07 | Scrub operations, test infrastructure, bug fixes |
| 0.3.2 | 2025-12-06 | Initial implementation |
