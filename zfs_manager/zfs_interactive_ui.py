#!/usr/bin/env python3
# ZFS Interactive UI
# An interactive CLI tool to manage ZFS resources remotely

import os
import sys
import logging
import argparse
from typing import Dict, List, Optional, Any
from zfs.remote_zfs import (
    ZFSRemote, ZFSConfig, DatasetKind, RaidType, 
    PoolStatus, OperationError, AuthenticationError, ConnectionError
)

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class ZFSInteractiveUI:
    """Interactive UI for ZFS management"""
    
    def __init__(self, host: str, port: int, api_key: str):
        """Initialize the UI with connection details
        
        Args:
            host: ZFS server hostname or IP
            port: ZFS server port
            api_key: API key for authentication
        """
        self.config = ZFSConfig(
            host=host,
            port=port,
            api_key=api_key
        )
        self.zfs = ZFSRemote(self.config)
        self.current_pool = None
        self.current_dataset = None
        
    def display_menu(self, title: str, options: Dict[str, str], back_option: bool = True) -> str:
        """Display a menu and get user selection
        
        Args:
            title: Menu title
            options: Dictionary of option keys and descriptions
            back_option: Whether to include a back option
            
        Returns:
            Selected option key
        """
        while True:
            print("\n" + "=" * 60)
            print(f" {title} ".center(60, "="))
            print("=" * 60)
            
            # Display options
            for key, description in options.items():
                print(f"  {key}. {description}")
            
            # Add back option if requested
            if back_option:
                print("  b. Back to previous menu")
                print("  q. Quit")
            else:
                print("  q. Quit")
            
            # Get user selection
            choice = input("\nEnter your choice: ").strip().lower()
            
            if choice == 'q':
                print("Exiting ZFS management.")
                sys.exit(0)
            elif back_option and choice == 'b':
                return 'b'
            elif choice in options:
                return choice
            else:
                print("\nInvalid option. Please try again.")
    
    def main_menu(self) -> None:
        """Display the main menu"""
        options = {
            "1": "Pool Management",
            "2": "Dataset Management",
            "3": "Snapshot Management"
        }
        
        while True:
            choice = self.display_menu("ZFS Management Main Menu", options, back_option=False)
            
            if choice == '1':
                self.pool_menu()
            elif choice == '2':
                self.dataset_menu()
            elif choice == '3':
                self.snapshot_menu()
    
    def pool_menu(self) -> None:
        """Display the pool management menu"""
        options = {
            "1": "List Pools",
            "2": "Get Pool Status",
            "3": "Create Pool",
            "4": "Destroy Pool"
        }
        
        while True:
            choice = self.display_menu("Pool Management", options)
            
            if choice == 'b':
                return
            elif choice == '1':
                self.list_pools()
            elif choice == '2':
                self.get_pool_status()
            elif choice == '3':
                self.create_pool()
            elif choice == '4':
                self.destroy_pool()
    
    def dataset_menu(self) -> None:
        """Display the dataset management menu"""
        options = {
            "1": "List Datasets",
            "2": "Create Dataset",
            "3": "Delete Dataset",
            "4": "Set Dataset Properties"
        }
        
        while True:
            choice = self.display_menu("Dataset Management", options)
            
            if choice == 'b':
                return
            elif choice == '1':
                self.list_datasets()
            elif choice == '2':
                self.create_dataset()
            elif choice == '3':
                self.delete_dataset()
            elif choice == '4':
                self.set_dataset_properties()
    
    def snapshot_menu(self) -> None:
        """Display the snapshot management menu"""
        options = {
            "1": "List Snapshots",
            "2": "Create Snapshot",
            "3": "Delete Snapshot"
        }
        
        while True:
            choice = self.display_menu("Snapshot Management", options)
            
            if choice == 'b':
                return
            elif choice == '1':
                self.list_snapshots()
            elif choice == '2':
                self.create_snapshot()
            elif choice == '3':
                self.delete_snapshot()
    
    #-------------------------------------------------
    # Pool Management Functions
    #-------------------------------------------------
    
    def list_pools(self) -> None:
        """List all pools"""
        try:
            pools = self.zfs.list_pools()
            
            print("\nAvailable pools:")
            if pools:
                for i, pool in enumerate(pools, 1):
                    print(f"  {i}. {pool}")
                
                # Allow setting current pool
                choice = input("\nSelect a pool number to set as current pool (or Enter to skip): ").strip()
                if choice.isdigit() and 1 <= int(choice) <= len(pools):
                    self.current_pool = pools[int(choice) - 1]
                    print(f"Current pool set to: {self.current_pool}")
            else:
                print("  No pools found.")
        except Exception as e:
            print(f"Error listing pools: {e}")
    
    def get_pool_status(self) -> None:
        """Get status for a pool"""
        pool_name = self._get_pool_name("Enter pool name: ")
        if not pool_name:
            return
        
        try:
            status = self.zfs.get_pool_status(pool_name)
            
            print("\nPool Status:")
            print(f"  Name: {status.name}")
            print(f"  Health: {status.health}")
            print(f"  Size: {self._format_size(status.size)}")
            print(f"  Allocated: {self._format_size(status.allocated)} ({status.capacity}%)")
            print(f"  Free: {self._format_size(status.free)}")
            print(f"  Number of vdevs: {status.vdevs}")
            
            if status.errors:
                print(f"  Errors: {status.errors}")
        except Exception as e:
            print(f"Error getting pool status: {e}")
    
    def create_pool(self) -> None:
        """Create a new pool"""
        name = input("Enter pool name: ").strip()
        if not name:
            print("Pool name cannot be empty.")
            return
        
        # Get disks
        disks_input = input("Enter disk paths separated by commas: ").strip()
        if not disks_input:
            print("Disks cannot be empty.")
            return
        
        disks = [disk.strip() for disk in disks_input.split(',')]
        
        # Get RAID type
        print("\nAvailable RAID types:")
        print("  1. Single disks (no RAID)")
        print("  2. Mirror (RAID1)")
        print("  3. RAIDZ (RAID5-like)")
        print("  4. RAIDZ2 (RAID6-like)")
        print("  5. RAIDZ3 (Triple parity)")
        
        raid_choice = input("Select RAID type (1-5): ").strip()
        
        raid_type = RaidType.SINGLE
        if raid_choice == '2':
            raid_type = RaidType.MIRROR
        elif raid_choice == '3':
            raid_type = RaidType.RAIDZ
        elif raid_choice == '4':
            raid_type = RaidType.RAIDZ2
        elif raid_choice == '5':
            raid_type = RaidType.RAIDZ3
        
        try:
            print(f"Creating pool '{name}' with {raid_type.name} configuration...")
            self.zfs.create_pool(name, disks, raid_type)
            print(f"Pool '{name}' created successfully.")
            self.current_pool = name
        except Exception as e:
            print(f"Error creating pool: {e}")
    
    def destroy_pool(self) -> None:
        """Destroy a pool"""
        pool_name = self._get_pool_name("Enter pool name to destroy: ")
        if not pool_name:
            return
        
        confirm = input(f"Are you SURE you want to destroy pool '{pool_name}'? This is IRREVERSIBLE. (yes/no): ").strip().lower()
        if confirm != 'yes':
            print("Pool destruction cancelled.")
            return
        
        force = input("Force destruction? (yes/no): ").strip().lower() == 'yes'
        
        try:
            self.zfs.destroy_pool(pool_name, force)
            print(f"Pool '{pool_name}' destroyed.")
            
            if self.current_pool == pool_name:
                self.current_pool = None
        except Exception as e:
            print(f"Error destroying pool: {e}")

    #-------------------------------------------------
    # Dataset Management Functions
    #-------------------------------------------------
    
    def list_datasets(self) -> None:
        """List datasets in a pool"""
        pool_name = self._get_pool_name("Enter pool name: ")
        if not pool_name:
            return
        
        try:
            datasets = self.zfs.list_datasets(pool_name)
            
            print(f"\nDatasets in pool '{pool_name}':")
            if datasets:
                for i, dataset in enumerate(datasets, 1):
                    print(f"  {i}. {dataset}")
                
                # Allow setting current dataset
                choice = input("\nSelect a dataset number to set as current dataset (or Enter to skip): ").strip()
                if choice.isdigit() and 1 <= int(choice) <= len(datasets):
                    self.current_dataset = datasets[int(choice) - 1]
                    print(f"Current dataset set to: {self.current_dataset}")
            else:
                print("  No datasets found.")
        except Exception as e:
            print(f"Error listing datasets: {e}")
    
    def create_dataset(self) -> None:
        """Create a new dataset"""
        pool_name = self._get_pool_name("Enter pool name: ")
        if not pool_name:
            return
        
        name = input("Enter dataset name (without pool prefix): ").strip()
        if not name:
            print("Dataset name cannot be empty.")
            return
        
        full_name = f"{pool_name}/{name}"
        
        # Get dataset type
        print("\nDataset type:")
        print("  1. Filesystem")
        print("  2. Volume")
        
        type_choice = input("Select type (1-2): ").strip()
        dataset_kind = DatasetKind.FILESYSTEM
        if type_choice == '2':
            dataset_kind = DatasetKind.VOLUME
        
        # Get properties
        props = {}
        if input("Do you want to set properties? (yes/no): ").strip().lower() == 'yes':
            print("Enter properties (compression=lz4, atime=off, etc.). Empty line to finish.")
            while True:
                prop_line = input("> ").strip()
                if not prop_line:
                    break
                    
                try:
                    key, value = prop_line.split('=', 1)
                    props[key.strip()] = value.strip()
                except ValueError:
                    print("Invalid format. Use 'key=value'.")
        
        try:
            self.zfs.create_dataset(full_name, dataset_kind, props if props else None)
            print(f"Dataset '{full_name}' created successfully.")
            self.current_dataset = full_name
        except Exception as e:
            print(f"Error creating dataset: {e}")
    
    def delete_dataset(self) -> None:
        """Delete a dataset"""
        dataset_name = self._get_dataset_name("Enter dataset name to delete: ")
        if not dataset_name:
            return
        
        confirm = input(f"Are you sure you want to delete dataset '{dataset_name}'? (yes/no): ").strip().lower()
        if confirm != 'yes':
            print("Dataset deletion cancelled.")
            return
        
        try:
            self.zfs.delete_dataset(dataset_name)
            print(f"Dataset '{dataset_name}' deleted.")
            
            if self.current_dataset == dataset_name:
                self.current_dataset = None
        except Exception as e:
            print(f"Error deleting dataset: {e}")
    
    def set_dataset_properties(self) -> None:
        """Set properties on a dataset"""
        dataset_name = self._get_dataset_name("Enter dataset name: ")
        if not dataset_name:
            return
        
        print("Enter properties to set (compression=lz4, atime=off, etc.). Empty line to finish.")
        props = {}
        while True:
            prop_line = input("> ").strip()
            if not prop_line:
                break
                
            try:
                key, value = prop_line.split('=', 1)
                props[key.strip()] = value.strip()
            except ValueError:
                print("Invalid format. Use 'key=value'.")
        
        if not props:
            print("No properties specified.")
            return
        
        try:
            self.zfs.set_properties(dataset_name, props)
            print(f"Properties set on '{dataset_name}'.")
        except Exception as e:
            print(f"Error setting properties: {e}")

    #-------------------------------------------------
    # Snapshot Management Functions
    #-------------------------------------------------
    
    def list_snapshots(self) -> None:
        """List snapshots for a dataset"""
        dataset_name = self._get_dataset_name("Enter dataset name: ")
        if not dataset_name:
            return
        
        try:
            snapshots = self.zfs.list_snapshots(dataset_name)
            
            print(f"\nSnapshots for dataset '{dataset_name}':")
            if snapshots:
                for i, snapshot in enumerate(snapshots, 1):
                    print(f"  {i}. {snapshot}")
            else:
                print("  No snapshots found.")
        except Exception as e:
            print(f"Error listing snapshots: {e}")
    
    def create_snapshot(self) -> None:
        """Create a new snapshot"""
        dataset_name = self._get_dataset_name("Enter dataset name: ")
        if not dataset_name:
            return
        
        snapshot_name = input("Enter snapshot name: ").strip()
        if not snapshot_name:
            print("Snapshot name cannot be empty.")
            return
        
        try:
            self.zfs.create_snapshot(dataset_name, snapshot_name)
            print(f"Snapshot '{dataset_name}@{snapshot_name}' created successfully.")
        except Exception as e:
            print(f"Error creating snapshot: {e}")
    
    def delete_snapshot(self) -> None:
        """Delete a snapshot"""
        dataset_name = self._get_dataset_name("Enter dataset name: ")
        if not dataset_name:
            return
        
        # List available snapshots
        try:
            snapshots = self.zfs.list_snapshots(dataset_name)
            
            print(f"\nAvailable snapshots for dataset '{dataset_name}':")
            if not snapshots:
                print("  No snapshots found.")
                return
            
            for i, snapshot in enumerate(snapshots, 1):
                print(f"  {i}. {snapshot}")
            
            # Allow selecting snapshot by number
            choice = input("\nEnter snapshot number to delete: ").strip()
            if not choice.isdigit() or int(choice) < 1 or int(choice) > len(snapshots):
                print("Invalid snapshot number.")
                return
            
            snapshot_name = snapshots[int(choice) - 1].split('@')[1]
            confirm = input(f"Are you sure you want to delete snapshot '{dataset_name}@{snapshot_name}'? (yes/no): ").strip().lower()
            if confirm != 'yes':
                print("Snapshot deletion cancelled.")
                return
            
            self.zfs.delete_snapshot(dataset_name, snapshot_name)
            print(f"Snapshot '{dataset_name}@{snapshot_name}' deleted.")
        except Exception as e:
            print(f"Error during snapshot deletion: {e}")

    #-------------------------------------------------
    # Helper Methods
    #-------------------------------------------------
    
    def _get_pool_name(self, prompt: str) -> Optional[str]:
        """Get pool name from user, offering current pool as default
        
        Args:
            prompt: Prompt to display
            
        Returns:
            Pool name or None if canceled
        """
        default = f" [{self.current_pool}]" if self.current_pool else ""
        pool_input = input(f"{prompt}{default}: ").strip()
        
        if not pool_input and self.current_pool:
            return self.current_pool
        elif not pool_input:
            print("Pool name cannot be empty.")
            return None
        
        return pool_input
    
    def _get_dataset_name(self, prompt: str) -> Optional[str]:
        """Get dataset name from user, offering current dataset as default
        
        Args:
            prompt: Prompt to display
            
        Returns:
            Dataset name or None if canceled
        """
        default = f" [{self.current_dataset}]" if self.current_dataset else ""
        dataset_input = input(f"{prompt}{default}: ").strip()
        
        if not dataset_input and self.current_dataset:
            return self.current_dataset
        elif not dataset_input:
            print("Dataset name cannot be empty.")
            return None
        
        return dataset_input
    
    def _format_size(self, size_bytes: int) -> str:
        """Format bytes to human-readable size
        
        Args:
            size_bytes: Size in bytes
            
        Returns:
            Human-readable size string
        """
        if size_bytes < 0:
            return "0 B"
        
        units = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB"]
        i = 0
        
        while size_bytes >= 1024 and i < len(units) - 1:
            size_bytes /= 1024
            i += 1
        
        return f"{size_bytes:.2f} {units[i]}"


def parse_args():
    """Parse command line arguments"""
    parser = argparse.ArgumentParser(description="ZFS Interactive Management UI")
    
    parser.add_argument("--host", 
                        default=os.environ.get("ZFS_HOST", "localhost"),
                        help="ZFS server hostname or IP (default: localhost)")
    
    parser.add_argument("--port", 
                        type=int,
                        default=int(os.environ.get("ZFS_PORT", "9876")),
                        help="ZFS server port (default: 9876)")
    
    parser.add_argument("--api-key",
                        default=os.environ.get("ZFS_API_KEY"),
                        help="API key for authentication (required)")
    
    return parser.parse_args()


def main():
    """Main entry point"""
    args = parse_args()
    
    if not args.api_key:
        print("Error: API key is required. Set it with --api-key or ZFS_API_KEY environment variable.")
        sys.exit(1)
    
    try:
        ui = ZFSInteractiveUI(args.host, args.port, args.api_key)
        print(f"Connected to ZFS server at {args.host}:{args.port}")
        ui.main_menu()
    except KeyboardInterrupt:
        print("\nExiting...")
    except ConnectionError as e:
        print(f"Error connecting to ZFS server: {e}")
    except AuthenticationError as e:
        print(f"Authentication failed: {e}")
    except Exception as e:
        logger.exception("Unexpected error")
        print(f"Unexpected error: {e}")


if __name__ == "__main__":
    main()
