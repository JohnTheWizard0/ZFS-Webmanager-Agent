# manage_zfs.py

import logging
from zfs.remote_zfs import ZFSRemote, ZFSConfig, DatasetKind, OperationError, AuthenticationError

# Configuration variables
ZFS_HOST = "192.168.7.102"
ZFS_PORT = 9876
ZFS_API_KEY = "7GMsgWf6OHCVvYwEnxQzPd5qI9N7ZVR8"  # Replace with your actual API key

def main():
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Configure ZFS client
    config = ZFSConfig(
        host=ZFS_HOST,
        port=ZFS_PORT,
        timeout=30,
        api_key=ZFS_API_KEY
    )

    # Initialize client
    zfs = ZFSRemote(config)

    try:
        # Create a dataset with compression enabled
        logger.info("Creating dataset with lz4 compression...")
        zfs.create_dataset(
            name="test-pool/TESTZFS",
            kind=DatasetKind.FILESYSTEM
        )
        
        zfs.set_properties(
            "test-pool/TESTZFS",
            {
                "compression": "zstd",
                "atime": "off"  # Just an example
            }
        )
        
        logger.info("Dataset created successfully")

        # List datasets to verify
        logger.info("Listing datasets in 'test-pool'...")
        datasets = zfs.list_datasets("test-pool")
        logger.info("Found datasets: %s", datasets)

    except AuthenticationError as e:
        logger.error("Authentication failed: %s", e)
    except OperationError as e:
        logger.error("ZFS operation failed: %s", e)
    except ConnectionError as e:
        logger.error("Connection failed: %s", e)
    except Exception as e:
        logger.exception("Unexpected error occurred")

# Function to create a dataset
def create_zfs_dataset(name, properties=None):
    """
    Create a ZFS dataset with the given name and properties.
    
    Args:
        name (str): Name of the dataset to create
        properties (dict, optional): Dictionary of properties to set
        
    Returns:
        bool: True if successful, False otherwise
    """
    logger = logging.getLogger(__name__)
    
    # Configure ZFS client
    config = ZFSConfig(
        host=ZFS_HOST,
        port=ZFS_PORT,
        timeout=30,
        api_key=ZFS_API_KEY
    )
    
    zfs = ZFSRemote(config)
    
    try:
        zfs.create_dataset(
            name=name,
            kind=DatasetKind.FILESYSTEM
        )
        
        if properties:
            zfs.set_properties(name, properties)
            
        logger.info(f"Created dataset: {name}")
        return True
    except Exception as e:
        logger.error(f"Failed to create dataset {name}: {e}")
        return False

# Function to list datasets
def list_zfs_datasets(pool):
    """
    List all datasets in the specified pool.
    
    Args:
        pool (str): Name of the pool
        
    Returns:
        list: List of datasets, or empty list on error
    """
    logger = logging.getLogger(__name__)
    
    config = ZFSConfig(
        host=ZFS_HOST,
        port=ZFS_PORT,
        timeout=30,
        api_key=ZFS_API_KEY
    )
    
    zfs = ZFSRemote(config)
    
    try:
        datasets = zfs.list_datasets(pool)
        logger.info(f"Listed datasets in {pool}")
        return datasets
    except Exception as e:
        logger.error(f"Failed to list datasets in {pool}: {e}")
        return []

# Function to delete a dataset
def delete_zfs_dataset(name):
    """
    Delete the specified ZFS dataset.
    
    Args:
        name (str): Name of the dataset to delete
        
    Returns:
        bool: True if successful, False otherwise
    """
    logger = logging.getLogger(__name__)
    
    config = ZFSConfig(
        host=ZFS_HOST,
        port=ZFS_PORT,
        timeout=30,
        api_key=ZFS_API_KEY
    )
    
    zfs = ZFSRemote(config)
    
    try:
        zfs.delete_dataset(name)
        logger.info(f"Deleted dataset: {name}")
        return True
    except Exception as e:
        logger.error(f"Failed to delete dataset {name}: {e}")
        return False

# Function to create a snapshot
def create_zfs_snapshot(dataset, snapshot_name):
    """
    Create a snapshot of the specified dataset.
    
    Args:
        dataset (str): Name of the dataset
        snapshot_name (str): Name for the snapshot
        
    Returns:
        bool: True if successful, False otherwise
    """
    logger = logging.getLogger(__name__)
    
    config = ZFSConfig(
        host=ZFS_HOST,
        port=ZFS_PORT,
        timeout=30,
        api_key=ZFS_API_KEY
    )
    
    zfs = ZFSRemote(config)
    
    try:
        zfs.create_snapshot(dataset, snapshot_name)
        logger.info(f"Created snapshot: {dataset}@{snapshot_name}")
        return True
    except Exception as e:
        logger.error(f"Failed to create snapshot {dataset}@{snapshot_name}: {e}")
        return False

if __name__ == "__main__":
    main()
