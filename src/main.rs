use warp::{Filter, Rejection, Reply};
use serde::{Deserialize, Serialize};
use libzetta::zfs::{
    DelegatingZfsEngine, 
    ZfsEngine,
    CreateDatasetRequest, 
    DatasetKind
};
use std::sync::Arc;
use tokio;
use std::path::PathBuf;
use std::collections::HashMap;  // Add this at the top with other imports

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

// Request/Response structures for datasets
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

// ZFS wrapper to make it easier to share between routes
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

    // List snapshots for a dataset
    async fn list_snapshots(&self, dataset: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let snapshots = self.engine.list_snapshots(dataset)?;
        Ok(snapshots
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    // Create a new snapshot
    async fn create_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        self.engine.snapshot(&[full_path], None)?;
        Ok(())
    }

    // Delete a snapshot
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

// Route handlers
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
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Snapshot created successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

async fn delete_snapshot_handler(
    dataset: String,
    snapshot_name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_snapshot(&dataset, &snapshot_name).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Snapshot deleted successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Route handlers for datasets
async fn list_datasets_handler(
    pool: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(warp::reply::json(&DatasetResponse {
            datasets,
            status: "success".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

async fn create_dataset_handler(
    body: CreateDataset,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_dataset(body).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Dataset created successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

async fn delete_dataset_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.delete_dataset(&name).await {
        Ok(_) => Ok(warp::reply::json(&ActionResponse {
            status: "success".to_string(),
            message: "Dataset deleted successfully".to_string(),
        })),
        Err(e) => Ok(warp::reply::json(&ActionResponse {
            status: "error".to_string(),
            message: e.to_string(),
        })),
    }
}

// Main function
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ZFS manager
    let zfs = ZfsManager::new()?;
    let zfs = warp::any().map(move || zfs.clone());

    // Define routes
    // Snapshot routes (your existing routes)
    let snapshot_routes = {
        let list = warp::get()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(zfs.clone())
            .and_then(list_snapshots_handler);

        let create = warp::post()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(warp::body::json())
            .and(zfs.clone())
            .and_then(create_snapshot_handler);

        let delete = warp::delete()
            .and(warp::path("snapshots"))
            .and(warp::path::param())
            .and(warp::path::param())
            .and(zfs.clone())
            .and_then(delete_snapshot_handler);

        list.or(create).or(delete)
    };

    let dataset_routes = {
        let list = warp::get()
            .and(warp::path("datasets"))
            .and(warp::path::param())
            .and(zfs.clone())
            .and_then(list_datasets_handler);
    
        // New delete route implementation
        let delete = warp::delete()
        .and(warp::path("datasets"))
        .and(warp::path::tail())  // This captures everything after /datasets/
        .and(zfs.clone())
        .and_then(|tail: warp::path::Tail, zfs: ZfsManager| {
            delete_dataset_handler(tail.as_str().to_string(), zfs)
        });
    
        let create = warp::post()
            .and(warp::path("datasets"))
            .and(warp::body::json())
            .and(zfs.clone())
            .and_then(create_dataset_handler);
    
        list.or(create).or(delete)
    };

    // Combine all routes
    let routes = snapshot_routes.or(dataset_routes);

    println!("Server starting on port 9876");
    warp::serve(routes).run(([0, 0, 0, 0], 9876)).await;

    Ok(())
}