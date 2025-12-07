mod auth;
mod handlers;
mod models;
mod utils;
mod zfs_management;

use warp::Filter;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use zfs_management::ZfsManager;
use models::{LastAction, CreatePool, CreateSnapshot, CreateDataset, CommandRequest, ExportPoolRequest, ImportPoolRequest};
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

        let create = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::body::json())
            .and(with_action_tracking("create_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: CreateSnapshot, zfs: ZfsManager, _| create_snapshot_handler(tail.as_str().to_string(), body, zfs));

        let delete = warp::delete()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(with_action_tracking("delete_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _| {
                delete_snapshot_by_path_handler(tail.as_str().to_string(), zfs)
            });

        list.or(create).or(delete)
    };

    // Dataset routes
    let dataset_routes = {
        let list = warp::get()
            .and(warp::path("datasets"))
            .and(warp::path::param())
            .and(with_action_tracking("list_datasets", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|pool: String, zfs: ZfsManager, _: ()| list_datasets_handler(pool, zfs));

        let delete = warp::delete()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(with_action_tracking("delete_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _: ()| delete_dataset_handler(tail.as_str().to_string(), zfs));

        let create = warp::post()
            .and(warp::path("datasets"))
            .and(warp::body::json())
            .and(with_action_tracking("create_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreateDataset, zfs: ZfsManager, _: ()| create_dataset_handler(body, zfs));

        list.or(create).or(delete)
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

    // Combine all API routes under /v1
    let v1_routes = warp::path("v1").and(
        health_routes
            .or(docs_route)
            .or(openapi_route)
            .or(pool_routes)
            .or(snapshot_routes)
            .or(dataset_routes)
            .or(command_routes)
    );

    // Start server
    println!("Server starting on port: 9876");
    println!("API base URL: http://localhost:9876/v1");
    println!("API docs: http://localhost:9876/v1/docs");
    warp::serve(v1_routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}