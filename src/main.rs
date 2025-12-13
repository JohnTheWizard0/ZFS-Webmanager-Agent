mod auth;
mod handlers;
mod models;
mod safety;
mod task_manager;
mod utils;
mod zfs_management;

use auth::*;
use handlers::*;
use models::{
    AddVdevRequest, ClearPoolRequest, CloneSnapshotRequest, CommandRequest, CreateDataset,
    CreatePool, CreateSnapshot, DeleteDatasetQuery, ExportPoolRequest, ImportPoolRequest,
    LastAction, ReceiveSnapshotRequest, ReplicateSnapshotRequest, RollbackRequest,
    SafetyOverrideRequest, SendSizeQuery, SendSnapshotRequest, SetPropertyRequest,
};
use safety::SafetyManager;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, RwLock};
use task_manager::TaskManager;
use utils::{safety_check, with_action_tracking, SafetyLockError};
use warp::{http::StatusCode, Filter, Rejection, Reply};
use zfs_management::ZfsManager;

/// Custom rejection handler for API errors
/// Converts ApiKeyError and SafetyLockError rejections into proper HTTP responses
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    // Handle safety lock first - return HTTP 200 with locked status per requirement
    if let Some(e) = err.find::<SafetyLockError>() {
        let json = warp::reply::json(&serde_json::json!({
            "status": "error",
            "message": e.0,
            "locked": true
        }));
        return Ok(warp::reply::with_status(json, StatusCode::OK));
    }

    let (code, message) = if let Some(e) = err.find::<ApiKeyError>() {
        match e {
            ApiKeyError::Missing => (StatusCode::UNAUTHORIZED, "Unauthorized: API key required"),
            ApiKeyError::Invalid => (StatusCode::UNAUTHORIZED, "Unauthorized: Invalid API key"),
        }
    } else if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Endpoint not found")
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed")
    } else if err.find::<warp::reject::InvalidHeader>().is_some() {
        (StatusCode::BAD_REQUEST, "Invalid header")
    } else if err.find::<warp::body::BodyDeserializeError>().is_some() {
        (StatusCode::BAD_REQUEST, "Invalid request body")
    } else {
        // Log unexpected rejections for debugging
        eprintln!("Unhandled rejection: {:?}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
    };

    let json = warp::reply::json(&serde_json::json!({
        "status": "error",
        "message": message
    }));

    Ok(warp::reply::with_status(json, code))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting ZFS Web Manager...");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Initialize safety manager (detects ZFS version)
    let safety_manager = SafetyManager::new()?;
    let safety_state = safety_manager.get_state();
    let settings = safety_manager.get_settings();

    // Log ZFS version and safety status
    println!(
        "ZFS Version: {} (detected via {})",
        safety_state.zfs_version.full_version, safety_state.zfs_version.detection_method
    );
    println!(
        "Approved range: {} - {}",
        settings.min_zfs_version, settings.max_zfs_version
    );

    if safety_state.locked {
        println!("WARNING: Safety lock ACTIVE - ZFS version {} not in approved range",
            safety_state.zfs_version.semantic_version);
        println!("         Use POST /v1/safety to override");
    } else {
        println!("Safety: ZFS version approved, all operations permitted");
    }

    // Load API key from credentials directory (never printed to console - SEC-03)
    let api_key = get_or_create_api_key()?;

    // Initialize ZFS manager
    let zfs = ZfsManager::new()?;
    let zfs = warp::any().map(move || zfs.clone());

    // Initialize action tracking
    let last_action = Arc::new(RwLock::new(None::<LastAction>));

    // Initialize task manager for async replication operations
    let task_manager = TaskManager::new();
    let task_mgr = warp::any().map(move || task_manager.clone());

    // Safety check filter for mutating routes
    let safety_filter = safety_check(safety_manager.clone());

    // Helper to inject SafetyManager
    let with_safety = {
        let sm = safety_manager.clone();
        warp::any().map(move || sm.clone())
    };

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
    // GET /v1/docs - Swagger UI (or ?format=json for lean API spec)
    // GET /v1/openapi.json - OpenAPI spec (generated from api.json)
    let docs_route = warp::get()
        .and(warp::path("docs"))
        .and(warp::path::end())
        .and(warp::query::<HashMap<String, String>>())
        .and_then(|query: HashMap<String, String>| {
            let format = query.get("format").cloned();
            docs_handler(format)
        });

    let openapi_route = warp::get()
        .and(warp::path("openapi.json"))
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

    // Safety routes
    // GET /v1/safety - Get safety status (no auth - must be accessible when locked)
    // POST /v1/safety - Override safety lock (requires auth - SEC-05)
    let safety_routes = {
        let get_status = warp::get()
            .and(warp::path("safety"))
            .and(warp::path::end())
            .and(with_safety.clone())
            .and_then(safety_status_handler);

        let override_lock = warp::post()
            .and(warp::path("safety"))
            .and(warp::path::end())
            .and(warp::body::json())
            .and(with_safety.clone())
            .and(api_key_check.clone()) // SEC-05: Require auth for safety override
            .and_then(|body: SafetyOverrideRequest, sm: SafetyManager, _| {
                safety_override_handler(body, sm)
            });

        get_status.or(override_lock)
    };

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
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("create_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreatePool, zfs: ZfsManager, _| create_pool_handler(body, zfs));

        let destroy = warp::delete()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path::end())
            .and(safety_filter.clone())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_action_tracking("delete_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |name: String, query: HashMap<String, String>, zfs: ZfsManager, _| {
                    let force = query.get("force").map(|v| v == "true").unwrap_or(false);
                    destroy_pool_handler(name, force, zfs)
                },
            );

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
            .and(safety_filter.clone())
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
            .and(safety_filter.clone())
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
            .and(safety_filter.clone())
            .and(with_action_tracking("stop_scrub", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, zfs: ZfsManager, _| stop_scrub_handler(name, zfs));

        let scrub_status = warp::get()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("scrub"))
            .and(warp::path::end())
            .and(with_action_tracking(
                "get_scrub_status",
                last_action.clone(),
            ))
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
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("export_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |name: String, body: ExportPoolRequest, zfs: ZfsManager, _| {
                    export_pool_handler(name, body, zfs)
                },
            );

        let list_importable = warp::get()
            .and(warp::path("pools"))
            .and(warp::path("importable"))
            .and(warp::path::end())
            .and(warp::query::<HashMap<String, String>>())
            .and(with_action_tracking(
                "list_importable_pools",
                last_action.clone(),
            ))
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
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("import_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: ImportPoolRequest, zfs: ZfsManager, _| import_pool_handler(body, zfs));

        // Vdev operations
        // POST /pools/{name}/vdev - add vdev to pool
        let add_vdev = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("vdev"))
            .and(warp::path::end())
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("add_vdev", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, body: AddVdevRequest, zfs: ZfsManager, _| {
                add_vdev_handler(name, body, zfs)
            });

        // DELETE /pools/{name}/vdev/{device_path...} - remove vdev from pool
        // Uses path::tail() to capture device paths with slashes (e.g., /dev/sda -> dev/sda)
        let remove_vdev = warp::delete()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("vdev"))
            .and(warp::path::tail())
            .and(safety_filter.clone())
            .and(with_action_tracking("remove_vdev", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |name: String, tail: warp::path::Tail, zfs: ZfsManager, _| {
                    // Reconstruct device path by prepending /
                    let device = format!("/{}", tail.as_str());
                    remove_vdev_handler(name, device, zfs)
                },
            );

        // POST /pools/{name}/clear - clear pool errors
        let clear_pool = warp::post()
            .and(warp::path("pools"))
            .and(warp::path::param())
            .and(warp::path("clear"))
            .and(warp::path::end())
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("clear_pool", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|name: String, body: ClearPoolRequest, zfs: ZfsManager, _| {
                clear_pool_handler(name, body, zfs)
            });

        // IMPORTANT: Route order matters for warp path matching!
        // - list_importable (GET /pools/importable) MUST come BEFORE status (GET /pools/{param})
        // - import_pool (POST /pools/import) MUST come BEFORE create (POST /pools + body)
        list.or(list_importable)
            .or(status)
            .or(import_pool)
            .or(create)
            .or(destroy)
            .or(scrub_start)
            .or(scrub_pause)
            .or(scrub_stop)
            .or(scrub_status)
            .or(export_pool)
            .or(add_vdev)
            .or(remove_vdev)
            .or(clear_pool)
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
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _| {
                list_snapshots_handler(tail.as_str().to_string(), zfs)
            });

        // Create snapshot route - check path BEFORE consuming body
        // to avoid body consumption issues with other routes
        let create = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                let path = tail.as_str();
                // Reject paths that belong to other routes BEFORE consuming body
                if path.ends_with("/clone")
                    || path.ends_with("/send")
                    || path.ends_with("/replicate")
                    || path.ends_with("/send-size")
                {
                    Err(warp::reject::not_found())
                } else {
                    Ok(tail)
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("create_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, body: CreateSnapshot, zfs: ZfsManager, _| async move {
                    create_snapshot_handler(tail.as_str().to_string(), body, zfs).await
                },
            );

        let delete = warp::delete()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(safety_filter.clone())
            .and(with_action_tracking("delete_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, zfs: ZfsManager, _| {
                delete_snapshot_by_path_handler(tail.as_str().to_string(), zfs)
            });

        // Clone route: POST /snapshots/{dataset}/{snapshot}/clone
        // IMPORTANT: Check path suffix BEFORE consuming body
        let clone = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                if tail.as_str().ends_with("/clone") {
                    Ok(tail)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("clone_snapshot", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: CloneSnapshotRequest, zfs: ZfsManager, _| async move {
                let path = tail.as_str();
                let snapshot_path = path.strip_suffix("/clone").unwrap();
                clone_snapshot_handler(snapshot_path.to_string(), body, zfs).await
            });

        // IMPORTANT: Route order for warp body consumption
        // clone checks for /clone suffix - if not matched, falls through
        // create handles all other POST /snapshots paths
        list.or(clone).or(create).or(delete)
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
            .and(with_action_tracking(
                "get_dataset_properties",
                last_action.clone(),
            ))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, zfs: ZfsManager, _: ()| async move {
                    let path = tail.as_str();
                    // Check if path ends with /properties
                    if let Some(dataset) = path.strip_suffix("/properties") {
                        get_dataset_properties_handler(dataset.to_string(), zfs).await
                    } else {
                        // Reject so other routes can match
                        Err(warp::reject::not_found())
                    }
                },
            );

        // PUT /datasets/{name}/properties - set a dataset property
        // **EXPERIMENTAL**: Uses CLI as FFI lacks property setting
        // IMPORTANT: Check path suffix BEFORE consuming body to avoid body consumption conflicts
        let set_property = warp::put()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                if tail.as_str().ends_with("/properties") {
                    Ok(tail)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("set_dataset_property", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|tail: warp::path::Tail, body: SetPropertyRequest, zfs: ZfsManager, _: ()| async move {
                let path = tail.as_str();
                let dataset = path.strip_suffix("/properties").unwrap();
                set_dataset_property_handler(dataset.to_string(), body, zfs).await
            });

        // POST /datasets/{path}/promote - promote a clone to independent dataset
        let promote = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(safety_filter.clone())
            .and(with_action_tracking("promote_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, zfs: ZfsManager, _: ()| async move {
                    let path = tail.as_str();
                    // Check if path ends with /promote
                    if let Some(clone_path) = path.strip_suffix("/promote") {
                        promote_dataset_handler(clone_path.to_string(), zfs).await
                    } else {
                        Err(warp::reject::not_found())
                    }
                },
            );

        // POST /datasets/{path}/rollback - rollback dataset to a snapshot
        // IMPORTANT: Check path suffix BEFORE consuming body to avoid body consumption conflicts
        let rollback = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                if tail.as_str().ends_with("/rollback") {
                    Ok(tail)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking(
                "rollback_dataset",
                last_action.clone(),
            ))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, body: RollbackRequest, zfs: ZfsManager, _: ()| async move {
                    let path = tail.as_str();
                    let dataset_path = path.strip_suffix("/rollback").unwrap();
                    rollback_dataset_handler(dataset_path.to_string(), body, zfs).await
                },
            );

        let delete = warp::delete()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and(safety_filter.clone())
            .and(warp::query::<DeleteDatasetQuery>())
            .and(with_action_tracking("delete_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, query: DeleteDatasetQuery, zfs: ZfsManager, _: ()| {
                    delete_dataset_handler(tail.as_str().to_string(), query.recursive, zfs)
                },
            );

        let create = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::end())
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(with_action_tracking("create_dataset", last_action.clone()))
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(|body: CreateDataset, zfs: ZfsManager, _: ()| {
                create_dataset_handler(body, zfs)
            });

        // IMPORTANT: create must come before promote/rollback because it uses path::end()
        // while promote/rollback use path::tail() with body::json() which would consume the body
        create
            .or(list)
            .or(get_properties)
            .or(set_property)
            .or(promote)
            .or(rollback)
            .or(delete)
    };

    // Command routes
    let command_routes = {
        warp::post()
            .and(warp::path("command"))
            .and(warp::path::end())
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(warp::any().map(move || last_action.clone()))
            .and(api_key_check.clone())
            .and_then(
                |body: CommandRequest, last_action: Arc<RwLock<Option<LastAction>>>, _| {
                    execute_command_handler(body, last_action)
                },
            )
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
            .and_then(|task_id: String, tm: TaskManager, _| get_task_status_handler(task_id, tm));

        // GET /v1/snapshots/{dataset}/{snapshot}/send-size - Estimate send size
        let send_size = warp::get()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and(warp::query::<SendSizeQuery>())
            .and(zfs.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail, query: SendSizeQuery, zfs: ZfsManager, _| async move {
                    let path = tail.as_str();
                    // Check if path ends with /send-size
                    if let Some(snapshot_path) = path.strip_suffix("/send-size") {
                        send_size_handler(snapshot_path.to_string(), query, zfs).await
                    } else {
                        Err(warp::reject::not_found())
                    }
                },
            );

        // POST /v1/snapshots/{dataset}/{snapshot}/send - Send snapshot to file
        // IMPORTANT: Check path suffix BEFORE consuming body to avoid body consumption issues
        let send_snapshot = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                // Check path BEFORE consuming body
                if tail.as_str().ends_with("/send") {
                    Ok(tail)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail,
                 body: SendSnapshotRequest,
                 zfs: ZfsManager,
                 tm: TaskManager,
                 _| async move {
                    let path = tail.as_str();
                    let snapshot_path = path.strip_suffix("/send").unwrap(); // Safe: checked above
                    send_snapshot_handler(snapshot_path.to_string(), body, zfs, tm).await
                },
            );

        // POST /v1/datasets/{path}/receive - Receive snapshot from file
        // IMPORTANT: Check path suffix BEFORE consuming body
        let receive_snapshot = warp::post()
            .and(warp::path("datasets"))
            .and(warp::path::tail())
            .and_then(|tail: warp::path::Tail| async move {
                if tail.as_str().ends_with("/receive") {
                    Ok(tail)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail,
                 body: ReceiveSnapshotRequest,
                 zfs: ZfsManager,
                 tm: TaskManager,
                 _| async move {
                    let path = tail.as_str();
                    let dataset_path = path.strip_suffix("/receive").unwrap();
                    receive_snapshot_handler(dataset_path.to_string(), body, zfs, tm).await
                },
            );

        // POST /v1/snapshots/{dataset}/{snapshot}/replicate - Replicate to another pool
        // Uses a separate base path to avoid body consumption conflict with /send
        let replicate_snapshot = warp::post()
            .and(warp::path("replication"))
            .and(warp::path::tail())
            .and(safety_filter.clone())
            .and(warp::body::json())
            .and(zfs.clone())
            .and(task_mgr.clone())
            .and(api_key_check.clone())
            .and_then(
                |tail: warp::path::Tail,
                 body: ReplicateSnapshotRequest,
                 zfs: ZfsManager,
                 tm: TaskManager,
                 _| async move {
                    let path = tail.as_str();
                    // Path format: dataset/snapshot (e.g., "backuppool/222")
                    replicate_snapshot_handler(path.to_string(), body, zfs, tm).await
                },
            );

        get_status
            .or(send_size)
            .or(send_snapshot)
            .or(receive_snapshot)
            .or(replicate_snapshot)
    };

    // Catch-all 404 route for non-existent endpoints
    // Must be last in the route chain - matches any path not handled by other routes
    // IMPORTANT: Only return 404 for paths that don't match known endpoint prefixes.
    // This ensures auth rejections from valid endpoints propagate to the rejection handler
    // and return proper 401 responses instead of being caught here as 404.
    let not_found_route = warp::path::tail()
        .and_then(|tail: warp::path::Tail| async move {
            let path = tail.as_str();
            // Known endpoint prefixes - if path matches these, reject so auth errors propagate
            let known_prefixes = [
                "pools",
                "datasets",
                "snapshots",
                "tasks",
                "command",
                "replication",
                "health",
                "docs",
                "openapi.json",
                "features",
                "safety",
            ];

            // Check if path starts with any known prefix
            let matches_known = known_prefixes
                .iter()
                .any(|prefix| path == *prefix || path.starts_with(&format!("{}/", prefix)));

            if matches_known {
                // Path matches a known endpoint - let auth rejection propagate
                Err(warp::reject::not_found())
            } else {
                // Truly unknown path - we'll return 404
                Ok(())
            }
        })
        .map(|_| {
            warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "status": "error",
                    "message": "Endpoint not found"
                })),
                StatusCode::NOT_FOUND,
            )
        });

    // Combine all API routes under /v1
    // IMPORTANT: Route order matters for warp body consumption!
    // Routes with path::end() + body::json() must come BEFORE routes with path::tail() + body::json()
    // because path::tail() is greedy and will consume the body before path::end() routes can match.
    // Order:
    // 1. dataset_routes (POST /datasets with path::end()) before task_routes (POST /datasets/{path}/receive)
    // 2. snapshot_routes (POST /snapshots/{tail}) before task_routes (POST /snapshots/{tail}/send)
    // IMPORTANT: Route order for warp body consumption!
    // task_routes has specific paths like /snapshots/{path}/send and /snapshots/{path}/clone
    // These MUST come BEFORE snapshot_routes which has generic /snapshots/{path}
    // Otherwise the generic route consumes the body before specific routes can match.
    let v1_routes = warp::path("v1")
        .and(
            health_routes
                .or(docs_route)
                .or(openapi_route)
                .or(zfs_features_route)
                .or(safety_routes) // Safety routes (no auth, works when locked)
                .or(pool_routes)
                .or(dataset_routes)
                .or(task_routes) // BEFORE snapshot_routes (has /send, /replicate)
                .or(snapshot_routes) // Generic POST /snapshots/{path} last
                .or(command_routes)
                .or(not_found_route), // Catch-all 404 for unmatched paths (must be last)
        )
        .recover(handle_rejection);

    // Start server
    println!("Server starting on port: 9876");
    println!("API base URL: http://localhost:9876/v1");
    println!("API docs: http://localhost:9876/v1/docs");
    println!("ZFS features: http://localhost:9876/v1/features");
    warp::serve(v1_routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}
