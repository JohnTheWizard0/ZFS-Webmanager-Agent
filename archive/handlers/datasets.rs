use warp::{Filter, Rejection, Reply, path::Tail};
use std::sync::Arc;
use crate::zfs::ZfsManager;
use crate::models::{CreateDataset, DatasetResponse, success_response, error_response};

pub struct DatasetHandlers {
    zfs: Arc<ZfsManager>,
}

impl DatasetHandlers {
    pub fn new(zfs: Arc<ZfsManager>) -> Self {
        Self { zfs }
    }
    
    async fn list(&self, pool: String) -> Result<impl Reply, Rejection> {
        match self.zfs.list_datasets(&pool).await {
            Ok(datasets) => Ok(warp::reply::json(&DatasetResponse {
                datasets,
                status: "success".to_string(),
            })),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    async fn create(&self, body: CreateDataset) -> Result<impl Reply, Rejection> {
        match self.zfs.create_dataset(body).await {
            Ok(_) => Ok(warp::reply::json(&success_response("Dataset created successfully"))),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    async fn delete(&self, name: String) -> Result<impl Reply, Rejection> {
        match self.zfs.delete_dataset(&name).await {
            Ok(_) => Ok(warp::reply::json(&success_response("Dataset deleted successfully"))),
            Err(e) => Ok(warp::reply::json(&error_response(&*e))),
        }
    }
    
    pub fn routes(&self, api_key_check: impl Filter<Extract = (), Error = Rejection> + Clone) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        let zfs_clone = Arc::clone(&self.zfs);
        
        // List datasets: GET /datasets/{pool}
        let list = warp::path!("datasets" / String)
            .and(warp::get())
            .and(api_key_check.clone())
            .map(move |pool: String, _| {
                (pool, Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|pool, zfs| async move {
                DatasetHandlers::new(zfs).list(pool).await
            });
        
        // Create dataset: POST /datasets
        let zfs_clone = Arc::clone(&self.zfs);
        let create = warp::path!("datasets")
            .and(warp::post())
            .and(warp::body::json())
            .and(api_key_check.clone())
            .map(move |body: CreateDataset, _| {
                (body, Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|body, zfs| async move {
                DatasetHandlers::new(zfs).create(body).await
            });
        
        // Delete dataset: DELETE /datasets/{name}
        let zfs_clone = Arc::clone(&self.zfs);
        let delete = warp::path("datasets")
            .and(warp::path::tail())
            .and(warp::delete())
            .and(api_key_check.clone())
            .map(move |tail: warp::path::Tail, _| {
                (tail.as_str().to_string(), Arc::clone(&zfs_clone))
            })
            .untuple_one()
            .and_then(|name, zfs| async move {
                DatasetHandlers::new(zfs).delete(name).await
            });
        
        list.or(create).or(delete)
    }
}