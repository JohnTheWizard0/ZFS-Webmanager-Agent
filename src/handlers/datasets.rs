// handlers/datasets.rs
// Dataset handlers: list, create, delete, get/set properties

use crate::models::{
    ActionResponse, CreateDataset, DatasetPropertiesResponse, DatasetResponse, SetPropertyRequest,
};
use crate::utils::{error_response, success_response, validate_dataset_name};
use crate::zfs_management::ZfsManager;
use warp::{Rejection, Reply};

pub async fn list_datasets_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(success_response(DatasetResponse {
            status: "success".to_string(),
            datasets,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list datasets: {}", e))),
    }
}

pub async fn create_dataset_handler(
    body: CreateDataset,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Validate dataset name before attempting creation
    if let Err(msg) = validate_dataset_name(&body.name) {
        return Ok(error_response(&format!("Invalid dataset name: {}", msg)));
    }

    match zfs.create_dataset(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Dataset created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create dataset: {}", e))),
    }
}

pub async fn delete_dataset_handler(
    name: String,
    recursive: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = if recursive {
        zfs.delete_dataset_recursive(&name).await
    } else {
        zfs.delete_dataset(&name).await
    };

    match result {
        Ok(_) => {
            let msg = if recursive {
                format!("Dataset '{}' and all children deleted successfully", name)
            } else {
                format!("Dataset '{}' deleted successfully", name)
            };
            Ok(success_response(ActionResponse {
                status: "success".to_string(),
                message: msg,
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to delete dataset: {}", e))),
    }
}

// =========================================================================
// Dataset Properties Handlers
// =========================================================================

/// Get all properties of a dataset
pub async fn get_dataset_properties_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_dataset_properties(&name).await {
        Ok(props) => Ok(success_response(DatasetPropertiesResponse {
            status: "success".to_string(),
            properties: props,
        })),
        Err(e) => Ok(error_response(&format!(
            "Failed to get dataset properties: {}",
            e
        ))),
    }
}

/// Set a property on a dataset
/// **EXPERIMENTAL**: Uses CLI (`zfs set`) as libzetta/libzfs FFI lacks property setting.
pub async fn set_dataset_property_handler(
    name: String,
    body: SetPropertyRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .set_dataset_property(&name, &body.property, &body.value)
        .await
    {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Property '{}' set to '{}' on dataset '{}'",
                body.property, body.value, name
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to set property: {}", e))),
    }
}
