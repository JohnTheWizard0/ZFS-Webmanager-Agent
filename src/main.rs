mod auth;
mod handlers;
mod models;
mod task_manager;
mod utils;
mod zfs_management;

use warp::Filter;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use zfs_management::ZfsManager;
use task_manager::TaskManager;
use models::{LastAction, CreatePool, CreateSnapshot, CreateDataset, CommandRequest, ExportPoolRequest, ImportPoolRequest, SetPropertyRequest, CloneSnapshotRequest, RollbackRequest, SendSizeQuery, SendSnapshotRequest, ReceiveSnapshotRequest, ReplicateSnapshotRequest};
use handlers::*;
use auth::*;
use utils::with_action_tracking;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting ZFS Web Manager...");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    
    // Generate or read API key
    let api_key = get_or_create_api_key()?;
    println!("\nAPI Key: {}", api_key);

    // Initialize ZFS manager
    let zfs = ZfsManager::new()?;
    let zfs = warp::any().map(move || zfs.clone());

    // Initialize action tracking
    let last_action = Arc::new(RwLock::new(None::<LastAction>));

    // Initialize task manager for async replication operations
    let task_manager = TaskManager::new();
    let task_mgr = warp::any().map(move || task_manager.clone());

    // API key check filter
    let api_key_check = warp::header::headers_cloned()
        .and(warp::any().map(move || api_key.clone()))
        .and_then(check_api_key);

    // Health route (no auth required)
    let health_routes = {
        let last_action_clone = last_action.clone();
        warp::get()
            .and(warp::path("health"))
            .and(warp::path::end())
            .and(warp::any().map(move || last_action_clone.clone()))
            .and_then(health_check_handler)
    };

    // OpenAPI docs routes (no auth required)
    // GET /v1/docs - Swagger UI
    // GET /v1/openapi.yaml - OpenAPI spec
    let docs_route = warp::get()
        .and(warp::path("docs"))
        .and(warp::path::end())
        .and_then(docs_handler);

    let openapi_route = warp::get()
        .and(warp::path("openapi.yaml"))
        .and(warp::path::end())
        .and_then(openapi_handler);

    // ZFS features discovery route (no auth required)
    // GET /v1/features - List all features and implementation status
    // Returns HTML by default, JSON if ?format=json
    let zfs_features_route = warp::get()
        .and(warp::path("features"))
        .and(warp::path::end())
        .and(warp::query::<HashMap<String, String>>())
        .and_then(|query: HashMap<String, String>| {
            let format = query.get("format").cloned();
            zfs_features_handler(format)
        });

    // Pool routes
    let pool_routes = {
        let list = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::end())
            .and(with_action_tracking("list_pools", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|zfs: ZfsManager, _| list_pools_handler(zfs));

        let status = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(with_action_tracking("get_pool_status", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| get_pool_status_handler(name, zfs));

        let create = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_action_tracking("create_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreatePool, zfs: ZfsManager, _| create_pool_handler(body, zfs));

        let destroy = warp::delete()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_action_tracking("delete_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())    
            .and_then(|name: String, query: HashMap<String, String>, zfs: ZfsManager, _| {
                let force = query.get("force").map(|v| v == "true").unwrap_or(false);
                destroy_pool_handler(name, force, zfs)
            });

        // Scrub routes (nested under pools)
        // POST /pools/{name}/scrub - start scrub
        // POST /pools/{name}/scrub/pause - pause scrub
        // POST /pools/{name}/scrub/stop - stop scrub
        // GET /pools/{name}/scrub - get scrub status
        let scrub_start = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("scrub"))
            .and(warp::path::end())
            .and(with_action_tracking("start_scrub", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| start_scrub_handler(name, zfs));

        let scrub_pause = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("scrub"))
            .and(warp::path("pause"))
            .and(warp::path::end())
            .and(with_action_tracking("pause_scrub", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| pause_scrub_handler(name, zfs));

        let scrub_stop = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("scrub"))
            .and(warp::path("stop"))
            .and(warp::path::end())
            .and(with_action_tracking("stop_scrub", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| stop_scrub_handler(name, zfs));

        let scrub_status = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("scrub"))
            .and(warp::path::end())
            .and(with_action_tracking("get_scrub_status", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| get_scrub_status_handler(name, zfs));

        // Import/Export routes
        // POST /pools/{name}/export - export a pool
        // GET /pools/importable - list importable pools
        // POST /pools/import - import a pool
        let export_pool = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("export"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_action_tracking("export_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, body: ExportPoolRequest, zfs: ZfsManager, _| {
                export_pool_handler(name, body, zfs)
            });

        let list_importable = warp::get()
            .and(warp::path("pools"))
            .and(warp::path("importable"))
            .and(warp::path::end())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_action_tracking("list_importable_pools", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|query: HashMap<String, String>, zfs: ZfsManager, _| {
                let dir = query.get("dir").cloned();
                list_importable_pools_handler(dir, zfs)
            });

        let import_pool = warp::post()
            .and(warp::path("pools"))
            .and(warp::path("import"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_action_tracking("import_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: ImportPoolRequest, zfs: ZfsManager, _| {
                import_pool_handler(body, zfs)
            });

        list.or(status).or(create).or(destroy)
            .or(scrub_start).or(scrub_pause).or(scrub_stop).or(scrub_status)
            .or(export_pool).or(list_importable).or(import_pool)
    };

    // Snapshot routes
    // Uses path::tail() to support dataset paths with slashes (e.g., pool/dataset)
    // - GET /snapshots/pool/dataset → list snapshots
    // - POST /snapshots/pool/dataset → create snapshot (name in body)
    // - DELETE /snapshots/pool/dataset/snapshot_name → delete snapshot
    let snapshot_routes = {
        let list = warp::get()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(with_action_tracking("list_snapshots", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _| list_snapshots_handler(tail.as_str().to_string(), zfs));

        // Create must come before clone to avoid body consumption issues
        // Reject if path ends with /clone (that's the clone route)
        let create = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_action_tracking("create_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: CreateSnapshot, zfs: ZfsManager, _| async move {
                let path = tail.as_str();
                // Reject if this is actually a clone request
                if path.ends_with("/clone") {
                    return Err(warp::reject::not_found());
                }
                create_snapshot_handler(path.to_string(), body, zfs).await
            });

        let delete = warp::delete()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(with_action_tracking("delete_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _| {
                delete_snapshot_by_path_handler(tail.as_str().to_string(), zfs)
            });

        // Clone route: POST /snapshots/{dataset}/{snapshot}/clone
        // Creates a writable clone from a snapshot (MF-003 Phase 3)
        let clone = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_action_tracking("clone_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: CloneSnapshotRequest, zfs: ZfsManager, _| async move {
                let path = tail.as_str();
                // Check if path ends with /clone
                if let Some(snapshot_path) = path.strip_suffix("/clone") {
                    clone_snapshot_handler(snapshot_path.to_string(), body, zfs).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // IMPORTANT: create must come before clone to avoid body consumption issues
        list.or(create).or(clone).or(delete)
    };

    // Dataset routes
    let dataset_routes = {
        let list = warp::get()
            .and(warp::path("datasets"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(with_action_tracking("list_datasets", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|pool: String, zfs: ZfsManager, _: ()| list_datasets_handler(pool, zfs));

        // GET /datasets/{name}/properties - get dataset properties
        // Matches paths like /datasets/pool/properties or /datasets/pool/child/properties
        let get_properties = warp::get()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(with_action_tracking("get_dataset_properties", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _: ()| async move {
                let path = tail.as_str();
                // Check if path ends with /properties
                if let Some(dataset) = path.strip_suffix("/properties") {
                    get_dataset_properties_handler(dataset.to_string(), zfs).await
                } else {
                    // Reject so other routes can match
                    Err(warp::reject::not_found())
                }
            });

        // PUT /datasets/{name}/properties - set a dataset property
        // **EXPERIMENTAL**: Uses CLI as FFI lacks property setting
        let set_property = warp::put()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_action_tracking("set_dataset_property", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: SetPropertyRequest, zfs: ZfsManager, _: ()| async move {
                let path = tail.as_str();
                // Check if path ends with /properties
                if let Some(dataset) = path.strip_suffix("/properties") {
                    set_dataset_property_handler(dataset.to_string(), body, zfs).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // POST /datasets/{path}/promote - promote a clone to independent dataset
        // FROM-SCRATCH implementation using lzc_promote() FFI (MF-003 Phase 3)
        let promote = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(with_action_tracking("promote_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _: ()| async move {
                let path = tail.as_str();
                // Check if path ends with /promote
                if let Some(clone_path) = path.strip_suffix("/promote") {
                    promote_dataset_handler(clone_path.to_string(), zfs).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // POST /datasets/{path}/rollback - rollback dataset to a snapshot
        // FROM-SCRATCH implementation using lzc_rollback_to() FFI (MF-003 Phase 3)
        // Safety levels: default (most recent only), force_destroy_newer, force_destroy_clones
        let rollback = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_action_tracking("rollback_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: RollbackRequest, zfs: ZfsManager, _: ()| async move {
                let path = tail.as_str();
                // Check if path ends with /rollback
                if let Some(dataset_path) = path.strip_suffix("/rollback") {
                    rollback_dataset_handler(dataset_path.to_string(), body, zfs).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        let delete = warp::delete()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(with_action_tracking("delete_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _: ()| delete_dataset_handler(tail.as_str().to_string(), zfs));

        let create = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_action_tracking("create_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreateDataset, zfs: ZfsManager, _: ()| create_dataset_handler(body, zfs));

        // IMPORTANT: create must come before promote/rollback because it uses path::end()
        // while promote/rollback use path::tail() with body::json() which would consume the body
        create.or(list).or(get_properties).or(set_property).or(promote).or(rollback).or(delete)
    };

    // Command routes
    let command_routes = {
        warp::post()
            .and(warp::path("command"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(warp::any().map(move || last_action.clone()))
            .and(api_key_check.clone())
            .and_then(|body: CommandRequest, last_action: Arc<RwLock<Option<LastAction>>>, _| {
                execute_command_handler(body, last_action)
            })
    };

    // Task routes (for async replication operations)
    // GET /v1/tasks/{task_id} - Get task status
    let task_routes = {
        let get_status = warp::get()
            .and(warp::path("tasks"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(|task_id: String, tm: TaskManager, _| {
                get_task_status_handler(task_id, tm)
            });

        // GET /v1/snapshots/{dataset}/{snapshot}/send-size - Estimate send size
        let send_size = warp::get()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::query::<SendSizeQuery>())
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, query: SendSizeQuery, zfs: ZfsManager, _| async move {
                let path = tail.as_str();
                // Check if path ends with /send-size
                if let Some(snapshot_path) = path.strip_suffix("/send-size") {
                    send_size_handler(snapshot_path.to_string(), query, zfs).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // POST /v1/snapshots/{dataset}/{snapshot}/send - Send snapshot to file
        let send_snapshot = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: SendSnapshotRequest, zfs: ZfsManager, tm: TaskManager, _| async move {
                let path = tail.as_str();
                // Check if path ends with /send
                if let Some(snapshot_path) = path.strip_suffix("/send") {
                    send_snapshot_handler(snapshot_path.to_string(), body, zfs, tm).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // POST /v1/datasets/{path}/receive - Receive snapshot from file
        let receive_snapshot = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: ReceiveSnapshotRequest, zfs: ZfsManager, tm: TaskManager, _| async move {
                let path = tail.as_str();
                // Check if path ends with /receive
                if let Some(dataset_path) = path.strip_suffix("/receive") {
                    receive_snapshot_handler(dataset_path.to_string(), body, zfs, tm).await
                } else {
                    Err(warp::reject::not_found())
                }
            });

        // POST /v1/snapshots/{dataset}/{snapshot}/replicate - Replicate to another pool
        // Uses a separate base path to avoid body consumption conflict with /send
        let replicate_snapshot = warp::post()
            .and(warp::path("replication"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: ReplicateSnapshotRequest, zfs: ZfsManager, tm: TaskManager, _| async move {
                let path = tail.as_str();
                // Path format: dataset/snapshot (e.g., "backuppool/222")
                replicate_snapshot_handler(path.to_string(), body, zfs, tm).await
            });

        get_status.or(send_size).or(send_snapshot).or(receive_snapshot).or(replicate_snapshot)
    };

    // Combine all API routes under /v1
    // IMPORTANT: Route order matters for warp body consumption!
    // Routes with path::end() + body::json() must come BEFORE routes with path::tail() + body::json()
    // because path::tail() is greedy and will consume the body before path::end() routes can match.
    // Order:
    // 1. dataset_routes (POST /datasets with path::end()) before task_routes (POST /datasets/{path}/receive)
    // 2. snapshot_routes (POST /snapshots/{tail}) before task_routes (POST /snapshots/{tail}/send)
    let v1_routes = warp::path("v1").and(
        health_routes
            .or(docs_route)
            .or(openapi_route)
            .or(zfs_features_route)
            .or(pool_routes)
            .or(dataset_routes)
            .or(snapshot_routes)
            .or(task_routes)
            .or(command_routes)
    );

    // Start server
    println!("Server starting on port: 9876");
    println!("API base URL: http://localhost:9876/v1");
    println!("API docs: http://localhost:9876/v1/docs");
    println!("ZFS features: http://localhost:9876/v1/features");
    warp::serve(v1_routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}