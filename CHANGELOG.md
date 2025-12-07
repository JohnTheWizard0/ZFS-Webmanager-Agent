# Changelog

All notable changes to this project will be documented in this file.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
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
