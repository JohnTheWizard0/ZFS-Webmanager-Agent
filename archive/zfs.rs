use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use libzetta::zfs::{
    DelegatingZfsEngine, 
    ZfsEngine,
    CreateDatasetRequest, 
    DatasetKind
};
use crate::models::CreateDataset;

#[derive(Clone)]
pub struct ZfsManager {
    engine: Arc<DelegatingZfsEngine>,
}

impl ZfsManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(ZfsManager {
            engine: Arc::new(DelegatingZfsEngine::new()?),
        })
    }

    pub async fn list_snapshots(&self, dataset: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let snapshots = self.engine.list_snapshots(dataset)?;
        Ok(snapshots
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    pub async fn create_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        self.engine.snapshot(&[full_path], None)?;
        Ok(())
    }

    pub async fn delete_snapshot(&self, dataset: &str, snapshot_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = PathBuf::from(format!("{}@{}", dataset, snapshot_name));
        self.engine.destroy(full_path)?;
        Ok(())
    }

    pub async fn list_datasets(&self, pool: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let datasets = self.engine.list_filesystems(pool)?;
        Ok(datasets
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    pub async fn create_dataset(&self, request: CreateDataset) -> Result<(), Box<dyn std::error::Error>> {
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

    pub async fn delete_dataset(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.engine.destroy(name)?;
        Ok(())
    }
}