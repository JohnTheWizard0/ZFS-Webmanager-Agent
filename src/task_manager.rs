//! Task Manager for async replication operations (MF-005)
//!
//! Handles:
//! - Task creation and tracking
//! - Pool busy state management (one task per pool)
//! - Task expiry after 1 hour
//! - Progress updates

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::models::{TaskState, TaskStatus, TaskOperation, TaskProgress};

/// Task expiry time in seconds (1 hour)
const TASK_EXPIRY_SECS: u64 = 3600;

/// Task manager for async operations
#[derive(Clone)]
pub struct TaskManager {
    /// All tasks indexed by task_id
    tasks: Arc<RwLock<HashMap<String, TaskState>>>,
    /// Pools currently busy (pool_name -> task_id)
    busy_pools: Arc<RwLock<HashMap<String, String>>>,
}

impl TaskManager {
    /// Create a new TaskManager
    pub fn new() -> Self {
        TaskManager {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            busy_pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Check if a pool is busy
    pub fn is_pool_busy(&self, pool: &str) -> Option<String> {
        let busy = self.busy_pools.read().unwrap();
        busy.get(pool).cloned()
    }

    /// Check if any of the given pools are busy
    /// Returns the first busy pool name and its task_id, or None
    pub fn any_pool_busy(&self, pools: &[String]) -> Option<(String, String)> {
        let busy = self.busy_pools.read().unwrap();
        for pool in pools {
            if let Some(task_id) = busy.get(pool) {
                return Some((pool.clone(), task_id.clone()));
            }
        }
        None
    }

    /// Create a new task
    /// Returns Err if any pool is already busy
    pub fn create_task(
        &self,
        operation: TaskOperation,
        pools: Vec<String>,
    ) -> Result<String, (String, String)> {
        // Check if any pool is busy
        if let Some((pool, task_id)) = self.any_pool_busy(&pools) {
            return Err((pool, task_id));
        }

        let task_id = format!("{}-{}",
            match operation {
                TaskOperation::Send => "send",
                TaskOperation::Receive => "recv",
                TaskOperation::Replicate => "repl",
            },
            &Uuid::new_v4().to_string()[..8]
        );

        let task = TaskState {
            task_id: task_id.clone(),
            status: TaskStatus::Pending,
            operation,
            pools_involved: pools.clone(),
            started_at: Self::now(),
            completed_at: None,
            progress: None,
            result: None,
            error: None,
        };

        // Mark pools as busy
        {
            let mut busy = self.busy_pools.write().unwrap();
            for pool in &pools {
                busy.insert(pool.clone(), task_id.clone());
            }
        }

        // Store task
        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task_id.clone(), task);
        }

        Ok(task_id)
    }

    /// Get task by ID
    pub fn get_task(&self, task_id: &str) -> Option<TaskState> {
        let tasks = self.tasks.read().unwrap();
        tasks.get(task_id).cloned()
    }

    /// Update task status to running
    pub fn mark_running(&self, task_id: &str) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Running;
        }
    }

    /// Update task progress
    pub fn update_progress(&self, task_id: &str, bytes_processed: u64, bytes_total: Option<u64>) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            let percent = bytes_total.map(|total| {
                if total > 0 {
                    (bytes_processed as f32 / total as f32) * 100.0
                } else {
                    0.0
                }
            });
            task.progress = Some(TaskProgress {
                bytes_processed,
                bytes_total,
                percent,
            });
        }
    }

    /// Mark task as completed with result
    pub fn complete_task(&self, task_id: &str, result: serde_json::Value) {
        // Release pools first
        self.release_pools(task_id);

        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            task.completed_at = Some(Self::now());
            task.result = Some(result);
        }
    }

    /// Mark task as failed with error
    pub fn fail_task(&self, task_id: &str, error: String) {
        // Release pools first
        self.release_pools(task_id);

        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.completed_at = Some(Self::now());
            task.error = Some(error);
        }
    }

    /// Release pools associated with a task
    fn release_pools(&self, task_id: &str) {
        let pools_to_release: Vec<String> = {
            let tasks = self.tasks.read().unwrap();
            tasks.get(task_id)
                .map(|t| t.pools_involved.clone())
                .unwrap_or_default()
        };

        let mut busy = self.busy_pools.write().unwrap();
        for pool in pools_to_release {
            if busy.get(&pool) == Some(&task_id.to_string()) {
                busy.remove(&pool);
            }
        }
    }

    /// Clean up expired tasks (completed/failed > 1 hour ago)
    pub fn cleanup_expired(&self) {
        let now = Self::now();
        let mut tasks = self.tasks.write().unwrap();

        tasks.retain(|_, task| {
            match task.status {
                TaskStatus::Completed | TaskStatus::Failed => {
                    if let Some(completed_at) = task.completed_at {
                        // Keep if not yet expired
                        now - completed_at < TASK_EXPIRY_SECS
                    } else {
                        true
                    }
                }
                // Always keep pending/running tasks
                _ => true
            }
        });
    }

    /// List all tasks (for debugging)
    pub fn list_tasks(&self) -> Vec<TaskState> {
        let tasks = self.tasks.read().unwrap();
        tasks.values().cloned().collect()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task() {
        let tm = TaskManager::new();
        let task_id = tm.create_task(TaskOperation::Send, vec!["tank".to_string()]).unwrap();
        assert!(task_id.starts_with("send-"));

        let task = tm.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.pools_involved, vec!["tank".to_string()]);
    }

    #[test]
    fn test_pool_busy() {
        let tm = TaskManager::new();
        let task_id = tm.create_task(TaskOperation::Send, vec!["tank".to_string()]).unwrap();

        // Same pool should fail
        let result = tm.create_task(TaskOperation::Receive, vec!["tank".to_string()]);
        assert!(result.is_err());
        let (pool, busy_task) = result.unwrap_err();
        assert_eq!(pool, "tank");
        assert_eq!(busy_task, task_id);

        // Different pool should succeed
        let task2 = tm.create_task(TaskOperation::Send, vec!["other".to_string()]);
        assert!(task2.is_ok());
    }

    #[test]
    fn test_replicate_marks_both_pools_busy() {
        let tm = TaskManager::new();
        let _ = tm.create_task(
            TaskOperation::Replicate,
            vec!["source".to_string(), "target".to_string()]
        ).unwrap();

        // Both pools should be busy
        assert!(tm.is_pool_busy("source").is_some());
        assert!(tm.is_pool_busy("target").is_some());
    }

    #[test]
    fn test_complete_releases_pools() {
        let tm = TaskManager::new();
        let task_id = tm.create_task(TaskOperation::Send, vec!["tank".to_string()]).unwrap();

        assert!(tm.is_pool_busy("tank").is_some());

        tm.complete_task(&task_id, serde_json::json!({"bytes": 1000}));

        assert!(tm.is_pool_busy("tank").is_none());
    }

    #[test]
    fn test_fail_releases_pools() {
        let tm = TaskManager::new();
        let task_id = tm.create_task(TaskOperation::Send, vec!["tank".to_string()]).unwrap();

        tm.fail_task(&task_id, "error".to_string());

        assert!(tm.is_pool_busy("tank").is_none());

        let task = tm.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.error, Some("error".to_string()));
    }

    #[test]
    fn test_progress_update() {
        let tm = TaskManager::new();
        let task_id = tm.create_task(TaskOperation::Send, vec!["tank".to_string()]).unwrap();

        tm.mark_running(&task_id);
        tm.update_progress(&task_id, 500, Some(1000));

        let task = tm.get_task(&task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        let progress = task.progress.unwrap();
        assert_eq!(progress.bytes_processed, 500);
        assert_eq!(progress.bytes_total, Some(1000));
        assert!((progress.percent.unwrap() - 50.0).abs() < 0.1);
    }
}
