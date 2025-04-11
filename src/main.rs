mod auth;
mod handlers;
mod models;
mod utils;
mod zfs_management;

use warp::{Filter, Rejection, Reply};
use std::sync::{Arc, RwLock};
use zfs_management::ZfsManager;
use models::LastAction;
use handlers::*; // Import all handlers
use auth::*;


//-----------------------------------------------------
// MAIN FUNCTION
//-----------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting ZFS Web Manager...");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    // Generate or read API key
    let api_key = get_or_create_api_key()?;
    println!("\nAPI Key: {}", api_key);

    // Initialize ZFS manager
    let zfs = ZfsManager::new()?;
    // Create a warp filter that injects the ZFS manager into route handlers
    let zfs = warp::any().map(move || zfs.clone());

    // In your main function, add this near where you create the ZfsManager:
    let last_action = Arc::new(RwLock::new(None::<LastAction>));

    // API key check filter - reusable middleware for authentication
    let api_key_check = warp::header::headers_cloned()
        .and(warp::any().map(move || api_key.clone()))
        .and_then(check_api_key);


    let health_routes = {
        let last_action_clone = last_action.clone();
        // GET /health - Health check endpoint
        warp::get()
            .and(warp::path("health"))
            .and(warp::path::end())
            .and(warp::any().map(move || last_action_clone.clone()))
            .and_then(health_check_handler)
    };

    // Define pool-related routes
    let pool_routes = {
        // GET /pools - List all pools
        let list = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::end())
            .and(with_action_tracking("list_pools", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|zfs: ZfsManager, _| list_pools_handler(zfs));

        // GET /pools/{name} - Get pool status
        let status = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(with_action_tracking("get_pool_status", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| get_pool_status_handler(name, zfs));

        // POST /pools - Create a new pool
        let create = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_action_tracking("create_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreatePool, zfs: ZfsManager, _| create_pool_handler(body, zfs));

        // DELETE /pools/{name} - Destroy a pool
        // Query parameter ?force=true can be used for forced destruction
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

        // Combine all pool routes
        list.or(status).or(create).or(destroy)
    };

    // Define snapshot-related routes
    let snapshot_routes = {
        // GET /snapshots/{dataset} - List snapshots
        let list = warp::get()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(with_action_tracking("list_snapshots", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())    
            .and_then(|dataset: String, zfs: ZfsManager, _| list_snapshots_handler(dataset, zfs));

        // POST /snapshots/{dataset} - Create snapshot
        let create = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(warp::body::json())
            .and(with_action_tracking("create_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|dataset: String, body: CreateSnapshot, zfs: ZfsManager, _| create_snapshot_handler(dataset, body, zfs));

        // DELETE /snapshots/{dataset}/{snapshot_name} - Delete snapshot
        let delete = warp::delete()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(warp::path::param())
            .and(with_action_tracking("delete_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|dataset: String, snapshot_name: String, zfs: ZfsManager, _| delete_snapshot_handler(dataset, snapshot_name, zfs));

        // Combine all snapshot routes
        list.or(create).or(delete)
    };

    // Define dataset-related routes
    let dataset_routes = {
        // GET /datasets/{pool} - List datasets
        let list = warp::get()
            .and(warp::path("datasets"))
            .and(warp::path::param())
            .and(with_action_tracking("list_datasets", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|pool: String, zfs: ZfsManager, _: ()| list_datasets_handler(pool, zfs));

        // DELETE /datasets/{name} - Delete dataset
        let delete = warp::delete()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(with_action_tracking("delete_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _: ()| delete_dataset_handler(tail.as_str().to_string(), zfs));

        // POST /datasets - Create dataset
        let create = warp::post()
            .and(warp::path("datasets"))
            .and(warp::body::json())
            .and(with_action_tracking("create_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreateDataset, zfs: ZfsManager, _: ()| create_dataset_handler(body, zfs));

        // Combine all dataset routes
        list.or(create).or(delete)
    };

    // Define command execution routes
    let command_routes = {
        // POST /command - Execute a Linux command
        let execute = warp::post()
            .and(warp::path("command"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(warp::any().map(move || last_action.clone()))
            .and(api_key_check.clone())
            .and_then(|body: CommandRequest, last_action: Arc<RwLock<Option<LastAction>>>, _| {
                execute_command_handler(body, last_action)
            });

        execute
    };

    // Combine all routes
    let routes = snapshot_routes
    .or(dataset_routes)
    .or(pool_routes)
    .or(health_routes)
    .or(command_routes);

    // Start the HTTP server
    println!("Server starting on port: 9876");
    warp::serve(routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}