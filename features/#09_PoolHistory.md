ACTIVE: #09_PoolHistory
STATUS: planning
DEPENDS: #04_PoolCRUD
---

## Purpose
Retrieve pool command history for auditing.

## Planned Endpoint
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/pools/{name}/history` | Get pool command history |

## Command Reference
- `zpool history <pool>`
- `zpool history -i <pool>` (internal events)
- `zpool history -l <pool>` (long format with user/host)

## Response Format (proposed)
```json
{
  "status": "success",
  "pool": "tank",
  "history": [
    {
      "timestamp": "2025-12-10T10:30:00Z",
      "command": "zpool create tank mirror /dev/sda /dev/sdb"
    }
  ]
}
```

## TODO
- [ ] Research libzetta support
- [ ] Determine if CLI fallback needed
- [ ] Implement endpoint
