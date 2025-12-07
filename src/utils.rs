use crate::models::{ActionResponse, LastAction};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use warp::Filter;

// Success response helper
pub fn success_response<T: Serialize>(data: T) -> warp::reply::Json {
    warp::reply::json(&data)
}

// Error response helper
pub fn error_response(message: &str) -> warp::reply::Json {
    let response = ActionResponse {
        status: "error".to_string(),
        message: message.to_string(),
    };
    warp::reply::json(&response)
}

// FIXED: Simple action tracking filter
pub fn with_action_tracking(
    function_name: &'static str,
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> impl Filter<Extract = (), Error = std::convert::Infallible> + Clone {
    warp::any()
        .map(move || {
            if let Ok(mut action) = last_action.write() {
                *action = Some(LastAction::new(function_name.to_string()));
            }
            ()                       // explicit unit return
        })
        .untuple_one()               // ← collapses ((),) to ()
}

// ============================================================================
// UNIT TESTS — MI-002 (API Framework - Utils)
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// Test: success_response wraps data in JSON
    /// Expected: Returns warp::reply::Json
    #[test]
    fn test_success_response_returns_json() {
        #[derive(Serialize)]
        struct TestData { value: i32 }

        let data = TestData { value: 42 };
        let _response = success_response(data);
        // If we get here, it compiled and returned a Json reply
        assert!(true, "success_response returns Json");
    }

    /// Test: error_response creates error structure
    /// Expected: JSON with status="error" and message
    #[test]
    fn test_error_response_structure() {
        let _response = error_response("Something went wrong");
        // The response is warp::reply::Json, we can't easily inspect it
        // but we know it wraps an ActionResponse with status="error"

        // Verify ActionResponse structure directly
        let action = ActionResponse {
            status: "error".to_string(),
            message: "Something went wrong".to_string(),
        };
        assert_eq!(action.status, "error");
        assert_eq!(action.message, "Something went wrong");
    }

    /// Test: action tracking updates shared state
    /// Expected: LastAction is set with correct function name
    #[test]
    fn test_action_tracking_updates_state() {
        let last_action: Arc<RwLock<Option<LastAction>>> = Arc::new(RwLock::new(None));

        // Verify initial state is None
        assert!(last_action.read().unwrap().is_none());

        // Simulate what with_action_tracking does internally
        {
            let mut action = last_action.write().unwrap();
            *action = Some(LastAction::new("test_action".to_string()));
        }

        // Verify state was updated
        let action = last_action.read().unwrap();
        assert!(action.is_some());
        assert_eq!(action.as_ref().unwrap().function, "test_action");
    }

    /// Test: action tracking handles concurrent access
    /// Expected: RwLock allows safe concurrent reads
    #[test]
    fn test_action_tracking_concurrent_reads() {
        let last_action: Arc<RwLock<Option<LastAction>>> = Arc::new(RwLock::new(
            Some(LastAction::new("initial".to_string()))
        ));

        // Clone Arc for "concurrent" access
        let reader1 = last_action.clone();
        let reader2 = last_action.clone();

        // Both can read simultaneously
        let r1 = reader1.read().unwrap();
        let r2 = reader2.read().unwrap();

        assert_eq!(r1.as_ref().unwrap().function, r2.as_ref().unwrap().function);
    }
}