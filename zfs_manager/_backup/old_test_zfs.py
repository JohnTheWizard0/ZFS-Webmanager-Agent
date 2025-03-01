import requests
import sys
from typing import Optional, Dict, Any

BASE_URL = "http://192.168.7.100:9876"  # Updated to match your host IP

# Dataset operations
def create_dataset(name: str, properties: Dict[str, Any] = None) -> None:
    """Create a new dataset with optional properties."""
    url = f"{BASE_URL}/datasets"
    payload = {
        "name": name,
        "kind": "filesystem",  # Default to filesystem, as it's the most common
        "properties": properties or {}
    }
    
    try:
        response = requests.post(url, json=payload)
        response.raise_for_status()
        print(f"Successfully created dataset: {name}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to create dataset: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

def list_datasets(pool: str) -> None:
    """List all datasets in a pool."""
    url = f"{BASE_URL}/datasets/{pool}"
    
    try:
        response = requests.get(url)
        response.raise_for_status()
        data = response.json()
        
        if not data.get("datasets"):
            print(f"No datasets found in pool: {pool}")
            return
            
        print(f"\nDatasets in {pool}:")
        for dataset in data["datasets"]:
            print(f"  - {dataset}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to list datasets: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

def delete_dataset(name: str) -> None:
    """Delete a specific dataset."""
    url = f"{BASE_URL}/datasets/{name}"
    
    try:
        response = requests.delete(url)
        response.raise_for_status()
        print(f"Successfully deleted dataset: {name}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to delete dataset: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

# Snapshot operations
def create_snapshot(dataset: str, snapshot_name: str) -> None:
    """Create a new snapshot."""
    url = f"{BASE_URL}/snapshots/{dataset}"
    payload = {"snapshot_name": snapshot_name}
    
    try:
        response = requests.post(url, json=payload)
        response.raise_for_status()
        print(f"Successfully created snapshot: {dataset}@{snapshot_name}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to create snapshot: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

def list_snapshots(dataset: str) -> None:
    """List all snapshots for a dataset."""
    url = f"{BASE_URL}/snapshots/{dataset}"
    
    try:
        response = requests.get(url)
        response.raise_for_status()
        data = response.json()
        
        if not data.get("snapshots"):
            print(f"No snapshots found for dataset: {dataset}")
            return
            
        print(f"\nSnapshots for {dataset}:")
        for snapshot in data["snapshots"]:
            print(f"  - {snapshot}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to list snapshots: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

def delete_snapshot(dataset: str, snapshot_name: str) -> None:
    """Delete a specific snapshot."""
    url = f"{BASE_URL}/snapshots/{dataset}/{snapshot_name}"
    
    try:
        response = requests.delete(url)
        response.raise_for_status()
        print(f"Successfully deleted snapshot: {dataset}@{snapshot_name}")
    except requests.exceptions.RequestException as e:
        print(f"Failed to delete snapshot: {e}")
        if hasattr(e.response, 'text'):
            print(f"Server response: {e.response.text}")

def get_user_input(prompt: str, allow_empty: bool = False) -> Optional[str]:
    """Get user input with optional empty value handling."""
    while True:
        value = input(prompt).strip()
        if value or allow_empty:
            return value
        print("This field cannot be empty. Please try again.")

def main():
    while True:
        print("\nZFS Management")
        print("\nDataset Operations:")
        print("1. Create dataset")
        print("2. List datasets")
        print("3. Delete dataset")
        print("\nSnapshot Operations:")
        print("4. Create snapshot")
        print("5. List snapshots")
        print("6. Delete snapshot")
        print("\n7. Exit")
        
        choice = get_user_input("\nSelect an option (1-7): ")
        
        if choice == "1":
            name = get_user_input("Enter dataset name (e.g., mypool/dataset1): ")
            kind = get_user_input("Enter dataset kind (filesystem/volume) [filesystem]: ") or "filesystem"
            properties = {}
            if get_user_input("Add custom properties? (y/n): ").lower() == 'y':
                while True:
                    prop_name = get_user_input("Enter property name (or press Enter to finish): ", allow_empty=True)
                    if not prop_name:
                        break
                    prop_value = get_user_input(f"Enter value for {prop_name}: ")
                    properties[prop_name] = prop_value
            create_dataset(name, properties)
            
        elif choice == "2":
            pool = get_user_input("Enter pool name to list datasets from: ")
            list_datasets(pool)
            
        elif choice == "3":
            name = get_user_input("Enter dataset name to delete: ")
            confirm = get_user_input(f"Are you sure you want to delete {name}? (y/n): ")
            if confirm.lower() == 'y':
                delete_dataset(name)
            
        elif choice == "4":
            dataset = get_user_input("Enter dataset (e.g., mypool/dataset1): ")
            snapshot_name = get_user_input("Enter snapshot name: ")
            create_snapshot(dataset, snapshot_name)
            
        elif choice == "5":
            dataset = get_user_input("Enter dataset to list snapshots from: ")
            list_snapshots(dataset)
            
        elif choice == "6":
            dataset = get_user_input("Enter dataset: ")
            snapshot_name = get_user_input("Enter snapshot name to delete: ")
            confirm = get_user_input(f"Are you sure you want to delete {dataset}@{snapshot_name}? (y/n): ")
            if confirm.lower() == 'y':
                delete_snapshot(dataset, snapshot_name)
            
        elif choice == "7":
            print("Goodbye!")
            sys.exit(0)
            
        else:
            print("Invalid option. Please try again.")
        
        input("\nPress Enter to continue...")

if __name__ == "__main__":
    main()
