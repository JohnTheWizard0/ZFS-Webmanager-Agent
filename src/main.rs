// Import required modules from the Warp web framework for creating HTTP endpoints
use warp::{Filter, Rejection, Reply};
// Import serialization/deserialization functionality
use serde::{Deserialize, Serialize};
// Import ZFS related functionality from libzetta
use libzetta::zfs::{
    DelegatingZfsEngine, ZfsEngine, CreateDatasetRequest, DatasetKind
};
// Import additional ZPool functionality from libzetta
use libzetta::zpool::{
    ZpoolEngine, ZpoolOpen3, CreateZpoolRequest, CreateVdevRequest, 
    CreateMode, DestroyMode
};
// Standard library imports
use std::sync::Arc;
use tokio;
use std::path::PathBuf;
use std::collections::HashMap;
use warp::http::HeaderMap;
use std::fs;
use std::io::Write;
use rand::Rng;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::Infallible;


//-----------------------------------------------------
// DATA MODELS
//-----------------------------------------------------

// Define a struct to track the last action
#[derive(Clone, Serialize)]
struct LastAction {
    function: String,
    timestamp: u64,
}

// Define the health response struct
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    last_action: Option<LastAction>,
}

// Response structures
#[derive(Serialize)]
struct ListResponse {
    snapshots: Vec<String>,
    status: String,
}

#[derive(Serialize)]
struct ActionResponse {
    status: String,
    message: String,
}

#[derive(Deserialize)]
struct CreateSnapshot {
    snapshot_name: String,
}

// Dataset structures
#[derive(Deserialize)]
struct CreateDataset {
    name: String,
    kind: String,  // "filesystem" or "volume"
    properties: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
struct DatasetResponse {
    datasets: Vec<String>,
    status: String,
}

// Pool status response
#[derive(Serialize)]
struct PoolStatus {
    name: String,
    health: String,
    size: u64,
    allocated: u64,
    free: i64,         // Changed from u64 to i64
    capacity: u8,       // Changed from u64 to u8
    vdevs: u32,
    errors: Option<String>,
}

// Pool list response
#[derive(Serialize)]
struct PoolListResponse {
    pools: Vec<String>,
    status: String,
}

// Pool creation request
#[derive(Deserialize)]
struct CreatePool {
    name: String,
    disks: Vec<String>,
    raid_type: Option<String>, // "mirror", "raidz", "raidz2", "raidz3", or null for individual disks
}

// Helper functions for response generation
fn success_response(message: &str) -> ActionResponse {
    ActionResponse {
        status: "success".to_string(),
        message: message.to_string(),
    }
}

fn error_response(error: &dyn std::error::Error) -> ActionResponse {
    ActionResponse {
        status: "error".to_string(),
        message: error.to_string(),
    }
}

// Create a middleware filter that tracks actions
fn with_action_tracking(
    action_name: &'static str,
    action_tracker: Arc<RwLock<Option<LastAction>>>,
) -> impl Filter<Extract = (), Error = Infallible> + Clone {
    // Clone the Arc outside the closure so it's moved into the filter
    let tracker = action_tracker.clone();
    
    warp::any()
        .map(move || {
            // Now use the tracker that was cloned outside the closure
            if let Ok(mut last_action) = tracker.write() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                *last_action = Some(LastAction {
                    function: action_name.to_string(),
                    timestamp: now,
                });
            }
        })
        .untuple_one()
}


//-----------------------------------------------------
// ZFS MANAGER IMPLEMENTATION
//-----------------------------------------------------

#[derive(Clone)]
struct ZfsManager {
    engine: Arc<DelegatingZfsEngine>,
}

impl ZfsManager {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(ZfsManager {
            engine: Arc::new(DelegatingZfsEngine::new()?),
        })
    }

    // List all available pools
    async fn list_pools(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Use ZpoolOpen3 for zpool operations
        let zpool_engine = ZpoolOpen3::default();
        
        // Get all pools with status
        let pools = zpool_engine.status_all(Default::default())?;
        
        // Extract pool names
        let pool_names = pools.into_iter()
            .map(|pool| pool.name().clone())
            .collect();
            
        Ok(pool_names)
    }

    // Fixed get_pool_status method with proper type handling
    async fn get_pool_status(&self, name: &str) -> Result<PoolStatus, Box<dyn std::error::Error>> {
        let zpool_engine = ZpoolOpen3::default();
        
        // Check if pool exists
        if !zpool_engine.exists(name)? {
            return Err(format!("Pool '{}' not found", name).into());
        }
        
        // Get detailed status
        let pool = zpool_engine.status(name, Default::default())?;
        
        // Get pool properties for size information
        let properties = zpool_engine.read_properties(name)?;
        
        // Convert to our response format
        Ok(PoolStatus {
            name: pool.name().clone(),
            health: format!("{:?}", pool.health()),
            // Dereference the values before casting
            size: *properties.size() as u64,
            allocated: *properties.alloc() as u64,
            free: *properties.free() as i64,  // Cast u64 to i64
            capacity: *properties.capacity() as u8,  // Cast u64 to u8
            vdevs: pool.vdevs().len() as u32,
            errors: pool.errors().clone(),
        })
    }

    // Create a new pool
    async fn create_pool(&self, request: CreatePool) -> Result<(), Box<dyn std::error::Error>> {
        let zpool_engine = ZpoolOpen3::default();
        
        // Check if pool already exists
        if zpool_engine.exists(&request.name)? {
            return Err(format!("Pool '{}' already exists", request.name).into());
        }
        
        // Convert disks to vdevs based on configuration
        let vdevs = match request.raid_type.as_deref() {
            Some("mirror") => {
                if request.disks.len() < 2 {
                    return Err("Mirror requires at least 2 disks".into());
                }
                vec![CreateVdevRequest::Mirror(request.disks.iter().map(PathBuf::from).collect())]
            },
            Some("raidz") => {
                if request.disks.len() < 3 {
                    return Err("RAIDZ requires at least 3 disks".into());
                }
                vec![CreateVdevRequest::RaidZ(request.disks.iter().map(PathBuf::from).collect())]
            },
            Some("raidz2") => {
                if request.disks.len() < 4 {
                    return Err("RAIDZ2 requires at least 4 disks".into());
                }
                vec![CreateVdevRequest::RaidZ2(request.disks.iter().map(PathBuf::from).collect())]
            },
            Some("raidz3") => {
                if request.disks.len() < 5 {
                    return Err("RAIDZ3 requires at least 5 disks".into());
                }
                vec![CreateVdevRequest::RaidZ3(request.disks.iter().map(PathBuf::from).collect())]
            },
            _ => {
                // Default to individual disks (no raid)
                request.disks.iter()
                    .map(|disk| CreateVdevRequest::SingleDisk(PathBuf::from(disk)))
                    .collect()
            }
        };

        // Build the create request
        let create_request = CreateZpoolRequest::builder()
            .name(request.name)
            .vdevs(vdevs)
            .create_mode(CreateMode::Gentle) // Can be overridden to Force if needed
            .build()?;
            
        // Create the pool
        zpool_engine.create(create_request)?;
        
        Ok(())
    }

    // Destroy an existing pool
    async fn destroy_pool(&self, name: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
        let zpool_engine = ZpoolOpen3::default();
        
        // Check if pool exists
        if !zpool_engine.exists(name)? {
            return Err(format!("Pool '{}' not found", name).into());
        }
        
        // Determine destroy mode based on force parameter
        let mode = if force {
            DestroyMode::Force
        } else {
            DestroyMode::Gentle
        };
        
        // Destroy the pool
        zpool_engine.destroy(name, mode)?;
        
        Ok(())
    }

    async fn list_snapshots(&self, dataset: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let snapshots = self.engine.list_snapshots(dataset)?;
        Ok(snapshots
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    async fn create_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        self.engine.snapshot(&[full_path], None)?;
        Ok(())
    }

    async fn delete_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        self.engine.destroy(full_path)?;
        Ok(())
    }

    async fn list_datasets(&self, pool: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let datasets = self.engine.list_filesystems(pool)?;
        Ok(datasets
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    async fn create_dataset(&self, request: CreateDataset) -> Result<(), Box<dyn std::error::Error>> {
        let kind = match request.kind.to_lowercase().as_str() {
            "filesystem" => DatasetKind::Filesystem,
            "volume" => DatasetKind::Volume,
            _ => return Err("Invalid dataset kind. Must be 'filesystem' or 'volume'".into()),
        };

        let dataset_request = CreateDatasetRequest::builder()
            .name(PathBuf::from(request.name))
            .kind(kind)
            .user_properties(request.properties)
            .build()?;

        self.engine.create(dataset_request)?;
        Ok(())
    }    

    async fn delete_dataset(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.engine.destroy(name)?;
        Ok(())
    }
}

//-----------------------------------------------------
// AUTHENTICATION
//-----------------------------------------------------

// Custom error type for API key validation failures
#[derive(Debug)]
struct ApiKeyError;
impl warp::reject::Reject for ApiKeyError {}

// Function to get an existing API key or create a new one
fn get_or_create_api_key() -> Result<String, Box<dyn std::error::Error>> {
    let file_path = ".zfswm_api";
    if let Ok(api_key) = fs::read_to_string(file_path) {
        Ok(api_key.trim().to_string())
    } else {
        let api_key: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let mut file = fs::File::create(file_path)?;
        file.write_all(api_key.as_bytes())?;
        Ok(api_key)
    }
}

// Check if the API key is valid
async fn check_api_key(headers: HeaderMap, our_api_key: String) -> Result<(), Rejection> {
    match headers.get("X-API-Key") {
        Some(key) if key.to_str().map(|s| s == our_api_key).unwrap_or(false) => Ok(()),
        _ => Err(warp::reject::custom(ApiKeyError)),
    }
}

//-----------------------------------------------------
// POOL HANDLERS
//-----------------------------------------------------

// List all pools
async fn list_pools_handler(
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_pools().await {
        Ok(pools) => Ok(warp::reply::json(&PoolListResponse {
            pools,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Get pool status
async fn get_pool_status_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_pool_status(&name).await {
        Ok(status) => Ok(warp::reply::json(&status)),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Create a new pool
async fn create_pool_handler(
    body: CreatePool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_pool(body).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Pool created successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Destroy a pool
async fn destroy_pool_handler(
    name: String,
    force: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.destroy_pool(&name, force).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Pool destroyed successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

//-----------------------------------------------------
// SNAPSHOT HANDLERS
//-----------------------------------------------------

async fn list_snapshots_handler(
    dataset: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_snapshots(&dataset).await {
        Ok(snapshots) => Ok(warp::reply::json(&ListResponse {
            snapshots,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

async fn create_snapshot_handler(
    dataset: String,
    body: CreateSnapshot,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_snapshot(&dataset, &body.snapshot_name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Snapshot created successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

async fn delete_snapshot_handler(
    dataset: String,
    snapshot_name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_snapshot(&dataset, &snapshot_name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Snapshot deleted successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

//-----------------------------------------------------
// DATASET HANDLERS
//-----------------------------------------------------

async fn list_datasets_handler(
    pool: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(warp::reply::json(&DatasetResponse {
            datasets,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

async fn create_dataset_handler(
    body: CreateDataset,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_dataset(body).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Dataset created successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

async fn delete_dataset_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_dataset(&name).await {
        Ok(_) => Ok(warp::reply::json(&success_response("Dataset deleted successfully"))),
        Err(e) => Ok(warp::reply::json(&error_response(&*e))),
    }
}

//-----------------------------------------------------
// MISC HANDLERS
//-----------------------------------------------------

// Health check handler
async fn health_check_handler(
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let last_action_data = if let Ok(action) = last_action.read() {
        action.clone()
    } else {
        None
    };
    
    Ok(warp::reply::json(&HealthResponse {
        status: "ok".to_string(),
        version,
        last_action: last_action_data,
    }))
}

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

    // Combine all routes
    let routes = snapshot_routes.or(dataset_routes).or(pool_routes).or(health_routes);

    // Start the HTTP server
    println!("Server starting on port: 9876");
    warp::serve(routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}