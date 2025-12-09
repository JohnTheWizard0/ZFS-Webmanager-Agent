# ZFS Agent Stress Tests Documentation

> **Version:** 1.0 | **Created:** 2025-12-09 | **Agent:** 1

---

## Feature Implementation Status

All planned features have been implemented:

| Feature | Status | Endpoint | Notes |
|---------|--------|----------|-------|
| Rollback | **Implemented** | `POST /v1/datasets/{path}/rollback` | FFI lzc_rollback_to(), 3 safety levels |
| Recursive delete | **Implemented** | `DELETE /v1/datasets/{name}?recursive=true` | FFI lzc_destroy() per item |
| Force export | **Implemented** | `POST /v1/pools/{name}/export` | `force: true` in body |
| Scrub stop | **Implemented** | `POST /v1/pools/{name}/scrub/stop` | libzetta stop_scrub() |
| Scrub pause | **Implemented** | `POST /v1/pools/{name}/scrub/pause` | libzetta pause_scrub() |
| Import rename | **Implemented** | `POST /v1/pools/import` | `new_name` field (CLI-based) |

### Recently Implemented (2025-12-09)

1. **Recursive Dataset Delete** - `DELETE /v1/datasets/{name}?recursive=true`
   - Lists all children and snapshots via libzetta
   - Sorts by depth (deepest first)
   - Uses lzc_destroy() FFI for each item

2. **Import with Rename** - `POST /v1/pools/import` with `{"name": "old", "new_name": "new"}`
   - CLI-based implementation (libzetta doesn't expose rename)
   - Supports optional `dir` for alternate search path

---

## Test Suite Overview

```
tests/
├── zfs_parcour.sh           # Happy-path E2E (existing, ~2 min)
├── zfs_stress_a_short.sh    # Dataset/Snapshot/Property edge cases (~3 min)
├── zfs_stress_a_long.sh     # Dataset/Snapshot/Property full stress (~10 min)
├── zfs_stress_b_short.sh    # Pool/Replication/Auth edge cases (~5 min)
└── zfs_stress_b_long.sh     # Pool/Replication/Auth full stress (~15 min)
```

---

## Test-A: Datasets, Properties, Snapshots

Focus: Data-layer operations and edge cases

### A1. Dataset Properties (MF-002)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| P1 | Set invalid property name | Medium | Error, no crash | Y | Y |
| P2 | Set invalid property value (compression=invalid) | Medium | ZFS error returned | Y | Y |
| P3 | Set read-only property (used, creation) | Medium | Error, property unchanged | Y | Y |
| P4 | Set property with special chars in value | Low | Accept or clean error | Y | Y |
| P5 | Set property on non-existent dataset | Medium | 404 error | Y | Y |
| P6 | Rapid property changes (10 sets in 2 seconds) | High | All succeed or clean failures | - | Y |
| P7 | Property inheritance - set on parent, verify child | Medium | Child inherits | - | Y |
| P8 | Override inherited property on child | Medium | Child has local value | - | Y |

### A2. Snapshots (MF-003)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| S1 | Snapshot with invalid chars (@, spaces) in name | High | Reject invalid | Y | Y |
| S2 | Snapshot on non-existent dataset | Medium | 404 error | Y | Y |
| S3 | Duplicate snapshot name | Medium | Error (already exists) | Y | Y |
| S4 | Delete snapshot that has dependent clone | High | Error | Y | Y |
| S5 | Create 5 snapshots rapidly | Medium | All succeed | Y | Y |
| S6 | Clone same snapshot multiple times | Medium | All clones created | - | Y |
| S7 | Promote clone, verify snapshot transfer | High | Original becomes clone | - | Y |
| S8 | Delete non-existent snapshot | Low | 404 error | - | Y |
| S9 | Snapshot name at max length (~200 chars) | Medium | Accept or clean error | - | Y |
| S10 | Create 50 snapshots, list all | High | List returns all 50 | - | Y |

### A3. Datasets (MF-002)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| D1 | Create deeply nested dataset (5 levels) | Medium | Success | Y | Y |
| D2 | Create dataset with special chars in name | High | Reject invalid | Y | Y |
| D3 | Create duplicate dataset | Medium | Error (already exists) | Y | Y |
| D4 | Delete dataset with child datasets | High | Error (need -r) | Y | Y |
| D4b | Recursive delete via API (?recursive=true) | High | Success, all children deleted | Y | Y |
| D5 | Delete dataset with snapshots | High | Error | Y | Y |
| D6 | Create volume vs filesystem | Medium | Both types work | - | Y |
| D7 | Create dataset on non-existent pool | Medium | Error | - | Y |
| D8 | Dataset name at max length | Medium | Accept or clean error | - | Y |
| D9 | 10 concurrent dataset creates | High | All succeed | - | Y |
| D10 | Delete dataset that is clone origin | High | Error | - | Y |

### A4. Rollback (MF-003)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| R1 | Rollback to most recent snapshot | Medium | Success | Y | Y |
| R2 | Rollback to older snapshot (blocked) | High | Error + blocking info | Y | Y |
| R3 | Rollback with force_destroy_newer | High | Success, snaps destroyed | - | Y |
| R4 | Rollback blocked by clones | High | Error + clone info | - | Y |
| R5 | Rollback to non-existent snapshot | Medium | 404 error | - | Y |

---

## Test-B: Pools, Replication, Auth, API

Focus: Infrastructure and integration edge cases

### B1. Pool Operations (MF-001)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| PO1 | Create pool with invalid disk path | High | Error, no partial state | Y | Y |
| PO2 | Create pool with disk already in pool | High | Error (device in use) | Y | Y |
| PO3 | Create duplicate pool name | Medium | Error (already exists) | Y | Y |
| PO4 | Create pool with different RAID types | Medium | All succeed | Y | Y |
| PO5 | Destroy non-existent pool | Low | 404 error | Y | Y |
| PO6 | Destroy pool with datasets (no force) | High | Error | - | Y |
| PO7 | Destroy pool with active snapshots | High | Error | - | Y |
| PO8 | Get status of non-existent pool | Low | 404 error | - | Y |
| PO9 | Pool name with special characters | Medium | Reject invalid | - | Y |
| PO10 | Pool name at max length | Low | Accept or error | - | Y |

### B2. Scrub Operations (MF-001)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| SC1 | Start scrub on non-existent pool | Medium | 404 error | Y | Y |
| SC2 | Stop scrub when none running | Low | Error or no-op | Y | Y |
| SC3 | Get scrub status on non-existent pool | Low | 404 error | - | Y |
| SC4 | Pause scrub when none running | Low | Error or no-op | - | Y |
| SC5 | Start/stop scrub cycle | Medium | Clean transitions | - | Y |

### B3. Export/Import (MF-001)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| EI1 | Export non-existent pool | Medium | 404 error | Y | Y |
| EI2 | Export pool, verify gone from list | Medium | Success | Y | Y |
| EI3 | Import non-existent pool | Medium | Error | Y | Y |
| EI4 | Import with rename (new_name field) | Medium | Success, pool renamed | Y | Y |
| EI5 | Double export (export already exported) | Low | Error | - | Y |
| EI6 | Import, export, re-import cycle | Medium | All succeed | - | Y |
| EI7 | List importable when none available | Low | Empty list | - | Y |
| EI8 | Force export with force=true | Medium | Success | - | Y |

### B4. Replication (MF-005)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| RE1 | Send non-existent snapshot | Medium | 404 error | Y | Y |
| RE2 | Send to invalid file path | Medium | Error | Y | Y |
| RE3 | Send to existing file (no overwrite) | Medium | Error | Y | Y |
| RE4 | Send size estimate on non-existent | Low | 404 error | Y | Y |
| RE5 | Receive from non-existent file | Medium | Error | Y | Y |
| RE6 | Receive to existing dataset (no force) | Medium | Error | - | Y |
| RE7 | Replicate non-existent snapshot | Medium | 404 error | - | Y |
| RE8 | Replicate to non-existent target pool | Medium | Error | - | Y |
| RE9 | Task status for non-existent task | Low | 404 or empty | - | Y |
| RE10 | Send with overwrite=true | Low | Success | - | Y |
| RE11 | Replicate to same pool | Low | Success | - | Y |
| RE12 | Large snapshot replication (~100MB) | High | Success + progress | - | Y |

### B5. Authentication (MI-001)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| AU1 | Request without API key | High | 401 Unauthorized | Y | Y |
| AU2 | Request with invalid API key | High | 401 Unauthorized | Y | Y |
| AU3 | Health endpoint without key | Low | Should work (public) | Y | Y |
| AU4 | Malformed API key (not UUID) | Medium | 401 Unauthorized | - | Y |
| AU5 | API key in wrong header | Medium | 401 Unauthorized | - | Y |

### B6. API Robustness (MI-002)

| ID | Test Case | Risk | Expected | Short | Long |
|----|-----------|------|----------|-------|------|
| AR1 | Malformed JSON body | Medium | 400 error | Y | Y |
| AR2 | Missing required fields | Medium | 400 with field name | Y | Y |
| AR3 | Extra unexpected fields | Low | Ignored | Y | Y |
| AR4 | Very large payload (1MB) | Medium | Reject or timeout | - | Y |
| AR5 | Empty body where required | Medium | 400 error | - | Y |
| AR6 | Wrong Content-Type header | Low | Error or handled | - | Y |
| AR7 | Request to non-existent endpoint | Low | 404 | - | Y |
| AR8 | Wrong HTTP method on endpoint | Low | 405 | - | Y |
| AR9 | 10 concurrent requests | High | All handled | - | Y |

---

## Test Execution

### Prerequisites

- ZFS kernel module loaded
- At least 4 spare disks/loop devices
- zfs-agent service running or buildable
- API key configured (default: 08670612-43df-4a0c-a556-2288457726a5)

### Running Tests

```bash
# Quick validation (short tests only, ~8 min total)
./tests/zfs_stress_a_short.sh && ./tests/zfs_stress_b_short.sh

# Full stress testing (~25 min total)
./tests/zfs_stress_a_long.sh && ./tests/zfs_stress_b_long.sh

# Individual test suites
./tests/zfs_stress_a_short.sh   # Dataset/Snapshot/Property (~3 min)
./tests/zfs_stress_a_long.sh    # Dataset/Snapshot/Property full (~10 min)
./tests/zfs_stress_b_short.sh   # Pool/Replication/Auth (~5 min)
./tests/zfs_stress_b_long.sh    # Pool/Replication/Auth full (~15 min)
```

### Environment Variables

```bash
API_URL=http://localhost:9876    # API endpoint
API_KEY=08670612-43df-4a0c-a556-2288457726a5  # API key
CLEANUP=true                     # Clean up test artifacts (default: true)
VERBOSE=true                     # Show detailed output (default: false)
```

---

## Test Result Interpretation

### Pass Criteria
- All "Expected" behaviors match actual responses
- No crashes or hangs
- Resources properly cleaned up
- Error messages are informative (not stack traces)

### Risk Levels
- **High**: Could cause data loss, corruption, or system instability
- **Medium**: Could cause service disruption or incorrect state
- **Low**: Minor issues, cosmetic, or edge cases

### Failure Handling
- Tests marked FAIL include error details
- SKIP means test prerequisite not met
- Cleanup runs regardless of test outcome

---

## Changelog

| Date | Change | By |
|------|--------|-----|
| 2025-12-09 | Initial stress test documentation | Agent 1 |
