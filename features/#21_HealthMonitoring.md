ACTIVE: #21_HealthMonitoring
STATUS: stable
DEPENDS: #01_APIFramework
---

## Purpose
Health check endpoint for monitoring and load balancers.

## Endpoint
| Method | Path | Auth |
|--------|------|------|
| GET | `/health` | No |

## Response
```json
{
  "status": "success",
  "version": "0.5.7",
  "last_action": {
    "function": "list_pools",
    "timestamp": 1733526600
  }
}
```

## Notes
- No authentication required
- Timestamp is Unix epoch seconds
- `last_action` is null if no authenticated requests yet
- Action tracking updates on every authenticated request
