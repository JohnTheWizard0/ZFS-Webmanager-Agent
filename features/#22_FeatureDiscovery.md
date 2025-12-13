ACTIVE: #22_FeatureDiscovery
STATUS: stable
DEPENDS: #01_APIFramework
---

## Purpose
Visual dashboard of all ZFS features with implementation status.

## Endpoint
| Method | Path | Auth |
|--------|------|------|
| GET | `/v1/features` | No |

## Query Parameters
- `format=json` — Return JSON instead of HTML

## Response (JSON)
```json
{
  "features": [
    {
      "name": "List Pools",
      "category": "Pool Operations",
      "implementation": "libzetta",
      "status": "implemented"
    }
  ]
}
```

## Implementation Badges
| Badge | Color | Meaning |
|-------|-------|---------|
| Libzetta | blue | Native Rust bindings |
| FFI | purple | FROM-SCRATCH C bindings |
| CLI | amber | Experimental CLI fallback |
| Planned | gray | Not yet implemented |

## Maintenance
Feature list in `src/models.rs:ZfsFeaturesResponse::build()`.
Keep synchronized with actual implementation.
