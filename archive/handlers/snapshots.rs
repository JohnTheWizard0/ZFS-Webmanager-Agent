use warp::{Filter, Rejection, Reply};
use std::sync::Arc;
use crate::zfs::ZfsManager;
use crate::models::{CreateSnapshot, ListResponse, success_response, error_response};

pub struct SnapshotHandlers {
    zfs: Arc<ZfsManager>,
}

impl SnapshotHandlers {
    pub fn new(zfs: Arc<ZfsManager>) -> Self {
        Self { zfs }
    }
    
    async fn list(&self, dataset: String) -> Result<impl Reply, Rejection> {
        match self.zfs.list_snapshots(&dataset).await {
            Ok(snapshots) => Ok(warp::reply::json(&ListResponse {
                snapshots,
                status: "success".to_string(),
            })),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    async fn create(&self, dataset: String, body: CreateSnapshot) -> Result<impl Reply, Rejection> {
        match self.zfs.create_snapshot(&dataset, &body.snapshot_name).await {
            Ok(_) => Ok(warp::reply::json(&success_response("Snapshot created successfully"))),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    async fn delete(&self, dataset: String, snapshot_name: String) -> Result<impl Reply, Rejection> {
        match self.zfs.delete_snapshot(&dataset, &snapshot_name).await {
            Ok(_) => Ok(warp::reply::json(&success_response("Snapshot deleted successfully"))),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    pub fn routes(&self, api_key_check: impl Filter<Extract = (), Error = Rejection> + Clone) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        let zfs_clone = Arc::clone(&self.zfs);
        
        // List snapshots: GET /snapshots/{dataset}
        let list = warp::path!("snapshots" / String)
            .and(warp::get())
            .and(api_key_check.clone())
            .map(move |dataset: String, _| {
                (dataset, Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|dataset, zfs| async move {
                SnapshotHandlers::new(zfs).list(dataset).await
            });
        
        // Create snapshot: POST /snapshots/{dataset}
        let zfs_clone = Arc::clone(&self.zfs);
        let create = warp::path!("snapshots" / String)
            .and(warp::post())
            .and(warp::body::json())
            .and(api_key_check.clone())
            .map(move |dataset: String, body: CreateSnapshot, _| {
                (dataset, body, Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|dataset, body, zfs| async move {
                SnapshotHandlers::new(zfs).create(dataset, body).await
            });
        
        // Delete snapshot: DELETE /snapshots/{dataset}/{snapshot_name}
        let zfs_clone = Arc::clone(&self.zfs);
        let delete = warp::path!("snapshots" / String / String)
            .and(warp::delete())
            .and(api_key_check.clone())
            .map(move |dataset: String, snapshot_name: String, _| {
                (dataset, snapshot_name, Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|dataset, snapshot_name, zfs| async move {
                SnapshotHandlers::new(zfs).delete(dataset, snapshot_name).await
            });
        
        list.or(create).or(delete)
    }
}