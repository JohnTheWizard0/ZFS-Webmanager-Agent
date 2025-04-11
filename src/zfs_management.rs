//-----------------------------------------------------
// ZFS MANAGER IMPLEMENTATION
//-----------------------------------------------------

// zfs_management.rs
use std::sync::Arc;
use libzetta::zfs::{DelegatingZfsEngine, ZfsEngine};
use libzetta::zpool::{ZpoolEngine, ZpoolOpen3};
use std::path::PathBuf;

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
