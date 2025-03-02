# This is the library that communicates directly with the rust agent is used by other python applications!
# For example "manage_zfs.py"
# !!! MADE FOR VERSION 0.3.0 OF THE RUST AGENT !!!

import requests
from typing import Optional, Dict, Any, List, Tuple
from urllib.parse import quote
from dataclasses import dataclass
from enum import Enum
import logging
import os

class DatasetKind(Enum):
    FILESYSTEM = "filesystem"
    VOLUME = "volume"

class RaidType(Enum):
    SINGLE = None
    MIRROR = "mirror"
    RAIDZ = "raidz"
    RAIDZ2 = "raidz2"
    RAIDZ3 = "raidz3"

class ZFSError(Exception):
    """Base exception for ZFS operations"""
    pass

class ConnectionError(ZFSError):
    """Raised when connection to remote host fails"""
    pass

class OperationError(ZFSError):
    """Raised when a ZFS operation fails"""
    pass

class AuthenticationError(ZFSError):
    """Raised when API key authentication fails"""
    pass

@dataclass
class ZFSConfig:
    """Configuration for ZFS remote connection"""
    host: str
    port: int = 9876
    timeout: int = 30
    verify_ssl: bool = True
    api_key: Optional[str] = None

@dataclass
class PoolStatus:
    """Status information about a pool"""
    name: str
    health: str
    size: int
    allocated: int
    free: int
    capacity: int
    vdevs: int
    errors: Optional[str] = None

class ZFSRemote:
    """Client for remote ZFS management"""
    
    def __init__(self, config: ZFSConfig):
        """Initialize ZFS remote client
        
        Args:
            config: ZFSConfig object with connection details
        """
        self.config = config
        self.base_url = f"http://{config.host}:{config.port}"
        self.session = requests.Session()
        self.session.verify = config.verify_ssl
        
        # Set up API key authentication
        if config.api_key:
            self.api_key = config.api_key
        elif os.environ.get("ZFS_API_KEY"):
            self.api_key = os.environ.get("ZFS_API_KEY")
        else:
            self.api_key = None
            logging.warning("No API key provided. Authentication will likely fail.")
        
        self.logger = logging.getLogger(__name__)

    def _make_request(self, method: str, endpoint: str, **kwargs) -> Dict[str, Any]:
        """Make HTTP request to remote ZFS server
        
        Args:
            method: HTTP method to use
            endpoint: API endpoint
            **kwargs: Additional arguments for requests
            
        Returns:
            Response data as dictionary
            
        Raises:
            ConnectionError: If connection fails
            AuthenticationError: If API key authentication fails
            OperationError: If operation fails
        """
        url = f"{self.base_url}/{endpoint.lstrip('/')}"
        self.logger.debug(f"Making {method} request to {url}")
        if 'json' in kwargs:
            self.logger.debug(f"Request payload: {kwargs['json']}")

        # Add API key header
        headers = kwargs.get('headers', {})
        if self.api_key:
            headers['X-API-Key'] = self.api_key
        kwargs['headers'] = headers

        try:
            kwargs.setdefault('timeout', self.config.timeout)
            response = self.session.request(
                method,
                url,
                **kwargs
            )
            
            # Handle authentication errors
            if response.status_code == 401 or response.status_code == 403:
                raise AuthenticationError("API key authentication failed")
                
            response.raise_for_status()
            return response.json()
        except AuthenticationError:
            raise
        except requests.exceptions.ConnectionError as e:
            raise ConnectionError(f"Failed to connect to {self.base_url}: {e}")
        except requests.exceptions.RequestException as e:
            error_msg = e.response.text if hasattr(e, 'response') and hasattr(e.response, 'text') else str(e)
            raise OperationError(f"Operation failed: {error_msg}")

    #-------------------------------------------------
    # Misc Methods
    #-------------------------------------------------

    def check_health(self) -> dict:
        try:
            response = self._make_request('GET', 'health')
            self.logger.debug(f"Health check result: {response}")
            return response
        except OperationError as e:
            # Convert operation errors during health checks to connection errors
            # since they likely indicate the service is not working properly
            raise ConnectionError(f"Health check failed: {e}")

    #-------------------------------------------------
    # Pool Management Methods
    #-------------------------------------------------
    
    def list_pools(self) -> List[str]:
        """List all available pools
        
        Returns:
            List of pool names
        """
        response = self._make_request('GET', 'pools')
        return response.get("pools", [])
    
    def get_pool_status(self, name: str) -> PoolStatus:
        """Get detailed status for a pool
        
        Args:
            name: Pool name
            
        Returns:
            PoolStatus object with status information
        """
        response = self._make_request('GET', f'pools/{quote(name)}')
        if response.get("status") == "error":
            raise OperationError(response.get("message", "Failed to get pool status"))
            
        return PoolStatus(
            name=response.get("name", name),
            health=response.get("health", "UNKNOWN"),
            size=response.get("size", 0),
            allocated=response.get("allocated", 0),
            free=response.get("free", 0),
            capacity=response.get("capacity", 0),
            vdevs=response.get("vdevs", 0),
            errors=response.get("errors")
        )
    
    def create_pool(self, name: str, disks: List[str], raid_type: RaidType = RaidType.SINGLE) -> None:
        """Create a new pool
        
        Args:
            name: Pool name
            disks: List of disks to use
            raid_type: RAID configuration to use
        """
        payload = {
            "name": name,
            "disks": disks,
            "raid_type": raid_type.value
        }
        
        self._make_request('POST', 'pools', json=payload)
        self.logger.info(f"Created pool: {name} with {raid_type.name} configuration")
    
    def destroy_pool(self, name: str, force: bool = False) -> None:
        """Destroy a pool
        
        Args:
            name: Pool name
            force: Whether to force destruction even if the pool has datasets
        """
        self._make_request('DELETE', f'pools/{quote(name)}{"?force=true" if force else ""}')
        self.logger.info(f"Destroyed pool: {name}")
        
    #-------------------------------------------------
    # Dataset Management Methods
    #-------------------------------------------------
    
    def create_dataset(self, name: str, kind: DatasetKind = DatasetKind.FILESYSTEM,
                  properties: Optional[Dict[str, Any]] = None) -> None:
        """Create a new dataset
        
        Args:
            name: Dataset name to create
            kind: Type of dataset to create (FILESYSTEM or VOLUME)
            properties: Optional properties to set on the dataset
        """
        payload = {
             "name": name,
             "kind": kind.value,
             "properties": properties
        }
        self._make_request('POST', 'datasets', json=payload)
        self.logger.info(f"Created dataset: {name}")

    def list_datasets(self, pool: str) -> List[str]:
        """List all datasets in a pool
        
        Args:
            pool: Pool name
            
        Returns:
            List of dataset names
        """
        response = self._make_request('GET', f'datasets/{quote(pool)}')
        return response.get("datasets", [])

    def delete_dataset(self, name: str) -> None:
        """Delete a dataset
        
        Args:
            name: Dataset name to delete
        """
        self._make_request('DELETE', f'datasets/{quote(name)}')
        self.logger.info(f"Deleted dataset: {name}")
    
    def set_properties(self, dataset: str, properties: Dict[str, str]) -> None:
        """Set native ZFS properties on a dataset
        
        Args:
            dataset: Dataset name (e.g. 'pool/dataset')
            properties: Dictionary of property name/value pairs
                       Example: {'compression': 'lz4', 'atime': 'off'}
        """
        payload = {
            "name": dataset,
            "kind": "filesystem",
            "properties": properties
        }
        self._make_request('POST', f'datasets/{quote(dataset)}/properties', json=payload)
        self.logger.info(f"Set properties on {dataset}: {properties}")

    #-------------------------------------------------
    # Snapshot Management Methods
    #-------------------------------------------------
    
    def create_snapshot(self, dataset: str, snapshot_name: str) -> None:
        """Create a new snapshot
        
        Args:
            dataset: Dataset name
            snapshot_name: Name for the new snapshot
        """
        payload = {"snapshot_name": snapshot_name}
        self._make_request('POST', f'snapshots/{quote(dataset)}', json=payload)
        self.logger.info(f"Created snapshot: {dataset}@{snapshot_name}")

    def list_snapshots(self, dataset: str) -> List[str]:
        """List all snapshots for a dataset
        
        Args:
            dataset: Dataset name
            
        Returns:
            List of snapshot names
        """
        response = self._make_request('GET', f'snapshots/{quote(dataset)}')
        return response.get("snapshots", [])

    def delete_snapshot(self, dataset: str, snapshot_name: str) -> None:
        """Delete a snapshot
        
        Args:
            dataset: Dataset name
            snapshot_name: Snapshot name to delete
        """
        self._make_request('DELETE', f'snapshots/{quote(dataset)}/{quote(snapshot_name)}')
        self.logger.info(f"Deleted snapshot: {dataset}@{snapshot_name}")
