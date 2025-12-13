ACTIVE: #23_SafetyLock
STATUS: implementing
DEPENDS: #01_APIFramework
---

## Purpose
ZFS version safety mechanism that detects ZFS version at startup and enters "read-only mode" if version is unapproved. Prevents accidental operations on untested ZFS versions.

## Endpoints
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/v1/safety` | No | Get safety status |
| POST | `/v1/safety` | No | Override safety lock |

## Behavior
1. **Startup**: Detect ZFS version, log it, compare against hardcoded approved list
2. **If mismatch**: Enter read-only mode (all POST/PUT/DELETE blocked except `/safety`)
3. **Override**: POST `{"action": "override"}` to unlock until restart

## Request (POST)
```json
{"action": "override"}
```

## Response (GET)
```json
{
  "status": "success",
  "locked": true,
  "compatible": false,
  "zfs_version": {
    "full_version": "2.0.3-1ubuntu6",
    "semantic_version": "2.0.3",
    "major": 2, "minor": 0, "patch": 3,
    "detection_method": "zfs version"
  },
  "agent_version": "0.5.8",
  "approved_versions": ["2.1", "2.2", "2.3"],
  "lock_reason": "ZFS version 2.0.3 is not in the approved list"
}
```

## Locked Response (Mutating Endpoint)
```json
{
  "status": "error",
  "message": "Safety lock active: ZFS version 2.0.3 is not approved. Use POST /v1/safety to override.",
  "locked": true
}
```

## Implementation
- **Detection**: `zfs version` -> `modinfo -F version zfs` -> `/sys/module/zfs/version`
- **Matching**: Prefix matching ("2.1" matches "2.1.5")
- **State**: Memory-only, resets on restart

## Files
- `src/safety.rs` - SafetyManager, version detection
- `src/models.rs` - ZfsVersionInfo, SafetyState, request/response structs
- `src/utils.rs` - safety_check filter
- `src/handlers.rs` - safety_status_handler, safety_override_handler
- `src/main.rs` - initialization, route registration
