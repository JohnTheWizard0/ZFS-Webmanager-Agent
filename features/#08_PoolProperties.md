ACTIVE: #08_PoolProperties
STATUS: planning
DEPENDS: #04_PoolCRUD
---

## Purpose
Get and set pool-level properties.

## Planned Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/pools/{name}/properties` | Get all pool properties |
| PUT | `/v1/pools/{name}/properties` | Set pool property |

## Command Reference
- `zpool get all <pool>`
- `zpool set <property>=<value> <pool>`

## Common Properties
| Property | Description |
|----------|-------------|
| ashift | Sector size exponent |
| autoexpand | Auto-expand on device resize |
| autoreplace | Auto-replace failed devices |
| comment | Pool comment |
| readonly | Read-only mode |

## TODO
- [ ] Research libzetta support for pool properties
- [ ] Design request/response format
- [ ] Implement endpoints
