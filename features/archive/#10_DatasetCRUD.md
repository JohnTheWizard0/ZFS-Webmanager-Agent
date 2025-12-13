ACTIVE: #10_DatasetCRUD
STATUS: stable
DEPENDS: #01_APIFramework, #02_Auth, #03_ZFSEngine
---

## Purpose
Basic dataset operations: list, create, delete.

## Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/datasets/{pool}` | List datasets in pool |
| POST | `/v1/datasets` | Create dataset |
| DELETE | `/v1/datasets/{path}` | Delete dataset |

## Dataset Kinds
| Kind | Notes |
|------|-------|
| filesystem | Default |
| volume | Requires `volsize` property |

## Create Request
```json
{
  "name": "tank/data",
  "kind": "filesystem",
  "properties": {
    "compression": "lz4"
  }
}
```

## Implementation
All via libzetta. Zvol creation uses `volume_size()` builder method.

## Tests
- `tests/api_datasets.rs`
- `tests/zfs_parcour.sh`
