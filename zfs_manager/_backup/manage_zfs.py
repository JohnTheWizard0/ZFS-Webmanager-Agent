# manage_zfs.py

import logging
from zfs.remote_zfs import ZFSRemote, ZFSConfig, DatasetKind, OperationError

def main():
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Configure ZFS client
    config = ZFSConfig(
        host="192.168.7.100",
        port=9876,
        timeout=30
    )

    # Initialize client
    zfs = ZFSRemote(config)

    try:
        # Create a dataset with compression enabled
        logger.info("Creating dataset with lz4 compression...")
        zfs.create_dataset(
            name="primarypool/TESTZFS",
            kind=DatasetKind.FILESYSTEM
        )
        
        zfs.set_properties(
            "primarypool/TESTZFS",
            {
                "compression": "zstd",
                "atime": "off"  # Just an example
            }
        )
        
        logger.info("Dataset created successfully")

        # List datasets to verify
        logger.info("Listing datasets in 'primarypool'...")
        datasets = zfs.list_datasets("primarypool")
        logger.info("Found datasets: %s", datasets)

    except OperationError as e:
        logger.error("ZFS operation failed: %s", e)
    except ConnectionError as e:
        logger.error("Connection failed: %s", e)
    except Exception as e:
        logger.exception("Unexpected error occurred")

if __name__ == "__main__":
    main()
