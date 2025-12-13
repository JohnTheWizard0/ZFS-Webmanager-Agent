ACTIVE: #01_APIFramework
STATUS: perpetual
DEPENDS: none
---

## Purpose
Core HTTP server infrastructure using Actix-web. Route composition, request/response patterns, action tracking.

## Key Files
- `src/main.rs` — Server setup, route definitions
- `src/utils.rs` — Response helpers, action tracking
- `src/models.rs` — Request/response structs

## Current State
- Server: `0.0.0.0:9876`
- Framework: Actix-web (migrated from Warp)
- Response pattern: `{ "status": "success|error", "message": "...", ... }`

## TODO
- [ ] Custom error recovery handler
- [ ] CORS configuration
- [ ] Request logging middleware
- [ ] Rate limiting integration
