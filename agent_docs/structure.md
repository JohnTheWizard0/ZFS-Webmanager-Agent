# Project Structure

> Last updated: 2025-12-13

## Directory Map

```
zfs-agent/
├── agent_docs/              # AI context files
│   ├── commands.md          # Platform & service commands
│   ├── structure.md         # This file - project anatomy
│   └── workflow.md          # Development workflow stages
├── features/                # Feature specs and status
│   └── #XY_FeatureName.md   # Template for new features
├── _resources/              # Reference documentation
│   ├── ZFS_documentation/   # OpenZFS programmatic docs (20 files)
│   ├── BLACKBOARD.md        # Module index, blockers, decisions
│   ├── CLI_func.md          # CLI-based ZFS functions
│   ├── ZFS-Features.md      # Feature tracking by category
│   ├── MC-*.md              # Connector module docs
│   ├── MF-*.md              # Feature module docs
│   ├── MI-*.md              # Infrastructure module docs
│   └── _TEMPLATE_*.md       # Module doc templates
├── src/                     # Rust source code
│   ├── main.rs              # Entry point, server setup
│   ├── handlers.rs          # API route handlers
│   ├── zfs_management.rs    # ZFS operations (libzetta wrapper)
│   ├── auth.rs              # API key authentication
│   ├── models.rs            # Request/response structs
│   ├── task_manager.rs      # Async task tracking
│   └── utils.rs             # Helper functions
├── tests/                   # Integration tests
│   ├── api_*.rs             # Rust API tests
│   ├── zfs_parcour.sh       # Main integration test runner
│   ├── zfs_stress_a_*.sh    # Dataset/Snapshot/Property stress
│   ├── zfs_stress_b_*.sh    # Pool/Replication/Auth stress
│   └── cleanup_tests.sh     # Test pool cleanup
├── target/                  # Build artifacts (git-ignored)
├── Cargo.toml               # Rust dependencies
├── Cargo.lock               # Locked dependency versions
├── openapi.yaml             # API specification (OpenAPI 3.0)
├── rust-toolchain.toml      # Rust version pinning
├── CLAUDE.md                # Agent instructions
└── README.md                # Project readme
```

## Key Modules

| Module | Purpose | Entry Point |
|--------|---------|-------------|
| API Server | Actix-web REST API | src/main.rs |
| ZFS Engine | libzetta wrapper + CLI fallback | src/zfs_management.rs |
| Handlers | Route logic for pools/datasets/snapshots | src/handlers.rs |
| Auth | API key middleware | src/auth.rs |
| Task Manager | Async operation tracking | src/task_manager.rs |

## Dependencies

External (Cargo.toml):
- `actix-web`: HTTP server framework
- `libzetta`: ZFS bindings (primary)
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
