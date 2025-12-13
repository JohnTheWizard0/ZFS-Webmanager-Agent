# Project Structure

> Last updated: 2025-12-13 (tree sync)

## Directory Map

```
zfs-agent/
├── agent_docs/              # AI context files
│   ├── cli_reference.md     # CLI commands reference
│   ├── commands.md          # Platform & service commands
│   ├── structure.md         # This file - project anatomy
│   ├── testing.md           # Test suite documentation
│   └── workflow.md          # Development workflow stages
├── features/                # Feature specs and status
│   ├── archive/             # Merged/archived feature specs
│   ├── #00_Refactoring.md   # Codebase refactoring tasks
│   ├── #01-#23_*.md         # Active feature specs (see features/)
│   └── #XY_FeatureName.md   # Template for new features
├── _resources/              # Reference documentation
│   ├── ZFS_documentation/   # OpenZFS programmatic docs (20 files)
│   └── ZFS-Features.md      # Feature tracking by category
├── src/                     # Rust source code
│   ├── main.rs              # Entry point, server setup, routes
│   ├── handlers/            # API route handlers (modular)
│   │   ├── mod.rs           # Re-exports all handlers
│   │   ├── docs.rs          # docs (Swagger/JSON), openapi (generated), health, features
│   │   ├── safety.rs        # safety_status, safety_override
│   │   ├── pools.rs         # pool handlers
│   │   ├── datasets.rs      # dataset handlers
│   │   ├── snapshots.rs     # snapshot handlers
│   │   ├── scrub.rs         # scrub handlers
│   │   ├── vdev.rs          # vdev handlers
│   │   ├── replication.rs   # send/receive/replicate handlers
│   │   └── utility.rs       # execute_command, format_bytes
│   ├── zfs_management/      # ZFS operations (modular)
│   │   ├── mod.rs           # Re-exports ZfsManager + types
│   │   ├── types.rs         # PoolStatus, ScrubStatus, etc.
│   │   ├── ffi.rs           # FFI declarations + RAII guards
│   │   ├── helpers.rs       # errno_to_string
│   │   ├── manager.rs       # ZfsManager struct + new()
│   │   ├── pools.rs         # pool operations
│   │   ├── datasets.rs      # dataset operations
│   │   ├── snapshots.rs     # snapshot operations
│   │   ├── scrub.rs         # scrub operations
│   │   ├── vdev.rs          # vdev operations
│   │   ├── replication.rs   # send/receive operations
│   │   └── tests.rs         # unit tests
│   ├── safety.rs            # ZFS version safety lock
│   ├── auth.rs              # API key authentication
│   ├── models.rs            # Request/response structs
│   ├── task_manager.rs      # Async task tracking
│   └── utils.rs             # Helper functions, filters
├── templates/               # HTML templates (embedded at compile time)
│   ├── docs.html            # Swagger UI page template
│   └── features.html        # Feature coverage page template
├── tests/                   # Integration tests
│   ├── api_*.rs             # Rust API tests (auth, datasets, health, pools, snapshots)
│   ├── zfs_stress_a_*.sh    # Dataset/Snapshot/Property stress (short/long)
│   ├── zfs_stress_b_*.sh    # Pool/Replication/Auth stress (short/long)
│   └── cleanup_tests.sh     # Test pool cleanup
├── target/                  # Build artifacts (git-ignored)
├── Cargo.toml               # Rust dependencies
├── Cargo.lock               # Locked dependency versions
├── api.json                 # API definition (source of truth, lean format)
├── features.json            # ZFS feature list (loaded at runtime by /features)
├── settings.json            # Agent configuration (safety settings)
├── rust-toolchain.toml      # Rust version pinning
├── CLAUDE.md                # Agent instructions
├── README.md                # Project readme
├── zfsreload-logo-dark-rounded.svg   # Brand logo (dark theme)
└── zfsreload-logo-light-rounded.svg  # Brand logo (light theme)
```

## Key Modules

| Module | Purpose | Entry Point |
|--------|---------|-------------|
| API Server | Warp REST API server | src/main.rs |
| ZFS Engine | libzetta wrapper + FFI + CLI fallback | src/zfs_management/mod.rs |
| Handlers | Route logic for pools/datasets/snapshots | src/handlers/mod.rs |
| Safety | ZFS version validation & lock | src/safety.rs |
| Auth | API key middleware | src/auth.rs |
| Task Manager | Async operation tracking | src/task_manager.rs |

## Dependencies

External (Cargo.toml):
- `warp`: HTTP server framework
- `libzetta`: ZFS bindings (primary)
- `libzfs`, `libzfs-sys`: FFI for advanced ZFS operations
- `serde`: Serialization
- `tokio`: Async runtime

Internal (cross-module):
- `handlers` → `zfs_management`: All ZFS operations
- `handlers` → `auth`: Request validation
- `main` → `handlers`: Route registration

## Module Prefixes

| Prefix | Type | Example |
|--------|------|---------|
| MI-XXX | Infrastructure | MI-001_Auth.md |
| MF-XXX | Feature | MF-001_PoolManagement.md |
| MC-XXX | Connector | MC-001_ZFSEngine.md |

## Update Instructions

When adding/removing modules or significant files:
1. Update directory map above
2. Update key modules table if new entry point
3. Note new dependencies
4. Run `tree` skill to verify
