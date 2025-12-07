# Changelog

All notable changes to this project will be documented in this file.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
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
