ACTIVE: #13_DatasetEncryption
STATUS: planning
DEPENDS: #10_DatasetCRUD
---

## Purpose
Encryption key management for encrypted datasets.

## Planned Endpoints
| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/datasets/{path}/load-key` | Load encryption key |
| POST | `/v1/datasets/{path}/unload-key` | Unload encryption key |
| POST | `/v1/datasets/{path}/change-key` | Change encryption key |

## Command Reference
- `zfs load-key <dataset>`
- `zfs unload-key <dataset>`
- `zfs change-key <dataset>`

## Considerations
- Key must be loaded before dataset can be mounted/accessed
- Replication of encrypted datasets requires `-w` (raw) flag
- Cannot use `-F` (force) on encrypted receive

## TODO
- [ ] Research libzetta/FFI support
- [ ] Design key input mechanism (file path, stdin, prompt)
- [ ] Implement endpoints
