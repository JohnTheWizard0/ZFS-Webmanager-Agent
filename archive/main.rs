mod models;
mod handlers;
mod zfs;
mod auth;
mod config;

use warp::Filter;
use std::sync::Arc;
use handlers::{SnapshotHandlers, DatasetHandlers};
use zfs::ZfsManager;
use config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::load()?;
    println!("API Key: {}", config.api_key);
    
    // Initialize ZFS manager
    let zfs_manager = ZfsManager::new()?;
    let zfs_manager = Arc::new(zfs_manager);
    
    // Create API key check filter
    let api_key_check = auth::api_key_filter(config.api_key.clone());
    
    // Initialize handlers
    let snapshot_handlers = SnapshotHandlers::new(Arc::clone(&zfs_manager));
    let dataset_handlers = DatasetHandlers::new(Arc::clone(&zfs_manager));
    
    // Combine routes
    let routes = snapshot_handlers.routes(api_key_check.clone())
        .or(dataset_handlers.routes(api_key_check));
    
    // Start server
    println!("Server starting on port 9876");
    warp::serve(routes).run(([0, 0, 0, 0], 9876)).await;
    
    Ok(())
}