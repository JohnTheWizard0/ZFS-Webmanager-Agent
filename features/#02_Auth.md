ACTIVE: #02_Auth
STATUS: perpetual
DEPENDS: #01_APIFramework
---

## Purpose
API key-based authentication. Generates UUID v4 key on first run, validates via `X-API-Key` header.

## Key Files
- `src/auth.rs` — Key generation, validation, error types
- `~/.config/zfs_webmanager/api_key.txt` — Stored key

## Endpoints
| Path | Auth Required |
|------|---------------|
| `GET /health` | No |
| `GET /v1/features` | No |
| All other `/v1/*` | Yes |

## Current State
- Key format: UUID v4
- Key displayed on startup
- Header: `X-API-Key: <uuid>`

## TODO
- [ ] Key rotation mechanism
- [ ] Rate limiting
- [ ] Multiple API keys support
- [ ] Key scopes/permissions
