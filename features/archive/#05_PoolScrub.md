ACTIVE: #05_PoolScrub
STATUS: stable
DEPENDS: #04_PoolCRUD
---

## Purpose
Pool scrub operations: start, pause, stop, status.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/pools/{name}/scrub` | Start scrub |
| POST | `/v1/pools/{name}/scrub/pause` | Pause scrub |
| POST | `/v1/pools/{name}/scrub/stop` | Stop scrub |
| GET | `/v1/pools/{name}/scrub` | Get scrub status |

## Implementation
- Start/pause/stop: libzetta
- Status: FROM-SCRATCH libzfs FFI (libzetta doesn't expose progress)

## Status Response
```json
{
  "state": "scanning",
  "function": "scrub",
  "start_time": 1733526600,
  "percent_done": 45.2,
  "examined": 549755813888,
  "to_examine": 1099511627776,
  "errors": 0
}
```
