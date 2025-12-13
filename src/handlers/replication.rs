// handlers/replication.rs
// Replication handlers: send_size, send, receive, replicate

use super::utility::format_bytes;
use crate::models::{
    ActionResponse, ReceiveSnapshotRequest, ReplicateSnapshotRequest, SendSizeQuery,
    SendSizeResponse, SendSnapshotRequest, TaskResponse,
};
use crate::task_manager::TaskManager;
use crate::utils::{error_response, success_response};
use crate::zfs_management::ZfsManager;
use std::process::Command;
use warp::{Rejection, Reply};

/// Estimate send stream size for a snapshot
/// GET /v1/snapshots/{dataset}/{snapshot}/send-size
pub async fn send_size_handler(
    snapshot_path: String, // dataset/snapshot_name
    query: SendSizeQuery,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // NOTE: lzc_send_space does NOT support recursive (-R)
        if query.recursive {
            return Ok(error_response("Recursive (-R) size estimation is not supported by FFI. Estimate individual snapshots."));
        }

        let incremental = query.from.is_some();
        let from_snapshot = query.from.as_ref().map(|from| {
            if from.contains('@') {
                from.clone()
            } else {
                format!("{}@{}", dataset, from)
            }
        });

        match zfs
            .estimate_send_size(
                &full_snapshot,
                from_snapshot.as_deref(),
                query.raw,
                false,
            )
            .await
        {
            Ok(estimated_bytes) => {
                let estimated_human = format_bytes(estimated_bytes);

                Ok(success_response(SendSizeResponse {
                    status: "success".to_string(),
                    snapshot: full_snapshot,
                    estimated_bytes,
                    estimated_human,
                    incremental,
                    from_snapshot,
                }))
            }
            Err(e) => Ok(error_response(&e)),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name/send-size",
        ))
    }
}

/// Send snapshot to file
/// POST /v1/snapshots/{dataset}/{snapshot}/send
pub async fn send_snapshot_handler(
    snapshot_path: String, // dataset/snapshot_name
    body: SendSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // Check pool busy state first
        let pool = ZfsManager::get_pool_from_path(&full_snapshot);
        if let Some(busy_task) = task_manager.is_pool_busy(&pool) {
            return Ok(error_response(&format!(
                "Pool '{}' is busy with task '{}'",
                pool, busy_task
            )));
        }

        // Dry run just returns estimated size
        if body.dry_run {
            let mut args = vec!["send", "-n", "-P"];
            if body.raw {
                args.push("-w");
            }
            if body.recursive {
                args.push("-R");
            }
            args.push(&full_snapshot);

            let output = Command::new("zfs").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut bytes: u64 = 0;
                    for line in stdout.lines() {
                        if line.starts_with("size") {
                            if let Some(s) = line.split_whitespace().nth(1) {
                                bytes = s.parse().unwrap_or(0);
                            }
                        }
                    }
                    return Ok(success_response(SendSizeResponse {
                        status: "success".to_string(),
                        snapshot: full_snapshot,
                        estimated_bytes: bytes,
                        estimated_human: format_bytes(bytes),
                        incremental: body.from_snapshot.is_some(),
                        from_snapshot: body.from_snapshot,
                    }));
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Ok(error_response(&format!(
                        "Dry run failed: {}",
                        stderr.trim()
                    )));
                }
                Err(e) => {
                    return Ok(error_response(&format!("Failed to estimate: {}", e)));
                }
            }
        }

        // Create task
        let task_id = match task_manager
            .create_task(crate::models::TaskOperation::Send, vec![pool.clone()])
        {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

        // Mark task running
        task_manager.mark_running(&task_id);

        // Execute send operation
        let from_snap = body.from_snapshot.as_deref();
        let result = zfs
            .send_snapshot_to_file(
                &full_snapshot,
                &body.output_file,
                from_snap,
                body.recursive,
                body.properties,
                body.raw,
                body.compressed,
                body.large_blocks,
                body.overwrite,
            )
            .await;

        match result {
            Ok(bytes_written) => {
                task_manager.complete_task(
                    &task_id,
                    serde_json::json!({
                        "bytes_written": bytes_written,
                        "snapshot": full_snapshot,
                        "output_file": body.output_file,
                    }),
                );

                Ok(success_response(TaskResponse {
                    status: "success".to_string(),
                    task_id,
                    message: Some(format!(
                        "Snapshot '{}' sent to '{}' ({} bytes)",
                        full_snapshot, body.output_file, bytes_written
                    )),
                }))
            }
            Err(e) => {
                task_manager.fail_task(&task_id, e.clone());
                Ok(error_response(&e))
            }
        }
    } else {
        Ok(error_response("Invalid snapshot path"))
    }
}

/// Receive snapshot from file
/// POST /v1/datasets/{path}/receive
pub async fn receive_snapshot_handler(
    target_dataset: String,
    body: ReceiveSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Dry run validation only
    if body.dry_run {
        if !std::path::Path::new(&body.input_file).exists() {
            return Ok(error_response(&format!(
                "Input file '{}' does not exist",
                body.input_file
            )));
        }
        return Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Dry run: would receive from '{}' to '{}'",
                body.input_file, target_dataset
            ),
        }));
    }

    // Check pool busy state
    let pool = ZfsManager::get_pool_from_path(&target_dataset);
    if let Some(busy_task) = task_manager.is_pool_busy(&pool) {
        return Ok(error_response(&format!(
            "Pool '{}' is busy with task '{}'",
            pool, busy_task
        )));
    }

    // Create task
    let task_id =
        match task_manager.create_task(crate::models::TaskOperation::Receive, vec![pool.clone()]) {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

    // Mark task running
    task_manager.mark_running(&task_id);

    // Execute receive operation
    let result = zfs
        .receive_snapshot_from_file(&target_dataset, &body.input_file, body.force)
        .await;

    match result {
        Ok(output) => {
            task_manager.complete_task(
                &task_id,
                serde_json::json!({
                    "target_dataset": target_dataset,
                    "input_file": body.input_file,
                    "output": output,
                }),
            );

            Ok(success_response(TaskResponse {
                status: "success".to_string(),
                task_id,
                message: Some(format!(
                    "Received to dataset '{}' from '{}'",
                    target_dataset, body.input_file
                )),
            }))
        }
        Err(e) => {
            task_manager.fail_task(&task_id, e.clone());
            Ok(error_response(&e))
        }
    }
}

/// Replicate snapshot directly to another pool (pipe send -> receive)
/// POST /v1/snapshots/{dataset}/{snapshot}/replicate
pub async fn replicate_snapshot_handler(
    snapshot_path: String, // dataset/snapshot_name
    body: ReplicateSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // Get pools for both source and target
        let source_pool = ZfsManager::get_pool_from_path(&full_snapshot);
        let target_pool = ZfsManager::get_pool_from_path(&body.target_dataset);

        // Check source pool busy state
        if let Some(busy_task) = task_manager.is_pool_busy(&source_pool) {
            return Ok(error_response(&format!(
                "Source pool '{}' is busy with task '{}'",
                source_pool, busy_task
            )));
        }

        // Check target pool busy state (if different from source)
        if source_pool != target_pool {
            if let Some(busy_task) = task_manager.is_pool_busy(&target_pool) {
                return Ok(error_response(&format!(
                    "Target pool '{}' is busy with task '{}'",
                    target_pool, busy_task
                )));
            }
        }

        // Dry run returns estimated size
        if body.dry_run {
            let mut args = vec!["send", "-n", "-P"];
            if body.raw {
                args.push("-w");
            }
            if body.recursive {
                args.push("-R");
            }
            args.push(&full_snapshot);

            let output = Command::new("zfs").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut bytes: u64 = 0;
                    for line in stdout.lines() {
                        if line.starts_with("size") {
                            if let Some(s) = line.split_whitespace().nth(1) {
                                bytes = s.parse().unwrap_or(0);
                            }
                        }
                    }
                    return Ok(success_response(SendSizeResponse {
                        status: "success".to_string(),
                        snapshot: full_snapshot,
                        estimated_bytes: bytes,
                        estimated_human: format_bytes(bytes),
                        incremental: body.from_snapshot.is_some(),
                        from_snapshot: body.from_snapshot,
                    }));
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Ok(error_response(&format!(
                        "Dry run failed: {}",
                        stderr.trim()
                    )));
                }
                Err(e) => {
                    return Ok(error_response(&format!("Failed to estimate: {}", e)));
                }
            }
        }

        // Mark BOTH pools as busy
        let pools = if source_pool != target_pool {
            vec![source_pool.clone(), target_pool.clone()]
        } else {
            vec![source_pool.clone()]
        };

        // Create task
        let task_id = match task_manager.create_task(crate::models::TaskOperation::Replicate, pools)
        {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

        // Mark task running
        task_manager.mark_running(&task_id);

        // Execute replication
        let from_snap = body.from_snapshot.as_deref();
        let result = zfs
            .replicate_snapshot(
                &full_snapshot,
                &body.target_dataset,
                from_snap,
                body.recursive,
                body.properties,
                body.raw,
                body.compressed,
                body.force,
            )
            .await;

        match result {
            Ok(output) => {
                task_manager.complete_task(
                    &task_id,
                    serde_json::json!({
                        "source": full_snapshot,
                        "target": body.target_dataset,
                        "output": output,
                    }),
                );

                Ok(success_response(TaskResponse {
                    status: "success".to_string(),
                    task_id,
                    message: Some(format!(
                        "Replicated '{}' to '{}'",
                        full_snapshot, body.target_dataset
                    )),
                }))
            }
            Err(e) => {
                task_manager.fail_task(&task_id, e.clone());
                Ok(error_response(&e))
            }
        }
    } else {
        Ok(error_response("Invalid snapshot path"))
    }
}
