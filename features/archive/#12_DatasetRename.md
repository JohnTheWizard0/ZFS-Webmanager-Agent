ACTIVE: #12_DatasetRename
STATUS: stable
DEPENDS: #10_DatasetCRUD
---

## Purpose
Rename datasets.

## Endpoint
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/datasets/{path}/rename` | Rename dataset |

## Request
```json
{
  "new_name": "tank/newname"
}
```

## Implementation
FROM-SCRATCH using `lzc_rename()` FFI.

libzetta doesn't expose rename functionality.
