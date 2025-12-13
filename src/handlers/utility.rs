// handlers/utility.rs
// Utility handlers: execute_command, task_status, format_bytes

use crate::models::{CommandRequest, CommandResponse, LastAction, TaskStatusResponse};
use crate::task_manager::TaskManager;
use crate::utils::{error_response, success_response};
use std::process::Command;
use std::sync::{Arc, RwLock};
use warp::{Rejection, Reply};

/// Execute arbitrary command handler
pub async fn execute_command_handler(
    body: CommandRequest,
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    // Update last action
    if let Ok(mut action) = last_action.write() {
        *action = Some(LastAction::new("execute_command".to_string()));
    }

    let mut cmd = Command::new(&body.command);

    if let Some(args) = body.args {
        cmd.args(args);
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined_output = format!("{}{}", stdout, stderr);

            Ok(success_response(CommandResponse {
                status: "success".to_string(),
                output: combined_output,
                exit_code: output.status.code().unwrap_or(-1),
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to execute command: {}", e))),
    }
}

/// Get task status by task_id
/// GET /v1/tasks/{task_id}
pub async fn get_task_status_handler(
    task_id: String,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Cleanup expired tasks first
    task_manager.cleanup_expired();

    match task_manager.get_task(&task_id) {
        Some(task) => Ok(success_response(TaskStatusResponse::from(&task))),
        None => Ok(error_response(&format!("Task '{}' not found", task_id))),
    }
}

/// Format bytes into human-readable string (e.g., "1.23 GB")
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
