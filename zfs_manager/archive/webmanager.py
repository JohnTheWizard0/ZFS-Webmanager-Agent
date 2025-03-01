# web_server_example.py
# Example Flask web server that directly uses the remote_zfs library

from flask import Flask, request, jsonify
import logging
from zfs.remote_zfs import ZFSRemote, ZFSConfig, DatasetKind, AuthenticationError, OperationError

app = Flask(__name__)

# ZFS API configuration
ZFS_CONFIG = {
    "host": "192.168.7.102",
    "port": 9876,
    "api_key": "7GMsgWf6OHCVvYwEnxQzPd5qI9N7ZVR8"  # Replace with your actual API key
}

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

def get_zfs_client():
    """Create and return a configured ZFSRemote client"""
    config = ZFSConfig(
        host=ZFS_CONFIG["host"],
        port=ZFS_CONFIG["port"],
        timeout=30,
        api_key=ZFS_CONFIG["api_key"]
    )
    return ZFSRemote(config)

@app.route('/api/datasets', methods=['POST'])
def create_dataset():
    """Create a new ZFS dataset"""
    try:
        data = request.json
        if not data or 'name' not in data:
            return jsonify({"status": "error", "message": "Dataset name is required"}), 400
        
        zfs = get_zfs_client()
        
        # Extract dataset properties
        properties = data.get('properties', {})
        kind_str = data.get('kind', 'filesystem')
        kind = DatasetKind.FILESYSTEM if kind_str == 'filesystem' else DatasetKind.VOLUME
        
        # Create the dataset
        zfs.create_dataset(
            name=data['name'],
            kind=kind,
            properties=properties
        )
        
        return jsonify({
            "status": "success",
            "message": f"Dataset {data['name']} created successfully"
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

@app.route('/api/datasets/<path:pool>', methods=['GET'])
def list_datasets(pool):
    """List all datasets in a pool"""
    try:
        zfs = get_zfs_client()
        datasets = zfs.list_datasets(pool)
        
        return jsonify({
            "status": "success",
            "datasets": datasets
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

@app.route('/api/datasets/<path:name>', methods=['DELETE'])
def delete_dataset(name):
    """Delete a dataset"""
    try:
        zfs = get_zfs_client()
        zfs.delete_dataset(name)
        
        return jsonify({
            "status": "success",
            "message": f"Dataset {name} deleted successfully"
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

@app.route('/api/snapshots/<path:dataset>', methods=['GET'])
def list_snapshots(dataset):
    """List all snapshots for a dataset"""
    try:
        zfs = get_zfs_client()
        snapshots = zfs.list_snapshots(dataset)
        
        return jsonify({
            "status": "success",
            "snapshots": snapshots
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

@app.route('/api/snapshots/<path:dataset>', methods=['POST'])
def create_snapshot(dataset):
    """Create a new snapshot"""
    try:
        data = request.json
        if not data or 'snapshot_name' not in data:
            return jsonify({"status": "error", "message": "Snapshot name is required"}), 400
        
        zfs = get_zfs_client()
        zfs.create_snapshot(dataset, data['snapshot_name'])
        
        return jsonify({
            "status": "success",
            "message": f"Snapshot {dataset}@{data['snapshot_name']} created successfully"
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

@app.route('/api/snapshots/<path:dataset>/<snapshot_name>', methods=['DELETE'])
def delete_snapshot(dataset, snapshot_name):
    """Delete a snapshot"""
    try:
        zfs = get_zfs_client()
        zfs.delete_snapshot(dataset, snapshot_name)
        
        return jsonify({
            "status": "success",
            "message": f"Snapshot {dataset}@{snapshot_name} deleted successfully"
        })
        
    except AuthenticationError as e:
        logger.error(f"Authentication error: {e}")
        return jsonify({"status": "error", "message": "Authentication failed"}), 401
    except OperationError as e:
        logger.error(f"Operation error: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
    except Exception as e:
        logger.exception("Unexpected error")
        return jsonify({"status": "error", "message": "Internal server error"}), 500

if __name__ == '__main__':
    app.run(debug=True, host='0.0.0.0', port=5000)