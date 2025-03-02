# ZFS Web Manager API

A RESTful API service for managing ZFS pools, datasets, and snapshots remotely. Built with Rust, Warp, and libzetta.

## Current State

The project implements a lightweight HTTP server exposing ZFS functionality through a clean REST API. It's designed to be a secure bridge between client applications and ZFS operations.

## Core Features

- **Authentication**: Simple API key-based authentication
- **Pool Management**: List, create, get status, and destroy ZFS pools
- **Dataset Operations**: Create, list, and delete datasets
- **Snapshot Handling**: Create, list, and delete snapshots
- **Health Monitoring**: Health check endpoint with version info and action tracking

## API Endpoints

### Health
- `GET /health` - Check server status and version

### Pools
- `GET /pools` - List all available pools
- `GET /pools/{name}` - Get detailed status for a specific pool
- `POST /pools` - Create a new pool with specified configuration
- `DELETE /pools/{name}` - Destroy a pool (use `?force=true` for forced destruction)

### Datasets
- `GET /datasets/{pool}` - List all datasets in a pool
- `POST /datasets` - Create a new dataset
- `DELETE /datasets/{name}` - Delete a dataset

### Snapshots
- `GET /snapshots/{dataset}` - List all snapshots for a dataset
- `POST /snapshots/{dataset}` - Create a new snapshot
- `DELETE /snapshots/{dataset}/{snapshot_name}` - Delete a specific snapshot

## Technical Implementation

- **Warp Framework**: Lightweight HTTP server with async request handling
- **libzetta Integration**: Uses both `DelegatingZfsEngine` and `ZpoolOpen3` for ZFS operations
- **Action Tracking**: Records the last performed action with timestamp
- **Error Handling**: Consistent error responses with informative messages

## Security

The API uses a simple token-based authentication system. On first run, it generates an API key stored in `.zfswm_api`, which must be included in requests via the `X-API-Key` header.

## Next Steps

- Implement property management for datasets
- Add support for ZFS replication and sending operations
- Create client libraries for various languages
- Add detailed documentation and OpenAPI specification
- Implement rate limiting and more advanced authentication

## Getting Started

1. Build the project with `cargo build --release`
2. Run the server with `./target/release/zfs-web-manager`
3. The API key will be displayed on first run
4. Use the API with your favorite HTTP client, including the API key in headers

```
curl -H "X-API-Key: YOUR_API_KEY" http://localhost:9876/health
```