ACTIVE: #10_ClearPoolErrors
STATUS: planning
DEPENDS: #03_ZFSEngine
---

## Purpose
Clear error counters on a pool or specific device after repair/replacement.

## Planned Endpoints
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/pools/{name}/clear` | Clear all pool errors |
| POST | `/v1/pools/{name}/clear/{device}` | Clear errors on specific device |

## Command Reference
- `zpool clear <pool>` - Clear all device errors in pool
- `zpool clear <pool> <device>` - Clear errors on specific device

## Use Cases
1. After replacing a failed disk
2. After resolving transient I/O errors
3. Reset error counters for monitoring purposes
4. Bring pool back to ONLINE state after DEGRADED recovery

## Implementation Options

### Option A: libzfs FFI (preferred)
- `zpool_clear()` function available in libzfs
- Signature: `int zpool_clear(zpool_handle_t *, const char *device, nvlist_t *rewind_policy)`
- Pass NULL for device to clear entire pool

### Option B: CLI fallback
- Shell out to `zpool clear <pool> [device]`
- Simple but less elegant

## Request/Response

### Request (POST /v1/pools/{name}/clear)
```json
{
  "device": "sda1"  // optional, omit to clear all
}
```

### Response
```json
{
  "success": true,
  "pool": "tank",
  "device": "sda1",  // or null if clearing all
  "message": "Error counters cleared"
}
```

## Error Conditions
| Code | Condition |
|------|-----------|
| 404 | Pool not found |
| 404 | Device not found in pool |
| 500 | Clear operation failed |

## TODO
- [ ] Verify zpool_clear() FFI binding availability
- [ ] Implement FFI wrapper in src/zfs_management/
- [ ] Add endpoint handler
- [ ] Add to OpenAPI spec
- [ ] Write tests
